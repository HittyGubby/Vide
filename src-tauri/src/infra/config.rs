use sea_orm::DbErr;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::infra::db::Database;

/// Application configuration, stored in the settings table.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub library_roots: Vec<PathBuf>,
    pub exclude_roots: Vec<PathBuf>,
    pub plugin_dir: PathBuf,
    pub concurrency_limits: HashMap<String, u32>,
}

/// Platform-aware default plugin directory.
///
/// Linux/macOS use the user config directory; Windows uses APPDATA. We keep
/// the application name segment so the location is stable across launches.
pub fn default_plugin_dir() -> PathBuf {
    dirs::config_dir()
        .map(|dir| dir.join("vide").join("plugins"))
        .unwrap_or_else(|| PathBuf::from("plugins"))
}

/// Resolve a configured plugin directory.
///
/// Absolute paths are used as-is. A relative path is first looked up in the
/// current working directory and then by walking up from cwd/executable (this
/// keeps the repository plugins directory working when Tauri runs from
/// src-tauri). Finally it resolves under the platform config directory.
pub fn resolve_plugin_dir(configured: &Path) -> PathBuf {
    if configured.is_absolute() {
        return configured.to_path_buf();
    }
    if configured.exists() {
        return configured.to_path_buf();
    }

    let mut starts: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            starts.push(parent.to_path_buf());
        }
    }

    for start in starts {
        let mut dir = start;
        for _ in 0..8 {
            let candidate = dir.join(configured);
            let has_manifest = candidate.join("manifest.json").exists()
                || candidate
                    .read_dir()
                    .map(|mut entries| {
                        entries.any(|entry| {
                            entry
                                .map(|entry| entry.path().join("manifest.json").exists())
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);
            if has_manifest {
                return candidate;
            }
            if !dir.pop() {
                break;
            }
        }
    }

    dirs::config_dir()
        .map(|dir| dir.join("vide").join(configured))
        .unwrap_or_else(default_plugin_dir)
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut limits = HashMap::new();
        limits.insert("yt-dlp".to_string(), 2);
        Self {
            library_roots: Vec::new(),
            exclude_roots: Vec::new(),
            plugin_dir: default_plugin_dir(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_repository_plugins_from_cwd_or_parent() {
        if !Path::new("../plugins").exists() && !Path::new("plugins").exists() {
            return;
        }
        let resolved = resolve_plugin_dir(Path::new("plugins"));
        assert!(resolved.ends_with("plugins"), "{resolved:?}");
        assert!(resolved.join("ytdlp-adapter/manifest.json").exists());
    }
}
