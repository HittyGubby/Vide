use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::features::plugin::types::{PluginManifest, PluginStatus};

/// A (platform, capability_type) pair used as a routing key.
pub type RouteKey = (String, String);

/// A single provider entry in the routing table.
#[derive(Debug, Clone)]
pub struct ProviderEntry {
    pub plugin_name: String,
    pub exec: Vec<String>,
    pub status: PluginStatus,
}

/// Runtime routing table: maps (platform, capability) → list of providers.
///
/// The first provider in the vec is the active (currently selected) one.
/// Remaining entries are fallbacks in order.
#[derive(Debug, Clone)]
pub struct RoutingTable {
    routes: HashMap<RouteKey, Vec<ProviderEntry>>,
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
        // Phase 1: collect all providers by route key
        let mut providers: HashMap<RouteKey, Vec<ProviderEntry>> = HashMap::new();
        let mut plugin_exec: HashMap<&str, Vec<String>> = HashMap::new();
        let mut plugin_order: Vec<&str> = Vec::new();

        for m in manifests {
            plugin_exec.insert(&m.name, m.exec.clone());
            plugin_order.push(&m.name);
        }

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

        // Phase 2: apply user overrides (reorder to put chosen provider first)
        for (override_key, chosen_plugin) in &config.overrides {
            // override_key format: "platform.capability"
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

        RoutingTable { routes: providers }
    }

    /// Get the active (first) provider for a (platform, capability) pair.
    pub fn get_active(&self, platform: &str, capability: &str) -> Option<&ProviderEntry> {
        self.routes.get(&(platform.to_string(), capability.to_string()))?.first()
    }

    /// Get all providers for a (platform, capability) pair (active + fallbacks).
    pub fn get_all(&self, platform: &str, capability: &str) -> Option<&Vec<ProviderEntry>> {
        self.routes.get(&(platform.to_string(), capability.to_string()))
    }

    /// Update the status of a specific provider.
    pub fn set_status(&mut self, platform: &str, capability: &str, plugin_name: &str, status: PluginStatus) {
        if let Some(entries) = self.routes.get_mut(&(platform.to_string(), capability.to_string())) {
            if let Some(entry) = entries.iter_mut().find(|e| e.plugin_name == plugin_name) {
                entry.status = status;
            }
        }
    }

    /// Promote the next fallback to active for a route (after failure).
    pub fn fallback(&mut self, platform: &str, capability: &str) -> Option<&ProviderEntry> {
        let key = (platform.to_string(), capability.to_string());
        if let Some(entries) = self.routes.get_mut(&key) {
            if entries.len() > 1 {
                let failed = entries.remove(0);
                // Put the failed one at the end
                entries.push(failed);
            }
            return entries.first();
        }
        None
    }

    /// Check if a route has any provider.
    pub fn has_route(&self, platform: &str, capability: &str) -> bool {
        self.routes.contains_key(&(platform.to_string(), capability.to_string()))
    }

    /// Number of distinct routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Serialize the current routing overrides to a ProviderConfig for persistence.
    pub fn to_config(&self) -> ProviderConfig {
        let mut overrides = HashMap::new();
        for ((platform, capability), entries) in &self.routes {
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
