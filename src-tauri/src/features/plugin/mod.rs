use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::info;

use crate::features::plugin::config::PluginConfig;
use crate::features::plugin::manifest::load_manifests;
use crate::features::plugin::router::RoutingTable;
use crate::features::plugin::runtime::PluginRuntime;
use crate::features::plugin::types::{
    Lyrics, PluginManifest, PluginStatus, PluginStream, StreamDescriptor, Track,
};
use crate::infra::db::Database;

pub mod commands;
pub mod config;
pub mod manifest;
pub mod router;
pub mod runtime;
pub mod types;

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub dir: String,
    pub enabled: bool,
    pub capabilities: Vec<CapabilityView>,
    pub default_argv: Vec<String>,
    pub argv: Vec<String>,
    pub platform_argv: HashMap<String, Vec<String>>,
    pub route_argv: HashMap<String, Vec<String>>,
    pub concurrency: Option<u32>,
    pub status: PluginStatus,
    pub last_stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityView {
    pub capability: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteProviderView {
    pub plugin: String,
    pub enabled: bool,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteView {
    pub platform: String,
    pub capability: String,
    pub providers: Vec<RouteProviderView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginState {
    pub plugin_dir: String,
    pub plugins: Vec<PluginInfo>,
    pub routes: Vec<RouteView>,
}

/// Orchestrator for the plugin system.
pub struct PluginManager {
    manifests: RwLock<Vec<PluginManifest>>,
    plugin_dir: RwLock<PathBuf>,
    pub config: Mutex<PluginConfig>,
    pub router: Mutex<RoutingTable>,
    pub runtime: PluginRuntime,
    default_concurrency: usize,
}

impl PluginManager {
    pub async fn init(plugin_dir: &Path, db: &Database, default_concurrency: usize) -> Self {
        let manifests = load_manifests(plugin_dir);
        let config = load_plugin_config(db).await;
        let router = RoutingTable::build(&manifests, &config);
        let runtime = PluginRuntime::new(default_concurrency);

        for manifest in &manifests {
            let max = config
                .plugins
                .get(&manifest.name)
                .and_then(|settings| settings.concurrency)
                .unwrap_or(default_concurrency as u32)
                .max(1) as usize;
            runtime.set_concurrency(&manifest.name, max).await;
        }

        info!(
            "plugin system initialized: {} manifests, {} routes",
            manifests.len(),
            router.route_count()
        );

        PluginManager {
            manifests: RwLock::new(manifests),
            plugin_dir: RwLock::new(plugin_dir.to_path_buf()),
            config: Mutex::new(config),
            router: Mutex::new(router),
            runtime,
            default_concurrency,
        }
    }

    pub fn manifest_count(&self) -> usize {
        self.manifests.read().map(|guard| guard.len()).unwrap_or(0)
    }

    pub async fn config_snapshot(&self) -> PluginConfig {
        self.config.lock().await.clone()
    }

    fn manifest_list(&self) -> Vec<PluginManifest> {
        self.manifests
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn find_manifest(&self, name: &str) -> Option<PluginManifest> {
        self.manifests
            .read()
            .ok()?
            .iter()
            .find(|manifest| manifest.name == name)
            .cloned()
    }

    async fn rebuild_router(&self) {
        let config = self.config.lock().await.clone();
        let manifests = self.manifest_list();
        let router = RoutingTable::build(&manifests, &config);
        *self.router.lock().await = router;
    }

    /// Rescan the plugin directory and rebuild routes without restarting.
    pub async fn reload(&self) -> Result<(), String> {
        let plugin_dir = self
            .plugin_dir
            .read()
            .map(|path| path.clone())
            .unwrap_or_default();
        let manifests = load_manifests(&plugin_dir);
        {
            let mut guard = self
                .manifests
                .write()
                .map_err(|_| "plugin manifest lock poisoned".to_string())?;
            *guard = manifests;
        }
        self.rebuild_router().await;

        let config = self.config.lock().await.clone();
        for manifest in self.manifest_list() {
            let max = config
                .plugins
                .get(&manifest.name)
                .and_then(|settings| settings.concurrency)
                .unwrap_or(self.default_concurrency as u32)
                .max(1) as usize;
            self.runtime.set_concurrency(&manifest.name, max).await;
        }
        Ok(())
    }

    /// Point the manager at a freshly resolved plugin directory and rescan.
    /// This lets the settings page recover without a restart when the
    /// configured path was initially resolved incorrectly.
    pub async fn reload_from(&self, plugin_dir: PathBuf) -> Result<(), String> {
        {
            let mut guard = self
                .plugin_dir
                .write()
                .map_err(|_| "plugin directory lock poisoned".to_string())?;
            *guard = plugin_dir;
        }
        self.reload().await
    }

    async fn execute(
        &self,
        platform: &str,
        capability: &str,
        params: Value,
    ) -> Result<Value, String> {
        let entries = {
            self.router
                .lock()
                .await
                .get_all(platform, capability)
                .ok_or_else(|| format!("no {capability} provider for platform '{platform}'"))?
        };

        if entries.is_empty() {
            return Err(format!(
                "no {capability} provider for platform '{platform}'"
            ));
        }

        let mut last_error = String::new();
        for entry in entries {
            let manifest = match self.find_manifest(&entry.plugin_name) {
                Some(manifest) => manifest,
                None => {
                    last_error = format!("manifest not found for '{}'", entry.plugin_name);
                    continue;
                }
            };

            let (enabled, argv) = {
                let config = self.config.lock().await;
                (
                    config.enabled(&entry.plugin_name),
                    config.effective_argv(&manifest, platform, capability),
                )
            };
            if !enabled {
                last_error = format!("plugin '{}' is disabled", entry.plugin_name);
                continue;
            }

            match self
                .runtime
                .invoke(&manifest, &argv, capability, params.clone())
                .await
            {
                Ok(invocation) => return Ok(invocation.result),
                Err(error) => last_error = error,
            }
        }

        Err(format!(
            "all {capability} providers failed for platform '{platform}': {last_error}"
        ))
    }

    pub async fn search(
        &self,
        platform: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<Track>, String> {
        let params = serde_json::json!({
            "query": query,
            "platform": platform,
            "limit": limit.clamp(1, 100),
        });
        let value = self.execute(platform, "search", params).await?;
        let mut tracks: Vec<Track> = serde_json::from_value(value)
            .map_err(|e| format!("invalid search result: {e}"))?;
        for track in &mut tracks {
            if track.platform.is_empty() {
                track.platform = platform.to_string();
            }
        }
        Ok(tracks)
    }

    pub async fn resolve(&self, platform: &str, source_id: &str) -> Result<Track, String> {
        let params = serde_json::json!({ "platform": platform, "source_id": source_id });
        let value = self.execute(platform, "resolve", params).await?;
        let mut track: Track = serde_json::from_value(value)
            .map_err(|e| format!("invalid resolve result: {e}"))?;
        if track.platform.is_empty() {
            track.platform = platform.to_string();
        }
        if track.source_id.is_empty() {
            track.source_id = source_id.to_string();
        }
        Ok(track)
    }

    pub async fn playlist(&self, url: &str) -> Result<Vec<Track>, String> {
        let providers = self.router.lock().await.get_all_capability("playlist");
        if providers.is_empty() {
            return Err("no playlist provider registered".to_string());
        }

        let params = serde_json::json!({ "url": url });
        let mut last_error = String::new();
        for (platform, entry) in providers {
            let manifest = match self.find_manifest(&entry.plugin_name) {
                Some(manifest) => manifest,
                None => continue,
            };
            let (enabled, argv) = {
                let config = self.config.lock().await;
                (
                    config.enabled(&entry.plugin_name),
                    config.effective_argv(&manifest, &platform, "playlist"),
                )
            };
            if !enabled {
                last_error = format!("plugin '{}' is disabled", entry.plugin_name);
                continue;
            }

            match self
                .runtime
                .invoke(&manifest, &argv, "playlist", params.clone())
                .await
            {
                Ok(invocation) => match serde_json::from_value::<Vec<Track>>(invocation.result) {
                    Ok(tracks) if !tracks.is_empty() => return Ok(tracks),
                    Ok(_) => last_error = format!("plugin '{}' returned no entries", entry.plugin_name),
                    Err(error) => {
                        last_error = format!("invalid playlist result from '{}': {error}", entry.plugin_name)
                    }
                },
                Err(error) => last_error = error,
            }
        }

        Err(format!("could not parse URL: {last_error}"))
    }

    pub async fn stream(
        &self,
        platform: &str,
        source_id: &str,
        song_id: &str,
    ) -> Result<StreamDescriptor, String> {
        let params = serde_json::json!({ "platform": platform, "source_id": source_id });
        let value = self.execute(platform, "stream", params).await?;
        let plugin_stream: PluginStream = serde_json::from_value(value)
            .map_err(|e| format!("invalid stream descriptor: {e}"))?;
        match plugin_stream {
            PluginStream::Pipe { cmd } => {
                if cmd.is_empty() {
                    return Err("stream descriptor has empty cmd".to_string());
                }
                Ok(StreamDescriptor::Pipe {
                    id: song_id.to_string(),
                    cmd,
                })
            }
        }
    }

    pub async fn lyrics(&self, platform: &str, source_id: &str) -> Result<Option<Lyrics>, String> {
        let params = serde_json::json!({ "platform": platform, "source_id": source_id });
        let value = self.execute(platform, "lyrics", params).await?;
        if value.is_null() {
            return Ok(None);
        }
        serde_json::from_value(value)
            .map(Some)
            .map_err(|e| format!("invalid lyrics result: {e}"))
    }

    pub async fn state(&self) -> PluginState {
        let config = self.config.lock().await.clone();
        let manifests = self.manifest_list();
        let mut plugins = Vec::with_capacity(manifests.len());
        for manifest in &manifests {
            let settings = config.settings(&manifest.name);
            plugins.push(PluginInfo {
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                description: manifest.description.clone(),
                dir: manifest.dir.to_string_lossy().to_string(),
                enabled: settings.enabled,
                capabilities: manifest
                    .capabilities
                    .iter()
                    .filter(|capability| capability.capability.is_supported())
                    .map(|capability| CapabilityView {
                        capability: capability.capability.as_str().to_string(),
                        platform: capability.platform.clone(),
                    })
                    .collect(),
                default_argv: manifest.exec.clone(),
                argv: settings.argv,
                platform_argv: settings.platform_argv,
                route_argv: settings.route_argv,
                concurrency: settings.concurrency,
                status: self.runtime.status(&manifest.name).await,
                last_stderr: self.runtime.last_stderr(&manifest.name).await,
            });
        }

        let snapshot = self.router.lock().await.snapshot();
        let mut routes = Vec::with_capacity(snapshot.len());
        for route in snapshot {
            let mut providers = Vec::with_capacity(route.providers.len());
            for name in route.providers {
                let manifest = self.find_manifest(&name);
                let (enabled, argv) = match manifest {
                    Some(manifest) => (
                        config.enabled(&name),
                        config.effective_argv(&manifest, &route.platform, &route.capability),
                    ),
                    None => (false, Vec::new()),
                };
                providers.push(RouteProviderView {
                    plugin: name,
                    enabled,
                    argv,
                });
            }
            routes.push(RouteView {
                platform: route.platform,
                capability: route.capability,
                providers,
            });
        }

        let plugin_dir = self
            .plugin_dir
            .read()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();
        PluginState {
            plugin_dir,
            plugins,
            routes,
        }
    }

    pub async fn set_plugin_enabled(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<PluginConfig, String> {
        self.ensure_manifest(name)?;
        let config = {
            let mut config = self.config.lock().await;
            config.settings_mut(name).enabled = enabled;
            config.clone()
        };
        Ok(config)
    }

    pub async fn set_plugin_argv(
        &self,
        name: &str,
        argv: Vec<String>,
    ) -> Result<PluginConfig, String> {
        self.ensure_manifest(name)?;
        let config = {
            let mut config = self.config.lock().await;
            config.settings_mut(name).argv = argv;
            config.clone()
        };
        Ok(config)
    }

    pub async fn set_platform_argv(
        &self,
        name: &str,
        platform: &str,
        argv: Vec<String>,
    ) -> Result<PluginConfig, String> {
        self.ensure_manifest(name)?;
        let config = {
            let mut config = self.config.lock().await;
            let settings = config.settings_mut(name);
            if argv.is_empty() {
                settings.platform_argv.remove(platform);
            } else {
                settings.platform_argv.insert(platform.to_string(), argv);
            }
            config.clone()
        };
        Ok(config)
    }

    pub async fn set_route_argv(
        &self,
        name: &str,
        platform: &str,
        capability: &str,
        argv: Vec<String>,
    ) -> Result<PluginConfig, String> {
        self.ensure_manifest(name)?;
        let key = format!("{platform}/{capability}");
        let config = {
            let mut config = self.config.lock().await;
            let settings = config.settings_mut(name);
            if argv.is_empty() {
                settings.route_argv.remove(&key);
            } else {
                settings.route_argv.insert(key, argv);
            }
            config.clone()
        };
        Ok(config)
    }

    pub async fn set_plugin_concurrency(
        &self,
        name: &str,
        concurrency: Option<u32>,
    ) -> Result<PluginConfig, String> {
        self.ensure_manifest(name)?;
        let config = {
            let mut config = self.config.lock().await;
            config.settings_mut(name).concurrency = concurrency;
            config.clone()
        };
        self.runtime
            .set_concurrency(
                name,
                concurrency.unwrap_or(self.default_concurrency as u32).max(1) as usize,
            )
            .await;
        Ok(config)
    }

    pub async fn set_route_providers(
        &self,
        platform: &str,
        capability: &str,
        providers: Vec<String>,
    ) -> Result<PluginConfig, String> {
        let mut clean: Vec<String> = Vec::new();
        for provider in providers {
            if clean.contains(&provider) {
                continue;
            }
            let manifest = self
                .find_manifest(&provider)
                .ok_or_else(|| format!("plugin '{provider}' is not installed"))?;
            let offers = manifest.capabilities.iter().any(|cap| {
                cap.capability.is_supported()
                    && cap.platform == platform
                    && cap.capability.as_str() == capability
            });
            if !offers {
                return Err(format!(
                    "plugin '{provider}' does not provide {platform}/{capability}"
                ));
            }
            clean.push(provider);
        }

        let config = {
            let mut config = self.config.lock().await;
            config.set_route(platform, capability, clean);
            config.clone()
        };
        self.rebuild_router().await;
        Ok(config)
    }

    fn ensure_manifest(&self, name: &str) -> Result<(), String> {
        if self.find_manifest(name).is_some() {
            Ok(())
        } else {
            Err(format!("plugin '{name}' is not installed"))
        }
    }

    pub async fn plugin_status(&self, name: &str) -> PluginStatus {
        self.runtime.status(name).await
    }

    pub async fn plugin_last_stderr(&self, name: &str) -> Option<String> {
        self.runtime.last_stderr(name).await
    }
}

async fn load_plugin_config(db: &Database) -> PluginConfig {
    match db.get_setting("plugin_config").await {
        Ok(Some(value)) => serde_json::from_value(value).unwrap_or_default(),
        _ => PluginConfig::default(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::Path;

    fn search_script(title: &str) -> String {
        format!(
            r#"read line
id=$(printf '%s' "$line" | awk -F'"' '{{print $4}}')
printf '{{"id":"%s","result":[{{"platform":"youtube","source_id":"x","title":"{title}"}}]}}\n' "$id"
"#
        )
    }

    fn write_plugin(root: &Path, name: &str, title: &str) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "protocol": 1,
            "name": name,
            "version": "1.0.0",
            "exec": ["sh", "-c", search_script(title)],
            "capabilities": [{"type": "search", "platform": "youtube"}]
        });
        std::fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn route_order_disable_and_argv_overrides_are_applied() {
        let root = std::env::temp_dir().join(format!(
            "vide-plugin-manager-{}",
            std::process::id()
        ));
        write_plugin(&root, "alpha", "alpha");
        write_plugin(&root, "beta", "beta");

        let db = Database::open_in_memory().await.unwrap();
        let manager = PluginManager::init(&root, &db, 2).await;

        manager
            .set_route_providers(
                "youtube",
                "search",
                vec!["beta".to_string(), "alpha".to_string()],
            )
            .await
            .unwrap();
        let tracks = manager.search("youtube", "hello", 20).await.unwrap();
        assert_eq!(tracks[0].title, "beta");

        manager.set_plugin_enabled("beta", false).await.unwrap();
        let tracks = manager.search("youtube", "hello", 20).await.unwrap();
        assert_eq!(tracks[0].title, "alpha");

        manager.set_plugin_enabled("beta", true).await.unwrap();
        manager
            .set_route_argv(
                "beta",
                "youtube",
                "search",
                vec!["sh".to_string(), "-c".to_string(), search_script("route")],
            )
            .await
            .unwrap();
        let tracks = manager.search("youtube", "hello", 20).await.unwrap();
        assert_eq!(tracks[0].title, "route");

        manager
            .set_platform_argv(
                "beta",
                "youtube",
                vec!["sh".to_string(), "-c".to_string(), search_script("platform")],
            )
            .await
            .unwrap();
        // Route override still wins over platform override.
        let tracks = manager.search("youtube", "hello", 20).await.unwrap();
        assert_eq!(tracks[0].title, "route");

        let state = manager.state().await;
        let route = state
            .routes
            .iter()
            .find(|route| route.platform == "youtube" && route.capability == "search")
            .unwrap();
        assert_eq!(route.providers[0].plugin, "beta");
        assert!(route.providers[0].enabled);

        // Hot reload picks up a newly installed plugin directory.
        write_plugin(&root, "gamma", "gamma");
        manager.reload().await.unwrap();
        let state = manager.state().await;
        assert_eq!(state.plugins.len(), 3);
        assert!(state.plugins.iter().any(|plugin| plugin.name == "gamma"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
