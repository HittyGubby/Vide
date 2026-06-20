use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The type of capability a plugin provides.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityType {
    Search,
    Metadata,
    Playback,
    Lyrics,
    Recommendation,
    /// Allows future capability types without recompilation.
    #[serde(other)]
    Other,
}

impl std::fmt::Display for CapabilityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapabilityType::Search => write!(f, "search"),
            CapabilityType::Metadata => write!(f, "metadata"),
            CapabilityType::Playback => write!(f, "playback"),
            CapabilityType::Lyrics => write!(f, "lyrics"),
            CapabilityType::Recommendation => write!(f, "recommendation"),
            CapabilityType::Other => write!(f, "other"),
        }
    }
}

/// A (capability, platform) pair that a plugin offers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlatformCapability {
    #[serde(rename = "type")]
    pub capability: String,
    pub platform: String,
}

/// Plugin manifest loaded from manifest.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub exec: Vec<String>,
    pub capabilities: Vec<PlatformCapability>,
}

/// Stream descriptor returned by a plugin's `stream` action.
/// §10.3: only two output types are valid.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamDescriptor {
    #[serde(rename = "url_stream")]
    UrlStream {
        #[serde(default)]
        id: String,
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
    },
    #[serde(rename = "stream")]
    Pipe {
        #[serde(default)]
        id: String,
        exec: StreamExec,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamExec {
    pub cmd: Vec<String>,
    pub mode: String,
}

/// A single item in search results from a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub platform: String,
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

/// Status of a plugin process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    Idle,
    Running,
    Error(String),
}
