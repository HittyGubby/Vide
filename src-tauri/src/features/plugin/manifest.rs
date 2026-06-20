use std::fs;
use std::io;
use std::path::Path;

use tracing::warn;

use crate::features::plugin::types::PluginManifest;

/// Scan a directory for plugin manifests and load them.
///
/// Looks for `manifest.json` in each subdirectory of the given path.
/// Returns all valid manifests; logs warnings for malformed ones.
pub fn load_manifests(plugin_dir: &Path) -> Vec<PluginManifest> {
    let mut manifests = Vec::new();

    let entries = match fs::read_dir(plugin_dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("cannot read plugin dir {:?}: {e}", plugin_dir);
            return manifests;
        }
    };

    for entry in entries.flatten() {
        let manifest_path = entry.path().join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        match load_single_manifest(&manifest_path) {
            Ok(m) => {
                if m.capabilities.is_empty() {
                    warn!("plugin '{}' has no capabilities, skipping", m.name);
                    continue;
                }
                manifests.push(m);
            }
            Err(e) => {
                warn!("failed to load manifest {:?}: {e}", manifest_path);
            }
        }
    }

    manifests
}

/// Load and validate a single manifest.json file.
fn load_single_manifest(path: &Path) -> io::Result<PluginManifest> {
    let content = fs::read_to_string(path)?;
    let manifest: PluginManifest =
        serde_json::from_str(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    if manifest.name.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "plugin name is empty"));
    }
    if manifest.exec.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("plugin '{}' has empty exec", manifest.name),
        ));
    }

    Ok(manifest)
}
