use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::features::plugin::types::PluginManifest;

/// Persisted user configuration for the plugin system.
///
/// Stored as one JSON value under the plugin_config setting.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginConfig {
    /// Per-plugin user settings. A missing entry means enabled with defaults.
    #[serde(default)]
    pub plugins: HashMap<String, PluginSettings>,
    /// Full ordered routes: platform -> capability -> plugin names.
    ///
    /// A missing route means "use the manifest-discovered default order".
    #[serde(default)]
    pub routes: HashMap<String, HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSettings {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Full default argv override. Empty means inherit manifest.exec.
    #[serde(default)]
    pub argv: Vec<String>,
    /// Per-platform argv overrides. Empty values inherit the default argv.
    #[serde(default)]
    pub platform_argv: HashMap<String, Vec<String>>,
    /// Per-route argv overrides, keyed by "platform/capability".
    #[serde(default)]
    pub route_argv: HashMap<String, Vec<String>>,
    /// Optional per-plugin concurrency override.
    #[serde(default)]
    pub concurrency: Option<u32>,
}

impl Default for PluginSettings {
    fn default() -> Self {
        PluginSettings {
            enabled: true,
            argv: Vec::new(),
            platform_argv: HashMap::new(),
            route_argv: HashMap::new(),
            concurrency: None,
        }
    }
}

fn default_enabled() -> bool {
    true
}

impl PluginConfig {
    pub fn enabled(&self, plugin: &str) -> bool {
        self.plugins
            .get(plugin)
            .map(|settings| settings.enabled)
            .unwrap_or(true)
    }

    pub fn settings(&self, plugin: &str) -> PluginSettings {
        self.plugins.get(plugin).cloned().unwrap_or_default()
    }

    pub fn settings_mut(&mut self, plugin: &str) -> &mut PluginSettings {
        self.plugins.entry(plugin.to_string()).or_default()
    }

    /// Resolve the full argv for one invocation:
    /// route override > platform override > plugin override > manifest.exec.
    pub fn effective_argv(
        &self,
        manifest: &PluginManifest,
        platform: &str,
        capability: &str,
    ) -> Vec<String> {
        if let Some(settings) = self.plugins.get(&manifest.name) {
            let route_key = format!("{platform}/{capability}");
            if let Some(argv) = settings
                .route_argv
                .get(&route_key)
                .filter(|argv| !argv.is_empty())
            {
                return argv.clone();
            }
            if let Some(argv) = settings
                .platform_argv
                .get(platform)
                .filter(|argv| !argv.is_empty())
            {
                return argv.clone();
            }
            if !settings.argv.is_empty() {
                return settings.argv.clone();
            }
        }
        manifest.exec.clone()
    }

    pub fn set_route(&mut self, platform: &str, capability: &str, providers: Vec<String>) {
        self.routes
            .entry(platform.to_string())
            .or_default()
            .insert(capability.to_string(), providers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::plugin::types::{Capability, PlatformCapability};
    use std::path::PathBuf;

    fn manifest() -> PluginManifest {
        PluginManifest {
            protocol: 1,
            name: "demo".to_string(),
            version: "1".to_string(),
            description: None,
            exec: vec!["python3".to_string(), "adapter.py".to_string()],
            capabilities: vec![PlatformCapability {
                capability: Capability::Search,
                platform: "youtube".to_string(),
            }],
            dir: PathBuf::new(),
        }
    }

    #[test]
    fn argv_precedence_route_platform_plugin_manifest() {
        let manifest = manifest();
        let mut config = PluginConfig::default();
        assert_eq!(
            config.effective_argv(&manifest, "youtube", "search"),
            manifest.exec
        );

        config.settings_mut("demo").argv = vec!["plugin".to_string()];
        assert_eq!(
            config.effective_argv(&manifest, "youtube", "search"),
            vec!["plugin".to_string()]
        );

        config
            .settings_mut("demo")
            .platform_argv
            .insert("youtube".to_string(), vec!["platform".to_string()]);
        assert_eq!(
            config.effective_argv(&manifest, "youtube", "search"),
            vec!["platform".to_string()]
        );

        config
            .settings_mut("demo")
            .route_argv
            .insert("youtube/search".to_string(), vec!["route".to_string()]);
        assert_eq!(
            config.effective_argv(&manifest, "youtube", "search"),
            vec!["route".to_string()]
        );
    }

    #[test]
    fn empty_overrides_inherit() {
        let manifest = manifest();
        let mut config = PluginConfig::default();
        config.settings_mut("demo").argv = Vec::new();
        config
            .settings_mut("demo")
            .platform_argv
            .insert("youtube".to_string(), Vec::new());
        assert_eq!(
            config.effective_argv(&manifest, "youtube", "search"),
            manifest.exec
        );
    }
}
