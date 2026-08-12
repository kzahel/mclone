#![forbid(unsafe_code)]

mod atlas;
mod block_registry;
mod figure;
mod first_party;
mod inventory;
mod model;
mod pack;
mod prepared_figure;
mod profile;
mod resource;
mod source;

use std::error::Error;
use std::fmt;

pub use atlas::{TextureAtlasPlan, TextureAtlasSprite, TextureSpriteInfo};
pub use block_registry::{
    BlockStateAsset, BlockStateAssetIndex, BlockStateRecord, BlockStateRegistry, BlockStateVariant,
    MultipartCase, MultipartWhen,
};
pub use figure::{
    ActorFigureId, CHICKEN_FIGURE_ID, CHICKEN_FIGURE_PATH, COW_FIGURE_ID, COW_FIGURE_PATH,
    DEER_BED_FIGURE_ID, DEER_BED_FIGURE_PATH, DEER_FIGURE_ID, DEER_FIGURE_PATH,
    DEER_HIDE_FIGURE_ID, DEER_HIDE_FIGURE_PATH, DEFAULT_PLAYER_FIGURE_ID,
    DEFAULT_PLAYER_FIGURE_PATH, FIRST_PARTY_ACTOR_FIGURE_IDS, FIRST_PARTY_SEMANTIC_PROP_FIGURE_IDS,
    FigureAlphaCoverage, FigureAlphaMode, FigureAsciiTexture, FigureAsset, FigureClip,
    FigureClipContact, FigureClipKey, FigureClipLocomotion, FigureClipRole, FigureClipTransform,
    FigureFace, FigureJoint, FigureMaterial, FigurePart, FigurePlaneSidedness, FigurePrimitive,
    HUNTING_SPEAR_FIGURE_ID, HUNTING_SPEAR_FIGURE_PATH, MALLARD_DUCK_FIGURE_ID,
    MALLARD_DUCK_FIGURE_PATH, MALLARD_FEATHER_FIGURE_ID, MALLARD_FEATHER_FIGURE_PATH,
    MALLARD_NEST_FIGURE_ID, MALLARD_NEST_FIGURE_PATH, SHED_ANTLER_FIGURE_ID,
    SHED_ANTLER_FIGURE_PATH, SemanticFigureId, UPRIGHT_BEAR_FIGURE_ID, UPRIGHT_BEAR_FIGURE_PATH,
    VENISON_FIGURE_ID, VENISON_FIGURE_PATH, chicken_figure_id, chicken_figure_path, cow_figure_id,
    cow_figure_path, deer_bed_figure_id, deer_bed_figure_path, deer_figure_id, deer_figure_path,
    deer_hide_figure_id, deer_hide_figure_path, default_player_figure_id,
    default_player_figure_path, first_party_actor_figure_path, hunting_spear_figure_id,
    hunting_spear_figure_path, load_figure_asset, mallard_duck_figure_id, mallard_duck_figure_path,
    mallard_feather_figure_id, mallard_feather_figure_path, mallard_nest_figure_id,
    mallard_nest_figure_path, semantic_figure_path, shed_antler_figure_id, shed_antler_figure_path,
    upright_bear_figure_id, upright_bear_figure_path, venison_figure_id, venison_figure_path,
};
pub use first_party::{
    FIRST_PARTY_AUDIO_POLICY_PATH, FIRST_PARTY_MISSING_REGISTRY_PATH,
    FIRST_PARTY_VISUAL_CATALOG_PATH, FIRST_PARTY_VISUAL_SCHEMA, FirstPartyAudioPolicy,
    FirstPartyVisualCatalog, FirstPartyVisualDefinition, MissingAssetRegistry,
    MissingAssetRegistryEntry,
};
pub use inventory::{
    AssetConsumerKind, AssetRequirementPolicy, CanonicalAssetRequirement,
    CanonicalFirstPartyAssetInventory, FirstPartyBlockVisual, FirstPartyVisualClass,
    canonical_first_party_asset_inventory,
};
pub use model::{
    BakedBlockModel, BakedBlockModelFace, BlockModel, BlockModelElement, BlockModelFace,
    BlockModelLibrary, ModelFaceDirection, TextureMaterial, TextureReference,
};
pub use pack::{
    AssetPackManifest, DEFAULT_PACK_MANIFEST_PATH, PACK_FORMAT_VERSION, PackedAssetSource,
};
pub use prepared_figure::{
    FigurePoseError, FigurePrepareError, PREPARED_FIGURE_COMPILER_ID, PreparedFigure,
    PreparedFigureAtlas, PreparedFigureBounds, PreparedFigureClip, PreparedFigureClipContact,
    PreparedFigureClipKey, PreparedFigureClipLocomotion, PreparedFigureDiagnostics,
    PreparedFigureDrawRange, PreparedFigurePart, PreparedFigurePartRotationOverride,
    PreparedFigurePass, PreparedFigurePassRange, PreparedFigurePoseSample,
    PreparedFigurePrimitiveKind, PreparedFigureVertex, evaluate_prepared_figure_clip_into,
    evaluate_prepared_figure_clip_with_part_rotation_overrides_into,
    evaluate_prepared_figure_rest_pose_into, load_prepared_figure, prepare_figure_asset,
};
pub use profile::{
    AUTHORED_FIRST_PARTY_PACK_ID, AssetPackAvailability, AssetPackCatalog, AssetPackDescriptor,
    AssetPackDiscovery, AssetPackId, AssetPackOrigin, AssetPackRole, AssetPackSelection,
    AssetProvenanceEntry, AssetProvenanceReport, AssetProvenanceSummary, AssetResolutionOrigin,
    AssetResolutionOutcome, DIAGNOSTIC_MISSING_PACK_ID, MINECRAFT_REFERENCE_PACK_ID,
    PROVISIONAL_FIRST_PARTY_PACK_ID, TexturePresentation, TextureVisualProfile,
};
pub use resource::{AssetPath, ResourceLocation};
pub use source::{
    AssetSource, AssetSourceChain, MemoryAssetSource, NamedAssetResolution,
    ProvenanceTrackingAssetSource, SharedAssetSource,
};

#[cfg(not(target_arch = "wasm32"))]
pub use source::FilesystemAssetSource;

pub type AssetResult<T> = Result<T, AssetError>;

#[derive(Debug)]
pub enum AssetError {
    InvalidAssetPath(String),
    InvalidResourceLocation(String),
    InvalidBlockState(String),
    InvalidModel(String),
    InvalidTexture(String),
    InvalidFigure(String),
    InvalidAssetPack(String),
    MissingAsset(AssetPath),
    Io(std::io::Error),
    Json {
        path: AssetPath,
        source: serde_json::Error,
    },
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAssetPath(path) => write!(f, "invalid asset path `{path}`"),
            Self::InvalidResourceLocation(location) => {
                write!(f, "invalid resource location `{location}`")
            }
            Self::InvalidBlockState(message) => write!(f, "invalid block state: {message}"),
            Self::InvalidModel(message) => write!(f, "invalid model: {message}"),
            Self::InvalidTexture(message) => write!(f, "invalid texture: {message}"),
            Self::InvalidFigure(message) => write!(f, "invalid figure: {message}"),
            Self::InvalidAssetPack(message) => write!(f, "invalid asset pack: {message}"),
            Self::MissingAsset(path) => write!(f, "missing asset {}", path.as_str()),
            Self::Io(error) => write!(f, "{error}"),
            Self::Json { path, source } => {
                write!(f, "failed to parse JSON asset {}: {source}", path.as_str())
            }
        }
    }
}

impl Error for AssetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AssetError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
