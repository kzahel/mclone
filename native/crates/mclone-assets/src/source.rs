use std::collections::BTreeMap;

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use crate::{AssetPath, AssetResult};

pub trait AssetSource {
    fn read(&self, path: &AssetPath) -> AssetResult<Option<Vec<u8>>>;
    fn list(&self, prefix: &str, suffix: &str) -> AssetResult<Vec<AssetPath>>;
}

#[derive(Clone, Debug, Default)]
pub struct MemoryAssetSource {
    assets: BTreeMap<AssetPath, Vec<u8>>,
}

impl MemoryAssetSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, path: AssetPath, bytes: impl Into<Vec<u8>>) -> Option<Vec<u8>> {
        self.assets.insert(path, bytes.into())
    }

    pub fn insert_text(&mut self, path: AssetPath, text: impl Into<String>) -> Option<Vec<u8>> {
        self.insert(path, text.into().into_bytes())
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }
}

impl AssetSource for MemoryAssetSource {
    fn read(&self, path: &AssetPath) -> AssetResult<Option<Vec<u8>>> {
        Ok(self.assets.get(path).cloned())
    }

    fn list(&self, prefix: &str, suffix: &str) -> AssetResult<Vec<AssetPath>> {
        Ok(self
            .assets
            .keys()
            .filter(|path| path.as_str().starts_with(prefix) && path.as_str().ends_with(suffix))
            .cloned()
            .collect())
    }
}

#[derive(Default)]
pub struct AssetSourceChain {
    sources: Vec<Box<dyn AssetSource>>,
}

impl AssetSourceChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, source: impl AssetSource + 'static) {
        self.sources.push(Box::new(source));
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

impl std::fmt::Debug for AssetSourceChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetSourceChain")
            .field("source_count", &self.sources.len())
            .finish()
    }
}

impl AssetSource for AssetSourceChain {
    fn read(&self, path: &AssetPath) -> AssetResult<Option<Vec<u8>>> {
        for source in &self.sources {
            if let Some(bytes) = source.read(path)? {
                return Ok(Some(bytes));
            }
        }
        Ok(None)
    }

    fn list(&self, prefix: &str, suffix: &str) -> AssetResult<Vec<AssetPath>> {
        let mut paths = std::collections::BTreeSet::new();
        for source in &self.sources {
            paths.extend(source.list(prefix, suffix)?);
        }
        Ok(paths.into_iter().collect())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FilesystemAssetSource {
    root: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FilesystemAssetSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn full_path(&self, path: &AssetPath) -> PathBuf {
        self.root.join(path.as_str())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl AssetSource for FilesystemAssetSource {
    fn read(&self, path: &AssetPath) -> AssetResult<Option<Vec<u8>>> {
        let full_path = self.full_path(path);
        match std::fs::read(full_path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn list(&self, prefix: &str, suffix: &str) -> AssetResult<Vec<AssetPath>> {
        let mut paths = Vec::new();
        collect_files(&self.root, &self.root.join(prefix), suffix, &mut paths)?;
        paths.sort();
        Ok(paths)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_files(
    root: &Path,
    current: &Path,
    suffix: &str,
    out: &mut Vec<AssetPath>,
) -> AssetResult<()> {
    let entries = match std::fs::read_dir(current) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(root, &path, suffix, out)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| crate::AssetError::InvalidAssetPath(path.display().to_string()))?
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            if relative.ends_with(suffix) {
                out.push(AssetPath::try_new(relative)?);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_source_reads_and_lists_pack_paths() {
        let mut source = MemoryAssetSource::new();
        source.insert_text(
            AssetPath::new("assets/minecraft/blockstates/stone.json"),
            "{}",
        );
        source.insert_text(
            AssetPath::new("assets/minecraft/models/block/stone.json"),
            "{}",
        );

        assert_eq!(
            source
                .read(&AssetPath::new("assets/minecraft/blockstates/stone.json"))
                .unwrap(),
            Some(b"{}".to_vec())
        );
        assert_eq!(
            source
                .list("assets/minecraft/blockstates/", ".json")
                .unwrap()
                .into_iter()
                .map(|path| path.as_str().to_owned())
                .collect::<Vec<_>>(),
            vec!["assets/minecraft/blockstates/stone.json"]
        );
    }

    #[test]
    fn source_chain_reads_first_available_asset_and_merges_lists() {
        let mut first = MemoryAssetSource::new();
        first.insert_text(
            AssetPath::new("assets/minecraft/blockstates/stone.json"),
            "first",
        );

        let mut second = MemoryAssetSource::new();
        second.insert_text(
            AssetPath::new("assets/minecraft/blockstates/dirt.json"),
            "second",
        );
        second.insert_text(
            AssetPath::new("assets/minecraft/blockstates/stone.json"),
            "shadowed",
        );

        let mut source = AssetSourceChain::new();
        source.push(first);
        source.push(second);

        assert_eq!(
            source
                .read(&AssetPath::new("assets/minecraft/blockstates/stone.json"))
                .unwrap(),
            Some(b"first".to_vec())
        );
        assert_eq!(
            source
                .list("assets/minecraft/blockstates/", ".json")
                .unwrap()
                .into_iter()
                .map(|path| path.as_str().to_owned())
                .collect::<Vec<_>>(),
            vec![
                "assets/minecraft/blockstates/dirt.json",
                "assets/minecraft/blockstates/stone.json"
            ]
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn filesystem_source_reads_real_extracted_asset_when_present() {
        let root = std::path::PathBuf::from("../reference/minecraft-1.17.1/extracted");
        if !root.exists() {
            return;
        }
        let source = FilesystemAssetSource::new(root);

        let stone = source
            .read(&AssetPath::new("assets/minecraft/blockstates/stone.json"))
            .unwrap();

        assert!(stone.is_some());
    }
}
