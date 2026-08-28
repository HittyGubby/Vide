use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::features::plugin::types::{PluginManifest, PluginStatus};

/// A (platform, capability_type) pair used as a routing key.
pub type RouteKey = (String, String);

/// A single provider entry in the routing table.
#[derive(Debug, Clone)]
pub struct ProviderEntry {
    pub plugin_name: String,
    #[allow(dead_code)]
    pub exec: Vec<String>,
    #[allow(dead_code)]
    pub status: PluginStatus,
}

/// Runtime routing table: maps (platform, capability) → list of providers.
///
/// The first provider in the vec is the active (currently selected) one.
/// Remaining entries are fallbacks in order.
/// Uses interior mutability (Mutex) so that all methods take &self.
#[derive(Debug)]
pub struct RoutingTable {
    routes: Mutex<HashMap<RouteKey, Vec<ProviderEntry>>>,
}

/// User-defined provider overrides, stored in SQLite settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    /// Maps "platform.capability" → plugin_name
    pub overrides: HashMap<String, String>,
    /// Per-plugin concurrency limits
    pub concurrency: HashMap<String, u32>,
}

impl RoutingTable {
    /// Build a routing table from loaded manifests and optional user config.
    pub fn build(manifests: &[PluginManifest], config: &ProviderConfig) -> Self {
        let mut providers: HashMap<RouteKey, Vec<ProviderEntry>> = HashMap::new();

        for m in manifests {
            for cap in &m.capabilities {
                let key = (cap.platform.clone(), cap.capability.clone());
                let entry = ProviderEntry {
                    plugin_name: m.name.clone(),
                    exec: m.exec.clone(),
                    status: PluginStatus::Idle,
                };
                providers.entry(key).or_default().push(entry);
            }
        }

        // Apply user overrides (reorder to put chosen provider first)
        for (override_key, chosen_plugin) in &config.overrides {
            let parts: Vec<&str> = override_key.splitn(2, '.').collect();
            if parts.len() != 2 {
                continue;
            }
            let key = (parts[0].to_string(), parts[1].to_string());
            if let Some(entries) = providers.get_mut(&key) {
                if let Some(pos) = entries.iter().position(|e| e.plugin_name == *chosen_plugin) {
                    let chosen = entries.remove(pos);
                    entries.insert(0, chosen);
                }
            }
        }

        RoutingTable {
            routes: Mutex::new(providers),
        }
    }

    /// Get the active (first) provider for a (platform, capability) pair.
    pub fn get_active(&self, platform: &str, capability: &str) -> Option<ProviderEntry> {
        self.routes
            .lock()
            .unwrap()
            .get(&(platform.to_string(), capability.to_string()))?
            .first()
            .cloned()
    }

    /// Get all providers for a (platform, capability) pair (active + fallbacks).
    pub fn get_all(&self, platform: &str, capability: &str) -> Option<Vec<ProviderEntry>> {
        self.routes
            .lock()
            .unwrap()
            .get(&(platform.to_string(), capability.to_string()))
            .cloned()
    }

    /// Update the status of a specific provider.
    pub fn set_status(
        &self,
        platform: &str,
        capability: &str,
        plugin_name: &str,
        status: PluginStatus,
    ) {
        if let Some(entries) = self
            .routes
            .lock()
            .unwrap()
            .get_mut(&(platform.to_string(), capability.to_string()))
        {
            if let Some(entry) = entries.iter_mut().find(|e| e.plugin_name == plugin_name) {
                entry.status = status;
            }
        }
    }

    /// Promote the next fallback to active for a route (after failure).
    /// Returns the new active provider, or None if no route exists.
    pub fn fallback(&self, platform: &str, capability: &str) -> Option<ProviderEntry> {
        let key = (platform.to_string(), capability.to_string());
        let mut routes = self.routes.lock().unwrap();
        if let Some(entries) = routes.get_mut(&key) {
            if entries.len() > 1 {
                let failed = entries.remove(0);
                entries.push(failed);
            }
            return entries.first().cloned();
        }
        None
    }

    /// Check if a route has any provider.
    pub fn has_route(&self, platform: &str, capability: &str) -> bool {
        self.routes
            .lock()
            .unwrap()
            .contains_key(&(platform.to_string(), capability.to_string()))
    }

    /// Find the active provider for *any* platform that supports the given
    /// capability. Used for platform-agnostic capabilities (e.g. parsing a
    /// playlist URL) where the caller does not know the platform in advance.
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

    /// Number of distinct routes.
    pub fn route_count(&self) -> usize {
        self.routes.lock().unwrap().len()
    }

    /// Serialize the current routing overrides to a ProviderConfig for persistence.
    pub fn to_config(&self) -> ProviderConfig {
        let routes = self.routes.lock().unwrap();
        let mut overrides = HashMap::new();
        for ((platform, capability), entries) in &*routes {
            if let Some(active) = entries.first() {
                let key = format!("{}.{}", platform, capability);
                overrides.insert(key, active.plugin_name.clone());
            }
        }
        ProviderConfig {
            overrides,
            ..Default::default()
        }
    }
}
