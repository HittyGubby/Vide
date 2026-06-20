use std::path::Path;

use tracing::info;

use crate::features::plugin::manifest::load_manifests;
use crate::features::plugin::router::{ProviderConfig, RoutingTable};
use crate::features::plugin::runtime::PluginRuntime;
use crate::features::plugin::types::PluginManifest;
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

        // Load saved provider config from DB
        let provider_config = load_provider_config(db).await;

        let router = RoutingTable::build(&manifests, &provider_config);
        let runtime = PluginRuntime::new(default_concurrency);

        // Apply concurrency limits from config
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
}

async fn load_provider_config(db: &Database) -> ProviderConfig {
    match db.get_setting("plugin_routing").await {
        Ok(Some(val)) => serde_json::from_value(val).unwrap_or_default(),
        _ => ProviderConfig::default(),
    }
}
