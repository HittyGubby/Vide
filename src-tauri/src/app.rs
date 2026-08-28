use std::path::{Path, PathBuf};
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
        let plugin_dir = resolve_plugin_dir(&app_config.plugin_dir);

        let config = Arc::new(Mutex::new(app_config));
        let network = Arc::new(Mutex::new(NetworkManager::new()));

        // Initialize plugin system if the directory exists (before the player,
        // which needs it to resolve online streams during auto-advance).
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

        // The player owns the queue/sequencing and resolves streams on its own.
        let player = Arc::new(Mutex::new(PlayerManager::new(Some(db.clone()), plugins.clone())));

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

/// Resolve the plugin directory to an absolute path that actually exists.
///
/// The configured value may be relative (SPEC default is `"plugins"`). In a
/// packaged app the working directory is not the project root, so a relative
/// `"plugins"` would resolve to a non-existent path and silently disable the
/// whole plugin system. To keep plugins working in both dev and packaged
/// builds we walk up from the executable's directory looking for a directory
/// named like the configured one that contains at least one `manifest.json`.
fn resolve_plugin_dir(configured: &Path) -> PathBuf {
    // Absolute paths are used as-is.
    if configured.is_absolute() {
        return configured.to_path_buf();
    }

    // If it exists relative to the current working directory, use it.
    if configured.exists() {
        return configured.to_path_buf();
    }

    // Otherwise walk up from the executable's directory looking for a folder
    // with a `manifest.json` (a real plugin directory).
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        for _ in 0..6 {
            let candidate = dir.join(configured);
            if candidate.join("manifest.json").exists()
                || candidate.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false)
            {
                return candidate;
            }
            if !dir.pop() {
                break;
            }
        }
    }

    // Fall back to the configured relative path.
    configured.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_absolute_passthrough() {
        let p = PathBuf::from("/abs/plugins");
        assert_eq!(resolve_plugin_dir(&p), p);
    }

    #[test]
    fn resolve_relative_finders_exists_dir() {
        // A relative dir that exists from cwd is used directly.
        let d = std::env::temp_dir().join("vide_plugin_test");
        let _ = std::fs::create_dir_all(&d);
        std::fs::write(d.join("manifest.json"), "{}").unwrap();
        let resolved = resolve_plugin_dir(Path::new("vide_plugin_test"));
        // The candidate must be an existing directory (either cwd-relative or
        // found by walking up — either way it should not be the bare name).
        assert!(resolved.as_os_str().to_string_lossy().contains("vide_plugin_test"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
