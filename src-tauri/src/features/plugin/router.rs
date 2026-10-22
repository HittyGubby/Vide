use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

use crate::features::plugin::config::PluginConfig;
use crate::features::plugin::types::PluginManifest;

/// A (platform, capability_type) pair used as a routing key.
pub type RouteKey = (String, String);

/// A single provider entry in the routing table.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderEntry {
    pub plugin_name: String,
}

/// Serializable snapshot of one route, used by the settings UI.
#[derive(Debug, Clone, Serialize)]
pub struct RouteProviders {
    pub platform: String,
    pub capability: String,
    pub providers: Vec<String>,
}

/// Runtime routing table: maps (platform, capability) to an ordered provider
/// list. The first provider is the active one; the rest are fallbacks.
#[derive(Debug)]
pub struct RoutingTable {
    routes: Mutex<HashMap<RouteKey, Vec<ProviderEntry>>>,
}

impl RoutingTable {
    /// Build routes from manifests, then apply the user's complete ordered
    /// overrides. Routes without an override keep the manifest-discovered
    /// order.
    pub fn build(manifests: &[PluginManifest], config: &PluginConfig) -> Self {
        let mut routes: HashMap<RouteKey, Vec<ProviderEntry>> = HashMap::new();

        for manifest in manifests {
            for capability in &manifest.capabilities {
                if !capability.capability.is_supported() {
                    continue;
                }
                let key = (
                    capability.platform.clone(),
                    capability.capability.as_str().to_string(),
                );
                let entries = routes.entry(key).or_default();
                if !entries
                    .iter()
                    .any(|entry| entry.plugin_name == manifest.name)
                {
                    entries.push(ProviderEntry {
                        plugin_name: manifest.name.clone(),
                    });
                }
            }
        }

        for (platform, capabilities) in &config.routes {
            for (capability, provider_names) in capabilities {
                let key = (platform.clone(), capability.clone());
                let mut entries: Vec<ProviderEntry> = Vec::new();
                for name in provider_names {
                    let exists = manifests.iter().any(|manifest| {
                        manifest.name == *name
                            && manifest.capabilities.iter().any(|cap| {
                                cap.capability.is_supported()
                                    && cap.platform == *platform
                                    && cap.capability.as_str() == capability
                            })
                    });
                    if !exists {
                        continue;
                    }
                    if !entries
                        .iter()
                        .any(|entry| entry.plugin_name == *name)
                    {
                        entries.push(ProviderEntry {
                            plugin_name: name.clone(),
                        });
                    }
                }
                routes.insert(key, entries);
            }
        }

        RoutingTable {
            routes: Mutex::new(routes),
        }
    }

    pub fn get_active(&self, platform: &str, capability: &str) -> Option<ProviderEntry> {
        self.routes
            .lock()
            .unwrap()
            .get(&(platform.to_string(), capability.to_string()))?
            .first()
            .cloned()
    }

    /// Ordered providers for one route (active + fallbacks).
    pub fn get_all(&self, platform: &str, capability: &str) -> Option<Vec<ProviderEntry>> {
        self.routes
            .lock()
            .unwrap()
            .get(&(platform.to_string(), capability.to_string()))
            .cloned()
    }

    pub fn has_route(&self, platform: &str, capability: &str) -> bool {
        self.routes
            .lock()
            .unwrap()
            .contains_key(&(platform.to_string(), capability.to_string()))
    }

    /// Find the active provider for any platform that supports the capability.
    /// Used for platform-agnostic operations such as playlist URL parsing.
    pub fn get_any_capability(&self, capability: &str) -> Option<(String, ProviderEntry)> {
        let routes = self.routes.lock().unwrap();
        for ((platform, cap), entries) in routes.iter() {
            if cap == capability {
                if let Some(active) = entries.first() {
                    return Some((platform.clone(), active.clone()));
                }
            }
        }
        None
    }

    /// Every provider for a capability across platforms, in route order.
    /// Playlist URL parsing uses this so an unknown URL falls through to the
    /// next provider instead of failing on the first one.
    pub fn get_all_capability(&self, capability: &str) -> Vec<(String, ProviderEntry)> {
        let routes = self.routes.lock().unwrap();
        let mut result = Vec::new();
        let mut keys: Vec<_> = routes.keys().cloned().collect();
        keys.sort();
        for key in keys {
            if key.1 != capability {
                continue;
            }
            if let Some(entries) = routes.get(&key) {
                for entry in entries {
                    result.push((key.0.clone(), entry.clone()));
                }
            }
        }
        // Platform-agnostic URL parsing has no route key to order by. Try the
        // Meting adapter before yt-dlp so a Chinese URL does not first wait for
        // a yt-dlp attempt. The plugin name is a stable, user-visible ordering.
        result.sort_by(|left, right| {
            left.1
                .plugin_name
                .cmp(&right.1.plugin_name)
                .then_with(|| left.0.cmp(&right.0))
        });
        result
    }

    pub fn route_count(&self) -> usize {
        self.routes.lock().unwrap().len()
    }

    /// Stable route snapshot for the plugin settings page.
    pub fn snapshot(&self) -> Vec<RouteProviders> {
        let routes = self.routes.lock().unwrap();
        let mut result: Vec<RouteProviders> = routes
            .iter()
            .map(|((platform, capability), entries)| RouteProviders {
                platform: platform.clone(),
                capability: capability.clone(),
                providers: entries
                    .iter()
                    .map(|entry| entry.plugin_name.clone())
                    .collect(),
            })
            .collect();
        result.sort_by(|a, b| {
            a.platform
                .cmp(&b.platform)
                .then_with(|| a.capability.cmp(&b.capability))
        });
        result
    }
}
