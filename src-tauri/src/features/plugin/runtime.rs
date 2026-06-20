use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::features::plugin::types::PluginManifest;
use crate::infra::ipc::{generate_request_id, PluginRequest, PluginResponse};

/// Per-action timeout limits (SPEC §11).
pub const TIMEOUT_SEARCH: Duration = Duration::from_secs(10);
pub const TIMEOUT_STREAM: Duration = Duration::from_secs(30);
pub const TIMEOUT_RESOLVE: Duration = Duration::from_secs(5);

/// A running plugin process handle.
struct PluginProcess {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
}

/// Manages plugin subprocess lifecycle and message dispatch.
pub struct PluginRuntime {
    /// Running plugin processes: plugin_name → process handle.
    processes: Mutex<HashMap<String, PluginProcess>>,
    /// Per-plugin concurrency semaphores: plugin_name → max requests.
    semaphores: Mutex<HashMap<String, Arc<Semaphore>>>,
    /// Default concurrency limit.
    default_concurrency: usize,
}

impl PluginRuntime {
    /// Create a new empty runtime.
    pub fn new(default_concurrency: usize) -> Self {
        PluginRuntime {
            processes: Mutex::new(HashMap::new()),
            semaphores: Mutex::new(HashMap::new()),
            default_concurrency,
        }
    }

    /// Start a plugin process if not already running.
    pub async fn ensure_running(&self, manifest: &PluginManifest) -> Result<(), String> {
        let mut procs = self.processes.lock().await;
        if procs.contains_key(&manifest.name) {
            return Ok(());
        }

        let mut child = Command::new(&manifest.exec[0])
            .args(&manifest.exec[1..])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn plugin '{}': {e}", manifest.name))?;

        let stdin = child.stdin.take().ok_or_else(|| "stdin not available".to_string())?;
        let stdout = child.stdout.take().ok_or_else(|| "stdout not available".to_string())?;

        let process = PluginProcess {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        };

        procs.insert(manifest.name.clone(), process);
        debug!("plugin '{}' started", manifest.name);
        Ok(())
    }

    /// Send a request to a plugin and wait for its response.
    ///
    /// Automatically acquires the concurrency semaphore and enforces
    /// the action-specific timeout.
    pub async fn send_request(
        &self,
        plugin_name: &str,
        action: &str,
        params: Value,
    ) -> Result<Value, String> {
        let semaphore = {
            let sems = self.semaphores.lock().await;
            sems.get(plugin_name)
                .cloned()
                .unwrap_or_else(|| Arc::new(Semaphore::new(self.default_concurrency)))
        };

        let _permit = semaphore
            .acquire()
            .await
            .map_err(|_| format!("semaphore closed for plugin '{plugin_name}'"))?;

        let mut procs = self.processes.lock().await;
        let proc = procs
            .get_mut(plugin_name)
            .ok_or_else(|| format!("plugin '{plugin_name}' is not running"))?;

        let request_id = generate_request_id();
        let req = PluginRequest {
            id: request_id.clone(),
            action: action.to_string(),
            params,
        };

        // Write request
        let req_line =
            serde_json::to_string(&req).map_err(|e| format!("serialization error: {e}"))?;
        proc.stdin
            .write_all(req_line.as_bytes())
            .await
            .map_err(|e| format!("write error to plugin '{plugin_name}': {e}"))?;
        proc.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| format!("write error to plugin '{plugin_name}': {e}"))?;
        proc.stdin
            .flush()
            .await
            .map_err(|e| format!("flush error for plugin '{plugin_name}': {e}"))?;

        // Determine timeout based on action
        let duration = match action {
            "search" => TIMEOUT_SEARCH,
            "stream" => TIMEOUT_STREAM,
            "resolve" => TIMEOUT_RESOLVE,
            _ => TIMEOUT_SEARCH,
        };

        // Read response with timeout
        let mut line = String::new();
        let read_future = proc.stdout.read_line(&mut line);
        match timeout(duration, read_future).await {
            Ok(Ok(0)) => {
                return Err(format!("plugin '{plugin_name}' closed stdout"));
            }
            Ok(Ok(_)) => {
                // Parse response
                let resp: PluginResponse = serde_json::from_str(line.trim())
                    .map_err(|e| format!("invalid JSON from plugin '{plugin_name}': {e}"))?;

                if resp.id != request_id {
                    warn!(
                        "plugin '{}' returned mismatched id (expected {}, got {})",
                        plugin_name, request_id, resp.id
                    );
                }

                if let Some(err) = resp.error {
                    return Err(format!("plugin '{plugin_name}' error: {err}"));
                }

                Ok(resp.result.unwrap_or(Value::Null))
            }
            Ok(Err(e)) => {
                Err(format!("read error from plugin '{plugin_name}': {e}"))
            }
            Err(_elapsed) => {
                // Timeout - kill the process
                warn!("plugin '{}' timed out on action '{}', killing", plugin_name, action);
                if let Some(proc) = procs.get_mut(plugin_name) {
                    let _ = proc.child.kill().await;
                    let _ = proc.child.wait().await;
                }
                procs.remove(plugin_name);
                Err(format!("plugin '{plugin_name}' timed out on action '{action}'"))
            }
        }
    }

    /// Stop a running plugin process.
    pub async fn stop(&self, plugin_name: &str) {
        let mut procs = self.processes.lock().await;
        if let Some(mut proc) = procs.remove(plugin_name) {
            let _ = proc.child.kill().await;
            let _ = proc.child.wait().await;
            debug!("plugin '{}' stopped", plugin_name);
        }
    }

    /// Stop all running plugin processes.
    pub async fn stop_all(&self) {
        let mut procs = self.processes.lock().await;
        for (name, mut proc) in procs.drain() {
            let _ = proc.child.kill().await;
            let _ = proc.child.wait().await;
            debug!("plugin '{}' stopped", name);
        }
    }

    /// Set concurrency limit for a plugin.
    pub async fn set_concurrency(&self, plugin: &str, max: usize) {
        let mut sems = self.semaphores.lock().await;
        sems.insert(plugin.to_string(), Arc::new(Semaphore::new(max)));
    }

    /// Check if a plugin process is running.
    pub async fn is_running(&self, plugin_name: &str) -> bool {
        let procs = self.processes.lock().await;
        procs.contains_key(plugin_name)
    }

    /// Number of currently running plugin processes.
    pub async fn running_count(&self) -> usize {
        let procs = self.processes.lock().await;
        procs.len()
    }
}
