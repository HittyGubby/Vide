use std::fs;
use std::io;
use std::path::Path;

use tracing::warn;

use crate::features::plugin::types::{PluginManifest, PROTOCOL_VERSION};

/// Scan a directory for plugin manifests and load them.
///
/// Looks for manifest.json in each subdirectory of the given path. Duplicate
/// plugin names are ignored (the first directory wins) so routing keys remain
/// unambiguous.
pub fn load_manifests(plugin_dir: &Path) -> Vec<PluginManifest> {
    let mut manifests: Vec<PluginManifest> = Vec::new();

    let entries = match fs::read_dir(plugin_dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("cannot read plugin dir {:?}: {e}", plugin_dir);
            return manifests;
        }
    };

    for entry in entries.flatten() {
        let dir = entry.path();
        let manifest_path = dir.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        match load_single_manifest(&manifest_path) {
            Ok(mut manifest) => {
                if manifest.capabilities.is_empty() {
                    warn!("plugin '{}' has no capabilities, skipping", manifest.name);
                    continue;
                }
                if manifests.iter().any(|existing| existing.name == manifest.name) {
                    warn!("duplicate plugin name '{}', skipping {:?}", manifest.name, dir);
                    continue;
                }
                manifest.dir = dir;
                manifests.push(manifest);
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

    if manifest.protocol > PROTOCOL_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "plugin '{}' requires protocol {}, core supports {}",
                manifest.name, manifest.protocol, PROTOCOL_VERSION
            ),
        ));
    }
    if manifest.name.trim().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "plugin name is empty"));
    }
    if manifest.version.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("plugin '{}' has empty version", manifest.name),
        ));
    }
    if manifest.exec.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("plugin '{}' has empty exec", manifest.name),
        ));
    }
    for capability in &manifest.capabilities {
        if !capability.capability.is_supported() {
            warn!(
                "plugin '{}' declares unknown capability '{}', ignoring it",
                manifest.name, capability.capability
            );
        }
        if capability.platform.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("plugin '{}' has empty platform", manifest.name),
            ));
        }
    }

    Ok(manifest)
}
