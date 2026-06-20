use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::features::network::NetworkManager;
use crate::features::player::PlayerManager;
use crate::infra::config::AppConfig;
use crate::infra::db::Database;

/// Top-level application state shared across all Tauri commands.
///
/// Wrapped in `Arc` / `Mutex` for thread-safe access from Tauri's
/// async command handlers.
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Arc<Mutex<AppConfig>>,
    pub player: Arc<Mutex<PlayerManager>>,
    pub network: Arc<Mutex<NetworkManager>>,
}

impl AppState {
    /// Initialize all subsystems in dependency order.
    ///
    /// Order:
    /// 1. Open database (creates schema)
    /// 2. Load config from database
    /// 3. Create player manager
    /// 4. Create network manager (not yet started)
    pub async fn initialize(db_path: &Path) -> Self {
        let db = Arc::new(
            Database::open(db_path)
                .await
                .expect("Failed to open database"),
        );

        let config = Arc::new(Mutex::new(
            AppConfig::load(&db).await.unwrap_or_default(),
        ));

        let player = Arc::new(Mutex::new(PlayerManager::new()));
        let network = Arc::new(Mutex::new(NetworkManager::new()));

        tracing::info!("AppState initialized");

        AppState {
            db,
            config,
            player,
            network,
        }
    }
}
