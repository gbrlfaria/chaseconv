use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct GltfJson {
    pub asset: GltfAsset,
    pub scene: u32,
    pub scenes: Vec<GltfScene>,
    pub nodes: Vec<GltfNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GltfAsset {
    pub generator: String,
    pub version: String,
}

impl GltfAsset {
    pub fn new() -> Self {
        Self {
            generator: format!("{} {}", env!("CARGO_CRATE_NAME"), env!("CARGO_PKG_VERSION")),
            version: String::from("2.0"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GltfScene {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub nodes: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GltfNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin: Option<u32>,
}
