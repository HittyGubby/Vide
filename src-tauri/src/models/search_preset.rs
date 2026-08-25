use serde::{Deserialize, Serialize};

/// A saved search preset — a named query string that builds a dynamic
/// library view (SPEC §7.1). For example:
///
/// ```text
/// name:  "Russian Folk"
/// query: "country:russia AND genre:folk"
/// ```
///
/// Clicking a preset applies its query to the library, rather than
/// maintaining a fixed, static list of songs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchPreset {
    /// Stable identifier (used for delete + last-selected tracking).
    pub id: String,
    /// Human-readable name shown in the sidebar.
    pub name: String,
    /// The query string, e.g. `country:russia AND genre:folk`.
    pub query: String,
}
