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
    ActorFigureId, CHICKEN_FIGURE_ID, CHICKEN_FIGURE_PATH, DEFAULT_PLAYER_FIGURE_ID,
    DEFAULT_PLAYER_FIGURE_PATH, FIRST_PARTY_ACTOR_FIGURE_IDS, FigureAsciiTexture, FigureAsset,
    FigureClip, FigureClipContact, FigureClipKey, FigureClipLocomotion, FigureClipTransform,
    FigureFace, FigureJoint, FigureMaterial, FigurePart, FigurePrimitive, UPRIGHT_BEAR_FIGURE_ID,
    UPRIGHT_BEAR_FIGURE_PATH, actor_figure_path, chicken_figure_id, chicken_figure_path,
    default_player_figure_id, default_player_figure_path, load_figure_asset,
    upright_bear_figure_id, upright_bear_figure_path,
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
    PreparedFigureDrawRange, PreparedFigurePart, PreparedFigurePoseSample,
    PreparedFigurePrimitiveKind, PreparedFigureVertex, evaluate_prepared_figure_clip_into,
    evaluate_prepared_figure_rest_pose_into, load_prepared_figure, prepare_figure_asset,
};
pub use profile::{
    AssetPackAvailability, AssetPackCatalog, AssetPackDescriptor, AssetPackDiscovery, AssetPackId,
    AssetPackOrigin, AssetPackRole, AssetPackSelection, AssetProvenanceEntry,
    AssetProvenanceReport, AssetProvenanceSummary, AssetResolutionOrigin, AssetResolutionOutcome,
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
