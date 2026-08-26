use sea_orm::DbErr;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::infra::db::Database;

/// Application configuration, stored in the settings table.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub library_roots: Vec<PathBuf>,
    pub exclude_roots: Vec<PathBuf>,
    pub plugin_dir: PathBuf,
    pub concurrency_limits: HashMap<String, u32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut limits = HashMap::new();
        limits.insert("yt-dlp".to_string(), 2);
        Self {
            library_roots: Vec::new(),
            exclude_roots: Vec::new(),
            plugin_dir: PathBuf::from("plugins"),
            concurrency_limits: limits,
        }
    }
}

impl AppConfig {
    /// Load configuration from the database, falling back to defaults.
    pub async fn load(db: &Database) -> Result<Self, DbErr> {
        let mut config = AppConfig::default();

        if let Some(val) = db.get_setting("library_roots").await? {
            if let Ok(roots) = serde_json::from_value::<Vec<String>>(val) {
                config.library_roots = roots.into_iter().map(PathBuf::from).collect();
            }
        }

        if let Some(val) = db.get_setting("exclude_roots").await? {
            if let Ok(roots) = serde_json::from_value::<Vec<String>>(val) {
                config.exclude_roots = roots.into_iter().map(PathBuf::from).collect();
            }
        }

        if let Some(val) = db.get_setting("plugin_dir").await? {
            if let Some(dir) = val.as_str() {
                config.plugin_dir = PathBuf::from(dir);
            }
        }

        if let Some(val) = db.get_setting("concurrency_limits").await? {
            if let Ok(limits) = serde_json::from_value::<HashMap<String, u32>>(val) {
                config.concurrency_limits = limits;
            }
        }

        Ok(config)
    }

    /// Save the current configuration to the database.
    pub async fn save(&self, db: &Database) -> Result<(), DbErr> {
        let roots: Vec<String> = self
            .library_roots
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        db.update_setting("library_roots", &serde_json::json!(roots))
            .await?;

        let exclusions: Vec<String> = self
            .exclude_roots
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        db.update_setting("exclude_roots", &serde_json::json!(exclusions))
            .await?;

        db.update_setting(
            "plugin_dir",
            &serde_json::json!(self.plugin_dir.to_string_lossy().as_ref()),
        )
        .await?;

        db.update_setting("concurrency_limits", &serde_json::json!(self.concurrency_limits))
            .await?;

        Ok(())
    }
}
