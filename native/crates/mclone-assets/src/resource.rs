use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use crate::{AssetError, AssetResult};

const DEFAULT_NAMESPACE: &str = "minecraft";

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceLocation {
    namespace: String,
    path: String,
}

impl ResourceLocation {
    pub fn new(namespace: impl Into<String>, path: impl Into<String>) -> AssetResult<Self> {
        let namespace = namespace.into();
        let path = path.into();
        if namespace.is_empty() || !namespace.chars().all(valid_namespace_char) {
            return Err(AssetError::InvalidResourceLocation(format!(
                "{namespace}:{path}"
            )));
        }
        if path.is_empty() || !path.chars().all(valid_path_char) {
            return Err(AssetError::InvalidResourceLocation(format!(
                "{namespace}:{path}"
            )));
        }
        Ok(Self { namespace, path })
    }

    pub fn minecraft(path: impl Into<String>) -> AssetResult<Self> {
        Self::new(DEFAULT_NAMESPACE, path)
    }

    pub fn parse(value: &str) -> AssetResult<Self> {
        let (namespace, path) = match value.split_once(':') {
            Some(("", path)) => (DEFAULT_NAMESPACE, path),
            Some((namespace, path)) => (namespace, path),
            None => (DEFAULT_NAMESPACE, value),
        };
        Self::new(namespace, path)
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for ResourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

impl FromStr for ResourceLocation {
    type Err = AssetError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Ord for ResourceLocation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path
            .cmp(&other.path)
            .then_with(|| self.namespace.cmp(&other.namespace))
    }
}

impl PartialOrd for ResourceLocation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetPath(String);

impl AssetPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self::try_new(path).expect("invalid asset path")
    }

    pub fn try_new(path: impl Into<String>) -> AssetResult<Self> {
        let path = path.into();
        if path.is_empty()
            || path.starts_with('/')
            || path.starts_with('\\')
            || path.contains('\\')
            || path.split('/').any(|part| part.is_empty() || part == "..")
        {
            return Err(AssetError::InvalidAssetPath(path));
        }
        Ok(Self(path))
    }

    pub fn blockstate_json(location: &ResourceLocation) -> Self {
        Self::asset_json(location, "blockstates")
    }

    pub fn model_json(location: &ResourceLocation) -> Self {
        Self::asset_json(location, "models")
    }

    pub fn texture_png(location: &ResourceLocation) -> Self {
        Self::new(format!(
            "assets/{}/textures/{}.png",
            location.namespace(),
            location.path()
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn asset_json(location: &ResourceLocation, directory: &str) -> Self {
        Self::new(format!(
            "assets/{}/{}/{}.json",
            location.namespace(),
            directory,
            location.path()
        ))
    }
}

impl fmt::Display for AssetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn valid_namespace_char(value: char) -> bool {
    matches!(value, '_' | '-' | '.' | 'a'..='z' | '0'..='9')
}

fn valid_path_char(value: char) -> bool {
    matches!(value, '_' | '-' | '/' | '.' | 'a'..='z' | '0'..='9')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_location_defaults_to_minecraft_namespace() {
        let location = ResourceLocation::parse("block/stone").unwrap();

        assert_eq!(location.namespace(), "minecraft");
        assert_eq!(location.path(), "block/stone");
        assert_eq!(location.to_string(), "minecraft:block/stone");
    }

    #[test]
    fn resource_location_rejects_invalid_characters() {
        assert!(ResourceLocation::parse("Minecraft:stone").is_err());
        assert!(ResourceLocation::parse("minecraft:block stone").is_err());
    }

    #[test]
    fn resource_location_orders_like_java_path_then_namespace() {
        let mut locations = vec![
            ResourceLocation::parse("z:last").unwrap(),
            ResourceLocation::parse("minecraft:block/stone").unwrap(),
            ResourceLocation::parse("custom:block/stone").unwrap(),
        ];

        locations.sort();

        assert_eq!(
            locations
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["custom:block/stone", "minecraft:block/stone", "z:last"]
        );
    }

    #[test]
    fn asset_paths_use_pack_relative_paths() {
        let stone = ResourceLocation::parse("minecraft:stone").unwrap();

        assert_eq!(
            AssetPath::blockstate_json(&stone).as_str(),
            "assets/minecraft/blockstates/stone.json"
        );
        assert_eq!(
            AssetPath::model_json(&ResourceLocation::parse("minecraft:block/stone").unwrap())
                .as_str(),
            "assets/minecraft/models/block/stone.json"
        );
    }

    #[test]
    fn asset_paths_reject_traversal() {
        assert!(AssetPath::try_new("../assets/minecraft/blockstates/stone.json").is_err());
        assert!(AssetPath::try_new("assets/minecraft//stone.json").is_err());
    }
}
