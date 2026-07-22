use std::collections::HashMap;

use serde::Deserialize;

use crate::{AssetError, AssetPath, AssetResult, AssetSource};

pub const DEFAULT_PLAYER_FIGURE_PATH: &str = "assets/mclone/figures/player.figure.json";
pub const UPRIGHT_BEAR_FIGURE_PATH: &str = "assets/mclone/figures/upright_bear.figure.json";
pub const CHICKEN_FIGURE_PATH: &str = "assets/mclone/figures/chicken.figure.json";
pub const DEFAULT_PLAYER_FIGURE_ID: ActorFigureId = ActorFigureId::from_static("mclone:player");
pub const UPRIGHT_BEAR_FIGURE_ID: ActorFigureId = ActorFigureId::from_static("mclone:upright_bear");
pub const CHICKEN_FIGURE_ID: ActorFigureId = ActorFigureId::from_static("mclone:chicken");
pub const FIRST_PARTY_ACTOR_FIGURE_IDS: [ActorFigureId; 3] = [
    DEFAULT_PLAYER_FIGURE_ID,
    UPRIGHT_BEAR_FIGURE_ID,
    CHICKEN_FIGURE_ID,
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActorFigureId(&'static str);

impl ActorFigureId {
    pub const fn from_static(id: &'static str) -> Self {
        Self(id)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureAsset {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub name: String,
    #[serde(rename = "defaultClip")]
    pub default_clip: Option<String>,
    #[serde(default)]
    pub materials: HashMap<String, FigureMaterial>,
    #[serde(default)]
    pub textures: HashMap<String, FigureAsciiTexture>,
    #[serde(default)]
    pub parts: Vec<FigurePart>,
    #[serde(default)]
    pub clips: HashMap<String, FigureClip>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureMaterial {
    pub color: String,
    #[serde(rename = "alphaMode")]
    pub alpha_mode: Option<FigureAlphaMode>,
    pub opacity: Option<f32>,
    #[serde(rename = "alphaCutoff")]
    pub alpha_cutoff: Option<f32>,
    #[serde(rename = "alphaCoverage")]
    pub alpha_coverage: Option<FigureAlphaCoverage>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FigureAlphaMode {
    Opaque,
    Mask,
    Blend,
    Additive,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FigureAlphaCoverage {
    Threshold,
    Dither,
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
    pub pivot: Option<[f32; 3]>,
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
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub sidedness: Option<FigurePlaneSidedness>,
    pub radius: Option<f32>,
    #[serde(rename = "radiusTop")]
    pub radius_top: Option<f32>,
    #[serde(rename = "radiusBottom")]
    pub radius_bottom: Option<f32>,
    pub length: Option<f32>,
    #[serde(rename = "widthSegments")]
    pub width_segments: Option<u32>,
    #[serde(rename = "heightSegments")]
    pub height_segments: Option<u32>,
    #[serde(rename = "capSegments")]
    pub cap_segments: Option<u32>,
    #[serde(rename = "radialSegments")]
    pub radial_segments: Option<u32>,
    pub faces: Option<HashMap<String, FigureFace>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FigurePlaneSidedness {
    Front,
    Double,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureFace {
    pub material: Option<String>,
    pub texture: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureClip {
    pub fps: Option<f32>,
    #[serde(default)]
    pub r#loop: bool,
    pub label: Option<String>,
    pub role: Option<FigureClipRole>,
    #[serde(rename = "nextClip")]
    pub next_clip: Option<String>,
    pub locomotion: Option<FigureClipLocomotion>,
    #[serde(default)]
    pub keys: Vec<FigureClipKey>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FigureClipRole {
    Locomotion,
    Idle,
    Action,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureClipKey(pub String, pub f32, pub FigureClipTransform);

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct FigureClipTransform {
    pub at: Option<[f32; 3]>,
    pub rot: Option<[f32; 3]>,
    pub scale: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureClipLocomotion {
    pub kind: String,
    #[serde(rename = "cycleDistance")]
    pub cycle_distance: f32,
    #[serde(default)]
    pub contacts: Vec<FigureClipContact>,
    pub direction: Option<[f32; 3]>,
    pub speed: Option<f32>,
    pub units: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FigureClipContact {
    pub part: String,
    #[serde(rename = "phaseStart")]
    pub phase_start: f32,
    #[serde(rename = "phaseEnd")]
    pub phase_end: f32,
    pub role: Option<String>,
    #[serde(rename = "stanceRatio")]
    pub stance_ratio: f32,
}

pub fn default_player_figure_path() -> AssetPath {
    AssetPath::new(DEFAULT_PLAYER_FIGURE_PATH)
}

pub fn upright_bear_figure_path() -> AssetPath {
    AssetPath::new(UPRIGHT_BEAR_FIGURE_PATH)
}

pub fn chicken_figure_path() -> AssetPath {
    AssetPath::new(CHICKEN_FIGURE_PATH)
}

pub const fn default_player_figure_id() -> ActorFigureId {
    DEFAULT_PLAYER_FIGURE_ID
}

pub const fn upright_bear_figure_id() -> ActorFigureId {
    UPRIGHT_BEAR_FIGURE_ID
}

pub const fn chicken_figure_id() -> ActorFigureId {
    CHICKEN_FIGURE_ID
}

pub fn actor_figure_path(id: ActorFigureId) -> Option<AssetPath> {
    match id.as_str() {
        "mclone:player" => Some(default_player_figure_path()),
        "mclone:upright_bear" => Some(upright_bear_figure_path()),
        "mclone:chicken" => Some(chicken_figure_path()),
        _ => None,
    }
}

pub fn load_figure_asset(source: &impl AssetSource, path: &AssetPath) -> AssetResult<FigureAsset> {
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

    #[test]
    fn loads_exported_walk_clip_metadata() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            default_player_figure_path(),
            include_str!("../../../../assets/mclone/figures/player.figure.json"),
        );

        let asset = load_figure_asset(&source, &default_player_figure_path()).unwrap();
        let walk = asset.clips.get("walk").unwrap();

        assert!(walk.r#loop);
        assert_eq!(walk.fps, Some(12.0));
        assert!(!walk.keys.is_empty());
        assert_eq!(walk.keys[0].0, "leg_l");
        let locomotion = walk.locomotion.as_ref().unwrap();
        assert_eq!(locomotion.kind, "biped-walk");
        assert!((locomotion.cycle_distance - 0.86).abs() < 1.0e-6);
        assert_eq!(locomotion.contacts.len(), 2);
        assert_eq!(locomotion.contacts[0].part, "foot_l");
    }

    #[test]
    fn loads_double_sided_plane_primitive() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            default_player_figure_path(),
            r##"
            {
              "schemaVersion": 1,
              "name": "card",
              "materials": { "petal": { "color": "#ffffff" } },
              "textures": {},
              "parts": [
                {
                  "name": "petal",
                  "material": "petal",
                  "primitive": {
                    "kind": "plane",
                    "width": 0.75,
                    "height": 1.25,
                    "sidedness": "double"
                  }
                }
              ],
              "clips": {}
            }
            "##,
        );

        let asset = load_figure_asset(&source, &default_player_figure_path()).unwrap();
        let primitive = &asset.parts[0].primitive;
        assert_eq!(primitive.kind, "plane");
        assert_eq!(primitive.width, Some(0.75));
        assert_eq!(primitive.height, Some(1.25));
        assert_eq!(primitive.sidedness, Some(FigurePlaneSidedness::Double));
    }

    #[test]
    fn default_actor_figure_id_resolves_to_player_asset_path() {
        assert_eq!(default_player_figure_id().as_str(), "mclone:player");
        assert_eq!(
            actor_figure_path(default_player_figure_id()).unwrap(),
            default_player_figure_path()
        );
        assert_eq!(upright_bear_figure_id().as_str(), "mclone:upright_bear");
        assert_eq!(
            actor_figure_path(upright_bear_figure_id()).unwrap(),
            upright_bear_figure_path()
        );
        assert_eq!(chicken_figure_id().as_str(), "mclone:chicken");
        assert_eq!(
            actor_figure_path(chicken_figure_id()).unwrap(),
            chicken_figure_path()
        );
        assert!(actor_figure_path(ActorFigureId::from_static("mclone:missing")).is_none());
    }
}
