use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::features::plugin::types::{PluginInvocation, PluginManifest, PluginStatus};
use crate::infra::ipc::{generate_request_id, PluginRequest};

/// Per-action timeout limits (SPEC §11).
pub const TIMEOUT_SEARCH: Duration = Duration::from_secs(10);
pub const TIMEOUT_STREAM: Duration = Duration::from_secs(30);
pub const TIMEOUT_RESOLVE: Duration = Duration::from_secs(5);
pub const TIMEOUT_PLAYLIST: Duration = Duration::from_secs(30);
pub const TIMEOUT_LYRICS: Duration = Duration::from_secs(25);
/// After a plugin has answered, give it a moment to exit before killing it.
pub const EXIT_GRACE: Duration = Duration::from_millis(500);

fn timeout_for(action: &str) -> Duration {
    match action {
        "search" => TIMEOUT_SEARCH,
        "stream" => TIMEOUT_STREAM,
        "resolve" => TIMEOUT_RESOLVE,
        "playlist" => TIMEOUT_PLAYLIST,
        "lyrics" => TIMEOUT_LYRICS,
        _ => TIMEOUT_SEARCH,
    }
}

#[derive(Debug, Deserialize)]
struct WireResponse {
    id: String,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<Value>,
}

fn format_plugin_error(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(message) = value.get("message").and_then(Value::as_str) {
        return message.to_string();
    }
    value.to_string()
}

/// One-shot plugin runner.
///
/// Plugins are not long-lived: each invocation spawns the configured argv,
/// writes one request line, closes stdin, reads one response line, then lets
/// the process exit (or kills it after a short grace period).
pub struct PluginRuntime {
    /// Per-plugin concurrency limit; a plugin's tasks may overlap, but the
    /// process itself is one-shot.
    semaphores: Mutex<HashMap<String, Arc<Semaphore>>>,
    default_concurrency: usize,
    statuses: Mutex<HashMap<String, PluginStatus>>,
    last_stderr: Mutex<HashMap<String, String>>,
}

impl PluginRuntime {
    pub fn new(default_concurrency: usize) -> Self {
        PluginRuntime {
            semaphores: Mutex::new(HashMap::new()),
            default_concurrency: default_concurrency.max(1),
            statuses: Mutex::new(HashMap::new()),
            last_stderr: Mutex::new(HashMap::new()),
        }
    }

    pub async fn set_concurrency(&self, plugin: &str, max: usize) {
        let mut sems = self.semaphores.lock().await;
        sems.insert(plugin.to_string(), Arc::new(Semaphore::new(max.max(1))));
    }

    pub async fn status(&self, plugin: &str) -> PluginStatus {
        self.statuses
            .lock()
            .await
            .get(plugin)
            .cloned()
            .unwrap_or(PluginStatus::Idle)
    }

    async fn set_status(&self, plugin: &str, status: PluginStatus) {
        self.statuses
            .lock()
            .await
            .insert(plugin.to_string(), status);
    }

    pub async fn last_stderr(&self, plugin: &str) -> Option<String> {
        self.last_stderr.lock().await.get(plugin).cloned()
    }

    async fn semaphore_for(&self, plugin: &str) -> Arc<Semaphore> {
        let mut sems = self.semaphores.lock().await;
        sems.entry(plugin.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(self.default_concurrency)))
            .clone()
    }

    /// Run one request against one plugin process.
    pub async fn invoke(
        &self,
        manifest: &PluginManifest,
        argv: &[String],
        action: &str,
        params: Value,
    ) -> Result<PluginInvocation, String> {
        if argv.is_empty() {
            return Err(format!("plugin '{}' has empty argv", manifest.name));
        }

        let semaphore = self.semaphore_for(&manifest.name).await;
        let _permit = semaphore
            .acquire()
            .await
            .map_err(|_| format!("semaphore closed for plugin '{}'", manifest.name))?;

        self.set_status(&manifest.name, PluginStatus::Running).await;

        let request_id = generate_request_id();
        let request = PluginRequest {
            id: request_id.clone(),
            action: action.to_string(),
            params,
        };
        let line = serde_json::to_string(&request)
            .map_err(|e| format!("request serialization error: {e}"))?;

        let mut command = Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .current_dir(&manifest.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .map_err(|e| format!("failed to spawn plugin '{}': {e}", manifest.name))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("plugin '{}' has no stdin", manifest.name))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("plugin '{}' has no stdout", manifest.name))?;
        let stderr = child.stderr.take();

        // Drain stderr concurrently so a chatty plugin cannot block on a full
        // pipe while the core waits for its JSON response.
        let stderr_task = tokio::spawn(async move {
            let mut text = String::new();
            if let Some(mut stderr) = stderr {
                let _ = stderr.read_to_string(&mut text).await;
            }
            text
        });

        if let Err(e) = async {
            stdin.write_all(line.as_bytes()).await?;
            stdin.write_all(b"
").await?;
            stdin.flush().await?;
            Ok::<(), std::io::Error>(())
        }
        .await
        {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let stderr_text = stderr_task.await.unwrap_or_default();
            self.finish_error(&manifest.name, &stderr_text, format!("write error: {e}"))
                .await;
            return Err(format!("write error to plugin '{}': {e}", manifest.name));
        }
        drop(stdin);

        let duration = timeout_for(action);
        let response = timeout(duration, async {
            let mut reader = BufReader::new(stdout);
            let mut logs: Vec<String> = Vec::new();
            let mut line = String::new();
            loop {
                line.clear();
                let n = reader
                    .read_line(&mut line)
                    .await
                    .map_err(|e| format!("read error: {e}"))?;
                if n == 0 {
                    return Err("plugin closed stdout before a response".to_string());
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<WireResponse>(trimmed) {
                    Ok(response) => {
                        if response.id != request_id {
                            logs.push(format!(
                                "ignored response with mismatched id: {}",
                                response.id
                            ));
                            continue;
                        }
                        if let Some(error) = response.error {
                            return Err(format_plugin_error(&error));
                        }
                        return Ok((response.result.unwrap_or(Value::Null), logs));
                    }
                    Err(_) => {
                        // Non-JSON stdout is not protocol; keep it for the UI.
                        logs.push(trimmed.to_string());
                    }
                }
            }
        })
        .await;

        let (result, logs, error_message) = match response {
            Ok(Ok((value, logs))) => (Some(value), logs, None),
            Ok(Err(e)) => (None, Vec::new(), Some(e)),
            Err(_) => (None, Vec::new(), Some(format!("timeout after {duration:?}"))),
        };

        let exit_status = if error_message.is_some() {
            let _ = child.kill().await;
            None
        } else {
            match timeout(EXIT_GRACE, child.wait()).await {
                Ok(Ok(status)) => Some(status),
                Ok(Err(_)) => None,
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    None
                }
            }
        };
        if error_message.is_some() {
            let _ = child.wait().await;
        }

        let stderr_text = stderr_task.await.unwrap_or_default();
        if !stderr_text.is_empty() {
            self.last_stderr
                .lock()
                .await
                .insert(manifest.name.clone(), stderr_text.clone());
        }

        if let Some(message) = error_message {
            self.set_status(
                &manifest.name,
                PluginStatus::Error(message.clone()),
            )
            .await;
            warn!("plugin '{}' failed: {message}", manifest.name);
            return Err(format!("plugin '{}': {message}", manifest.name));
        }

        let exit_code = exit_status.and_then(|status| status.code());
        self.set_status(&manifest.name, PluginStatus::Exited(exit_code))
            .await;
        debug!("plugin '{}' finished {:?}", manifest.name, exit_code);

        Ok(PluginInvocation {
            result: result.unwrap_or(Value::Null),
            stderr: stderr_text,
            logs,
            exit_code,
        })
    }

    async fn finish_error(&self, plugin: &str, stderr: &str, message: String) {
        if !stderr.is_empty() {
            self.last_stderr
                .lock()
                .await
                .insert(plugin.to_string(), stderr.to_string());
        }
        self.set_status(plugin, PluginStatus::Error(message)).await;
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::features::plugin::types::PluginStatus;
    use std::path::PathBuf;

    fn manifest(dir: PathBuf, script: &str) -> PluginManifest {
        PluginManifest {
            protocol: 1,
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            exec: vec!["sh".to_string(), "-c".to_string(), script.to_string()],
            capabilities: vec![],
            dir,
        }
    }

    #[tokio::test]
    async fn one_shot_invocation_returns_result_and_logs() {
        let dir = std::env::temp_dir().join(format!(
            "vide-plugin-runtime-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = r#"read line
id=$(printf '%s' "$line" | awk -F'"' '{print $4}')
echo 'not json'
printf '{"id":"%s","result":{"ok":true}}\n' "$id"
"#;
        let runtime = PluginRuntime::new(2);
        let plugin = manifest(dir.clone(), script);
        let invocation = runtime
            .invoke(&plugin, &plugin.exec, "search", serde_json::json!({"query":"x"}))
            .await
            .unwrap();
        assert_eq!(invocation.result["ok"], true);
        assert!(invocation.logs.iter().any(|line| line == "not json"));
        assert_eq!(invocation.exit_code, Some(0));
        assert!(matches!(
            runtime.status("test-plugin").await,
            PluginStatus::Exited(Some(0))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn one_shot_invocation_reports_error() {
        let dir = std::env::temp_dir().join(format!(
            "vide-plugin-runtime-error-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = r#"read line
id=$(printf '%s' "$line" | awk -F'"' '{print $4}')
printf '{"id":"%s","error":{"code":"failed","message":"boom"}}\n' "$id"
"#;
        let runtime = PluginRuntime::new(2);
        let plugin = manifest(dir.clone(), script);
        let error = runtime
            .invoke(&plugin, &plugin.exec, "resolve", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(error.contains("boom"), "{error}");
        assert!(matches!(
            runtime.status("test-plugin").await,
            PluginStatus::Error(_)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
