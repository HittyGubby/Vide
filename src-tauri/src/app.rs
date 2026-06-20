use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::features::network::NetworkManager;
use crate::features::player::PlayerManager;
use crate::features::plugin::PluginManager;
use crate::infra::config::AppConfig;
use crate::infra::db::Database;

/// Top-level application state shared across all Tauri commands.
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Arc<Mutex<AppConfig>>,
    pub player: Arc<Mutex<PlayerManager>>,
    pub network: Arc<Mutex<NetworkManager>>,
    /// Plugin system: loaded manifests, routing table, runtime.
    #[allow(dead_code)]
    pub plugins: Option<Arc<PluginManager>>,
}

impl AppState {
    /// Initialize all subsystems in dependency order.
    ///
    /// Order:
    /// 1. Open database
    /// 2. Load config
    /// 3. Initialize plugin system (if plugin_dir exists)
    /// 4. Create player manager
    /// 5. Create network manager
    pub async fn initialize(db_path: &Path) -> Self {
        let db = Arc::new(
            Database::open(db_path)
                .await
                .expect("Failed to open database"),
        );

        // Load config first (before wrapping in Arc/Mutex) to read plugin_dir
        let app_config = AppConfig::load(&db).await.unwrap_or_default();
        let plugin_dir = app_config.plugin_dir.clone();

        let config = Arc::new(Mutex::new(app_config));
        let player = Arc::new(Mutex::new(PlayerManager::new()));
        let network = Arc::new(Mutex::new(NetworkManager::new()));

        // Initialize plugin system if the directory exists
        let plugins = if plugin_dir.exists() && plugin_dir.is_dir() {
            let pm = PluginManager::init(&plugin_dir, &db, 4).await;
            tracing::info!(
                "Plugin system initialized: {} manifests, {} routes",
                pm.manifests.len(),
                pm.router.route_count()
            );
            Some(Arc::new(pm))
        } else {
            tracing::info!("No plugin directory found at {:?}, plugins disabled", plugin_dir);
            None
        };

        tracing::info!("AppState initialized");

        AppState {
            db,
            config,
            player,
            network,
            plugins,
        }
    }
}
