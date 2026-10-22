use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Protocol version understood by this core.
pub const PROTOCOL_VERSION: u32 = 1;

/// Capabilities defined by protocol v1.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Search,
    Resolve,
    Stream,
    Playlist,
    Lyrics,
    /// Forward compatibility: manifests may contain newer capability names.
    #[serde(other)]
    Unknown,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::Search => "search",
            Capability::Resolve => "resolve",
            Capability::Stream => "stream",
            Capability::Playlist => "playlist",
            Capability::Lyrics => "lyrics",
            Capability::Unknown => "unknown",
        }
    }

    pub fn is_supported(&self) -> bool {
        !matches!(self, Capability::Unknown)
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A (capability, platform) pair that a plugin offers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlatformCapability {
    #[serde(rename = "type")]
    pub capability: Capability,
    pub platform: String,
}

/// Plugin manifest loaded from manifest.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    #[serde(default = "default_protocol")]
    pub protocol: u32,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub exec: Vec<String>,
    pub capabilities: Vec<PlatformCapability>,
    /// Absolute plugin directory; not part of the file format. Used as cwd.
    #[serde(skip)]
    pub dir: PathBuf,
}

fn default_protocol() -> u32 {
    PROTOCOL_VERSION
}

/// One item returned by search/resolve/playlist.
///
/// source_id is the platform item id; it is deliberately not called id
/// because id in the wire protocol is only the request correlation id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub platform: String,
    #[serde(rename = "source_id", alias = "id")]
    pub source_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
}

/// Backwards-compatible alias for code that still speaks of search results.
pub type SearchResultItem = Track;

/// A stream descriptor returned by a plugin's stream action (protocol v1).
///
/// Protocol v1 only permits pipe descriptors: the plugin describes a command
/// that writes media bytes to stdout, then exits. The core owns the process,
/// cache file, and playback.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginStream {
    Pipe {
        cmd: Vec<String>,
    },
}

/// Internal playback source. Local files and the remote-sharing server still
/// use direct URLs; plugin streams are converted to Pipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamDescriptor {
    UrlStream {
        #[serde(default)]
        id: String,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
    },
    Pipe {
        #[serde(default)]
        id: String,
        cmd: Vec<String>,
    },
}

/// Online lyrics result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lyrics {
    pub text: String,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
}

/// Runtime status of a plugin invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    Idle,
    Running,
    Exited(Option<i32>),
    Error(String),
}

/// Result of one one-shot plugin invocation.
#[derive(Debug, Clone)]
pub struct PluginInvocation {
    pub result: serde_json::Value,
    /// Raw stderr from this invocation (not persisted by the core).
    pub stderr: String,
    /// Non-JSON stdout lines (protocol noise / plugin logs).
    pub logs: Vec<String>,
    pub exit_code: Option<i32>,
}
