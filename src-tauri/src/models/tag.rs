use serde::{Deserialize, Serialize};

/// namespace:value tag structure, e.g. "country:russia", "genre:pop"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag {
    pub namespace: String,
    pub value: String,
}

/// MetaTag is used for UI grouping of tags and does not participate in queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaTag {
    pub namespace: String,
    pub display_name: String,
    pub children: Vec<String>,
}
