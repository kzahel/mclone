use std::collections::HashMap;

use serde::Deserialize;

use crate::{AssetError, AssetPath, AssetResult, AssetSource};

pub const DEFAULT_PLAYER_FIGURE_PATH: &str = "assets/mclone/figures/player.figure.json";

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureAsset {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub materials: HashMap<String, FigureMaterial>,
    #[serde(default)]
    pub textures: HashMap<String, FigureAsciiTexture>,
    #[serde(default)]
    pub parts: Vec<FigurePart>,
    #[serde(default)]
    pub clips: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureMaterial {
    pub color: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureAsciiTexture {
    pub palette: HashMap<String, String>,
    pub pixels: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigurePart {
    pub name: String,
    pub parent: Option<String>,
    pub at: Option<[f32; 3]>,
    pub rot: Option<[f32; 3]>,
    pub material: Option<String>,
    pub texture: Option<String>,
    pub joint: Option<FigureJoint>,
    pub primitive: FigurePrimitive,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureJoint {
    pub pivot: Option<[f32; 3]>,
    pub axis: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigurePrimitive {
    pub kind: String,
    pub size: Option<[f32; 3]>,
    pub faces: Option<HashMap<String, FigureFace>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureFace {
    pub material: Option<String>,
    pub texture: Option<String>,
}

pub fn default_player_figure_path() -> AssetPath {
    AssetPath::new(DEFAULT_PLAYER_FIGURE_PATH)
}

pub fn load_figure_asset(
    source: &impl AssetSource,
    path: &AssetPath,
) -> AssetResult<FigureAsset> {
    let bytes = source
        .read(path)?
        .ok_or_else(|| AssetError::MissingAsset(path.clone()))?;
    serde_json::from_slice(&bytes).map_err(|source| AssetError::Json {
        path: path.clone(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryAssetSource;

    #[test]
    fn loads_figure_asset_from_asset_source() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            default_player_figure_path(),
            r##"
            {
              "schemaVersion": 1,
              "name": "tiny",
              "materials": { "skin": { "color": "#ffffff" } },
              "textures": {},
              "parts": [
                {
                  "name": "body",
                  "material": "skin",
                  "primitive": { "kind": "box", "size": [1, 1, 1] }
                }
              ],
              "clips": {}
            }
            "##,
        );

        let asset = load_figure_asset(&source, &default_player_figure_path()).unwrap();

        assert_eq!(asset.schema_version, 1);
        assert_eq!(asset.name, "tiny");
        assert_eq!(asset.parts[0].primitive.kind, "box");
    }
}
