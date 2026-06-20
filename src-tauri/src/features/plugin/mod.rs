use std::path::Path;

use serde_json::Value;
use tracing::info;

use crate::features::plugin::manifest::load_manifests;
use crate::features::plugin::router::{ProviderConfig, RoutingTable};
use crate::features::plugin::types::PluginStatus;
use crate::features::plugin::runtime::PluginRuntime;
use crate::features::plugin::types::{PluginManifest, SearchResultItem, StreamDescriptor};
use crate::infra::db::Database;

pub mod manifest;
pub mod router;
pub mod runtime;
pub mod types;

/// Orchestrator for the entire plugin system.
///
/// Combines manifest loading, routing, and process runtime.
/// Lifetime-managed via Tauri State (Arc<PluginManager>).
pub struct PluginManager {
    pub manifests: Vec<PluginManifest>,
    pub router: RoutingTable,
    pub runtime: PluginRuntime,
}

impl PluginManager {
    /// Initialize the plugin system from a plugin directory.
    ///
    /// 1. Load manifests from disk
    /// 2. Load saved routing config from DB
    /// 3. Build routing table
    /// 4. Create runtime
    pub async fn init(plugin_dir: &Path, db: &Database, default_concurrency: usize) -> Self {
        let manifests = load_manifests(plugin_dir);
        let provider_config = load_provider_config(db).await;
        let router = RoutingTable::build(&manifests, &provider_config);
        let runtime = PluginRuntime::new(default_concurrency);

        for (plugin, max) in &provider_config.concurrency {
            runtime.set_concurrency(plugin, *max as usize).await;
        }

        info!(
            "plugin system initialized: {} manifests, {} routes",
            manifests.len(),
            router.route_count()
        );

        PluginManager {
            manifests,
            router,
            runtime,
        }
    }

    /// Start all plugin processes.
    pub async fn start_all(&self) {
        for manifest in &self.manifests {
            if let Err(e) = self.runtime.ensure_running(manifest).await {
                tracing::warn!("failed to start plugin '{}': {}", manifest.name, e);
            }
        }
    }

    /// Stop all plugin processes.
    pub async fn stop_all(&self) {
        self.runtime.stop_all().await;
    }

    /// Persist current routing config to the database.
    pub async fn save_routing_config(&self, db: &Database) {
        let config = self.router.to_config();
        if let Ok(val) = serde_json::to_value(&config) {
            let _ = db.update_setting("plugin_routing", &val).await;
        }
    }

    // ── Helpers ─────────────────────────────────────────

    fn find_manifest(&self, name: &str) -> Option<&PluginManifest> {
        self.manifests.iter().find(|m| m.name == name)
    }

    /// Execute a plugin action with automatic provider routing, process
    /// lifecycle, timeout, and fallback.
    async fn execute(
        &self,
        platform: &str,
        capability: &str,
        params: Value,
    ) -> Result<Value, String> {
        let mut attempts = 0;
        loop {
            let entry = self
                .router
                .get_active(platform, capability)
                .ok_or_else(|| {
                    format!("no {capability} provider for platform '{platform}'")
                })?;

            let manifest = self
                .find_manifest(&entry.plugin_name)
                .ok_or_else(|| format!("manifest not found for '{}'", entry.plugin_name))?;

            self.runtime.ensure_running(manifest).await?;

            match self
                .runtime
                .send_request(&entry.plugin_name, capability, params.clone())
                .await
            {
                Ok(value) => {
                    self.router.set_status(
                        platform,
                        capability,
                        &entry.plugin_name,
                        PluginStatus::Running,
                    );
                    return Ok(value);
                }
                Err(e) => {
                    self.router.set_status(
                        platform,
                        capability,
                        &entry.plugin_name,
                        PluginStatus::Error(e.clone()),
                    );
                    // Try fallback if available
                    if self.router.fallback(platform, capability).is_some() {
                        attempts += 1;
                        tracing::warn!(
                            "plugin '{}' failed for {platform}/{capability}, trying fallback ({attempts})",
                            entry.plugin_name
                        );
                        continue;
                    }
                    return Err(format!(
                        "all providers failed for {platform}/{capability}: {e}"
                    ));
                }
            }
        }
    }

    // ── Public API ──────────────────────────────────────

    /// Search for songs on a platform.
    pub async fn search(
        &self,
        platform: &str,
        query: &str,
    ) -> Result<Vec<SearchResultItem>, String> {
        let params = serde_json::json!({ "q": query, "platform": platform });
        let value = self.execute(platform, "search", params).await?;
        serde_json::from_value(value).map_err(|e| format!("invalid search result: {e}"))
    }

    /// Resolve full metadata for a known platform song.
    pub async fn resolve(
        &self,
        platform: &str,
        id: &str,
    ) -> Result<SearchResultItem, String> {
        let params = serde_json::json!({ "platform": platform, "id": id });
        let value = self.execute(platform, "resolve", params).await?;
        serde_json::from_value(value).map_err(|e| format!("invalid resolve result: {e}"))
    }

    /// Get a playable stream descriptor for a platform song.
    pub async fn stream(
        &self,
        platform: &str,
        id: &str,
        song_id: &str,
    ) -> Result<StreamDescriptor, String> {
        let params = serde_json::json!({
            "platform": platform,
            "id": id,
            "song_id": song_id,
        });
        let value = self.execute(platform, "stream", params).await?;
        let mut desc: StreamDescriptor =
            serde_json::from_value(value).map_err(|e| format!("invalid stream descriptor: {e}"))?;
        // Ensure StreamDescriptor carries the song_id as its id
        match &mut desc {
            StreamDescriptor::UrlStream { id: ref mut did, .. }
            | StreamDescriptor::Pipe { id: ref mut did, .. } => {
                if did.is_empty() {
                    *did = song_id.to_string();
                }
            }
        }
        Ok(desc)
    }
}

async fn load_provider_config(db: &Database) -> ProviderConfig {
    match db.get_setting("plugin_routing").await {
        Ok(Some(val)) => serde_json::from_value(val).unwrap_or_default(),
        _ => ProviderConfig::default(),
    }
}
