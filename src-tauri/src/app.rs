use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::features::network::NetworkManager;
use crate::features::player::PlayerManager;
use crate::features::plugin::PluginManager;
use crate::infra::config::{resolve_plugin_dir, AppConfig};
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
        let plugin_dir = resolve_plugin_dir(&app_config.plugin_dir);
        if let Err(e) = std::fs::create_dir_all(&plugin_dir) {
            tracing::warn!("cannot create plugin dir {:?}: {e}", plugin_dir);
        }

        let config = Arc::new(Mutex::new(app_config));
        let network = Arc::new(Mutex::new(NetworkManager::new()));

        // Initialize plugin system if the directory exists (before the player,
        // which needs it to resolve online streams during auto-advance).
        let plugins = if plugin_dir.exists() && plugin_dir.is_dir() {
            let pm = PluginManager::init(&plugin_dir, &db, 4).await;
            tracing::info!(
                "Plugin system initialized: {} manifests, {} routes",
                pm.manifest_count(),
                pm.router.lock().await.route_count()
            );
            Some(Arc::new(pm))
        } else {
            tracing::info!("No plugin directory found at {:?}, plugins disabled", plugin_dir);
            None
        };

        // The player owns the queue/sequencing and resolves streams on its own.
        // Restore the previous queue before the frontend can press next/prev:
        // the startup auto-resume calls play_song without setting a queue, so
        // without this next/prev used to be no-ops until a manual list click
        // re-registered the queue.
        let data_dir = db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let mut player_manager =
            PlayerManager::new(Some(db.clone()), plugins.clone(), data_dir);
        player_manager.restore_queue().await;
        let player = Arc::new(Mutex::new(player_manager));

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


