#![forbid(unsafe_code)]

mod block_registry;
mod resource;
mod source;

use std::error::Error;
use std::fmt;

pub use block_registry::{
    BlockStateAsset, BlockStateAssetIndex, BlockStateRecord, BlockStateRegistry,
};
pub use resource::{AssetPath, ResourceLocation};
pub use source::{AssetSource, MemoryAssetSource};

#[cfg(not(target_arch = "wasm32"))]
pub use source::FilesystemAssetSource;

pub type AssetResult<T> = Result<T, AssetError>;

#[derive(Debug)]
pub enum AssetError {
    InvalidAssetPath(String),
    InvalidResourceLocation(String),
    InvalidBlockState(String),
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
