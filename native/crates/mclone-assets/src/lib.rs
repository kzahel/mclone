#![forbid(unsafe_code)]

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPath(String);

impl AssetPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_asset_paths_plain() {
        let path = AssetPath::new("minecraft:block/stone");
        assert_eq!(path.as_str(), "minecraft:block/stone");
    }
}
