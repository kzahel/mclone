use std::cell::RefCell;
use std::collections::BTreeMap;

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use crate::{
    AssetError, AssetPackCatalog, AssetPackId, AssetPackOrigin, AssetPackSelection, AssetPath,
    AssetProvenanceEntry, AssetProvenanceReport, AssetResolutionOrigin, AssetResolutionOutcome,
    AssetResult,
};

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
    sources: Vec<AssetSourceEntry>,
}

struct AssetSourceEntry {
    identity: AssetResolutionOrigin,
    source: Box<dyn AssetSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedAssetResolution {
    pub bytes: Vec<u8>,
    pub source: AssetResolutionOrigin,
}

impl AssetSourceChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, source: impl AssetSource + 'static) {
        self.sources.push(AssetSourceEntry {
            identity: AssetResolutionOrigin::anonymous(),
            source: Box::new(source),
        });
    }

    pub fn push_named(
        &mut self,
        pack_id: AssetPackId,
        origin: AssetPackOrigin,
        source: impl AssetSource + 'static,
    ) {
        self.push_named_boxed(pack_id, origin, Box::new(source));
    }

    pub fn from_selection(
        catalog: &AssetPackCatalog,
        selection: &AssetPackSelection,
        sources: impl IntoIterator<Item = (AssetPackId, Box<dyn AssetSource>)>,
    ) -> AssetResult<Self> {
        let mut sources = sources.into_iter().collect::<BTreeMap<_, _>>();
        let mut chain = Self::new();
        for descriptor in catalog.source_order(selection)? {
            let source = sources.remove(&descriptor.id).ok_or_else(|| {
                AssetError::InvalidAssetPack(format!(
                    "selected asset pack `{}` has no readable source",
                    descriptor.id
                ))
            })?;
            chain.push_named_boxed(descriptor.id.clone(), descriptor.origin, source);
        }
        Ok(chain)
    }

    pub fn resolve(&self, path: &AssetPath) -> AssetResult<Option<NamedAssetResolution>> {
        for entry in &self.sources {
            if let Some(bytes) = entry.source.read(path)? {
                return Ok(Some(NamedAssetResolution {
                    bytes,
                    source: entry.identity.clone(),
                }));
            }
        }
        Ok(None)
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    fn push_named_boxed(
        &mut self,
        pack_id: AssetPackId,
        origin: AssetPackOrigin,
        source: Box<dyn AssetSource>,
    ) {
        self.sources.push(AssetSourceEntry {
            identity: AssetResolutionOrigin::named(pack_id, origin),
            source,
        });
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
        Ok(self.resolve(path)?.map(|resolution| resolution.bytes))
    }

    fn list(&self, prefix: &str, suffix: &str) -> AssetResult<Vec<AssetPath>> {
        let mut paths = std::collections::BTreeSet::new();
        for entry in &self.sources {
            paths.extend(entry.source.list(prefix, suffix)?);
        }
        Ok(paths.into_iter().collect())
    }
}

/// Asset-source view that records one final resolution per logical path.
///
/// Preparation is deliberately single-threaded; interior mutability keeps the
/// ordinary `AssetSource` consumer contract while retaining exact named-source
/// provenance for every read.
pub struct ProvenanceTrackingAssetSource<'a> {
    chain: &'a AssetSourceChain,
    entries: RefCell<BTreeMap<AssetPath, AssetProvenanceEntry>>,
}

impl<'a> ProvenanceTrackingAssetSource<'a> {
    pub fn new(chain: &'a AssetSourceChain) -> Self {
        Self {
            chain,
            entries: RefCell::new(BTreeMap::new()),
        }
    }

    pub fn record_suppressed(&self, path: AssetPath, source: AssetResolutionOrigin) {
        self.entries.borrow_mut().insert(
            path.clone(),
            AssetProvenanceEntry {
                path,
                source,
                outcome: AssetResolutionOutcome::Suppressed,
            },
        );
    }

    pub fn resolved_origin(&self, path: &AssetPath) -> Option<AssetResolutionOrigin> {
        self.entries
            .borrow()
            .get(path)
            .filter(|entry| entry.outcome == AssetResolutionOutcome::Resolved)
            .map(|entry| entry.source.clone())
    }

    pub fn report(&self, epoch: u64, selection: AssetPackSelection) -> AssetProvenanceReport {
        let mut report = AssetProvenanceReport::new(epoch, selection);
        for entry in self.entries.borrow().values() {
            report.record(entry.clone());
        }
        report
    }
}

impl AssetSource for ProvenanceTrackingAssetSource<'_> {
    fn read(&self, path: &AssetPath) -> AssetResult<Option<Vec<u8>>> {
        let resolution = self.chain.resolve(path)?;
        let (bytes, source, outcome) = match resolution {
            Some(resolution) => (
                Some(resolution.bytes),
                resolution.source,
                AssetResolutionOutcome::Resolved,
            ),
            None => (
                None,
                AssetResolutionOrigin::anonymous(),
                AssetResolutionOutcome::Missing,
            ),
        };
        self.entries.borrow_mut().insert(
            path.clone(),
            AssetProvenanceEntry {
                path: path.clone(),
                source,
                outcome,
            },
        );
        Ok(bytes)
    }

    fn list(&self, prefix: &str, suffix: &str) -> AssetResult<Vec<AssetPath>> {
        self.chain.list(prefix, suffix)
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
    use crate::AssetPackDescriptor;

    #[cfg(not(target_arch = "wasm32"))]
    fn extracted_asset_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("reference/minecraft-1.17.1/extracted")
    }

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
    fn provenance_tracking_retains_named_winner_and_missing_lookup() {
        let authored_id = AssetPackId::new("authored");
        let fallback_id = AssetPackId::new("fallback");
        let authored = AssetPackDescriptor::new(
            authored_id.clone(),
            "Authored",
            AssetPackOrigin::FirstParty,
            10,
        )
        .unwrap();
        let fallback = AssetPackDescriptor::new(
            fallback_id.clone(),
            "Fallback",
            AssetPackOrigin::Generated,
            30,
        )
        .unwrap()
        .required();
        let catalog = AssetPackCatalog::new([authored, fallback]).unwrap();
        let selection = AssetPackSelection::new([authored_id.clone()]);
        let mut authored_source = MemoryAssetSource::new();
        let shared = AssetPath::new("assets/mclone/shared.png");
        authored_source.insert(shared.clone(), b"authored".to_vec());
        let mut fallback_source = MemoryAssetSource::new();
        fallback_source.insert(shared.clone(), b"fallback".to_vec());
        let chain = AssetSourceChain::from_selection(
            &catalog,
            &selection,
            [
                (
                    authored_id,
                    Box::new(authored_source) as Box<dyn AssetSource>,
                ),
                (
                    fallback_id,
                    Box::new(fallback_source) as Box<dyn AssetSource>,
                ),
            ],
        )
        .unwrap();
        let tracker = ProvenanceTrackingAssetSource::new(&chain);

        assert_eq!(tracker.read(&shared).unwrap().unwrap(), b"authored");
        assert!(
            tracker
                .read(&AssetPath::new("assets/mclone/missing.png"))
                .unwrap()
                .is_none()
        );
        let report = tracker.report(9, selection);

        assert_eq!(report.epoch, 9);
        assert_eq!(report.summary().first_party, 1);
        assert_eq!(report.summary().missing, 1);
        assert_eq!(report.summary().unknown, 0);
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

    #[test]
    fn selected_named_chain_cannot_resolve_from_disabled_reference_pack() {
        let authored_id = AssetPackId::new("mclone-authored");
        let reference_id = AssetPackId::new("minecraft-1.17.1-reference");
        let fallback_id = AssetPackId::new("mclone-generated");
        let catalog = AssetPackCatalog::new([
            crate::AssetPackDescriptor::new(
                authored_id.clone(),
                "Mclone authored",
                AssetPackOrigin::FirstParty,
                10,
            )
            .unwrap(),
            crate::AssetPackDescriptor::new(
                reference_id.clone(),
                "Minecraft reference",
                AssetPackOrigin::MinecraftReference,
                20,
            )
            .unwrap(),
            crate::AssetPackDescriptor::new(
                fallback_id.clone(),
                "Generated fallback",
                AssetPackOrigin::Generated,
                30,
            )
            .unwrap()
            .required(),
        ])
        .unwrap();
        let selection = AssetPackSelection::new([authored_id.clone()]);
        let path = AssetPath::new("assets/minecraft/textures/block/stone.png");
        let reference_only = AssetPath::new("assets/minecraft/reference-only.txt");

        let mut authored = MemoryAssetSource::new();
        authored.insert_text(path.clone(), "authored");
        let mut reference = MemoryAssetSource::new();
        reference.insert_text(path.clone(), "minecraft");
        reference.insert_text(reference_only.clone(), "must stay disabled");
        let mut fallback = MemoryAssetSource::new();
        fallback.insert_text(path.clone(), "generated");
        let chain = AssetSourceChain::from_selection(
            &catalog,
            &selection,
            [
                (
                    authored_id.clone(),
                    Box::new(authored) as Box<dyn AssetSource>,
                ),
                (reference_id, Box::new(reference) as Box<dyn AssetSource>),
                (fallback_id, Box::new(fallback) as Box<dyn AssetSource>),
            ],
        )
        .unwrap();

        let resolution = chain.resolve(&path).unwrap().unwrap();
        assert_eq!(resolution.bytes, b"authored");
        assert_eq!(resolution.source.pack_id, Some(authored_id));
        assert_eq!(resolution.source.origin, AssetPackOrigin::FirstParty);

        assert!(chain.resolve(&reference_only).unwrap().is_none());
        assert_eq!(chain.len(), 2, "disabled source was not admitted to chain");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn filesystem_source_reads_real_extracted_asset_when_present() {
        let root = extracted_asset_root();
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
