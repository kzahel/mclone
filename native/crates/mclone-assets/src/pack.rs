use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use flate2::read::DeflateDecoder;
use serde::Deserialize;

use crate::{
    AssetError, AssetPackDescriptor, AssetPackDiscovery, AssetPackId, AssetPackOrigin,
    AssetPackRole, AssetPath, AssetResult, AssetSource,
};

pub const PACK_FORMAT_VERSION: u32 = 1;
pub const DEFAULT_PACK_MANIFEST_PATH: &str = "mclone-pack.json";

const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0201_4b50;
const LOCAL_FILE_SIGNATURE: u32 = 0x0403_4b50;
const EOCD_FIXED_LEN: usize = 22;
const CENTRAL_DIRECTORY_FIXED_LEN: usize = 46;
const LOCAL_FILE_FIXED_LEN: usize = 30;
const ZIP64_SENTINEL_16: u16 = u16::MAX;
const ZIP64_SENTINEL_32: u32 = u32::MAX;
const METHOD_STORE: u16 = 0;
const METHOD_DEFLATE: u16 = 8;
const GENERAL_PURPOSE_ENCRYPTED: u16 = 1 << 0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPackManifest {
    pub format_version: u32,
    pub asset_set: String,
    pub file_count: usize,
    pub pack_id: Option<AssetPackId>,
    pub display_name: Option<String>,
    pub origin: AssetPackOrigin,
    pub roles: BTreeSet<AssetPackRole>,
    pub asset_schema: Option<String>,
    pub content_fingerprint: Option<String>,
}

impl AssetPackManifest {
    pub fn descriptor(
        &self,
        discovery: AssetPackDiscovery,
        priority: u16,
    ) -> AssetResult<AssetPackDescriptor> {
        let origin = match self.origin {
            AssetPackOrigin::Unknown => discovery.trusted_origin.unwrap_or_default(),
            declared => declared,
        };
        let roles = if self.roles.is_empty() {
            discovery.trusted_roles
        } else {
            self.roles.clone()
        };
        let mut descriptor = AssetPackDescriptor::new(
            self.pack_id.clone().unwrap_or(discovery.fallback_id),
            self.display_name
                .clone()
                .unwrap_or(discovery.fallback_display_name),
            origin,
            priority,
        )?
        .with_roles(roles);
        descriptor.asset_schema.clone_from(&self.asset_schema);
        descriptor
            .content_fingerprint
            .clone_from(&self.content_fingerprint);
        Ok(descriptor)
    }
}

#[derive(Clone, Debug)]
pub struct PackedAssetSource {
    bytes: Arc<[u8]>,
    manifest: AssetPackManifest,
    entries: BTreeMap<AssetPath, ZipEntry>,
}

impl PackedAssetSource {
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> AssetResult<Self> {
        let bytes: Arc<[u8]> = Arc::from(bytes.into());
        let zip_entries = parse_central_directory(&bytes)?;
        let manifest_entry = zip_entries.get(DEFAULT_PACK_MANIFEST_PATH).ok_or_else(|| {
            pack_error(format!(
                "missing required pack manifest `{DEFAULT_PACK_MANIFEST_PATH}`"
            ))
        })?;
        let manifest_bytes = read_zip_entry_data(&bytes, manifest_entry)?;
        let raw_manifest: RawAssetPackManifest =
            serde_json::from_slice(&manifest_bytes).map_err(|source| AssetError::Json {
                path: AssetPath::new(DEFAULT_PACK_MANIFEST_PATH),
                source,
            })?;
        validate_raw_manifest(
            raw_manifest,
            &zip_entries,
            bytes,
            DEFAULT_PACK_MANIFEST_PATH,
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file(path: impl AsRef<Path>) -> AssetResult<Self> {
        Self::from_bytes(std::fs::read(path)?)
    }

    pub fn manifest(&self) -> &AssetPackManifest {
        &self.manifest
    }

    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    pub fn contains(&self, path: &AssetPath) -> bool {
        self.entries.contains_key(path)
    }
}

impl AssetSource for PackedAssetSource {
    fn read(&self, path: &AssetPath) -> AssetResult<Option<Vec<u8>>> {
        let Some(entry) = self.entries.get(path) else {
            return Ok(None);
        };
        Ok(Some(read_zip_entry_data(&self.bytes, entry)?))
    }

    fn list(&self, prefix: &str, suffix: &str) -> AssetResult<Vec<AssetPath>> {
        Ok(self
            .entries
            .keys()
            .filter(|path| path.as_str().starts_with(prefix) && path.as_str().ends_with(suffix))
            .cloned()
            .collect())
    }
}

#[derive(Clone, Debug)]
struct ZipEntry {
    name: String,
    method: u16,
    flags: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    local_header_offset: u32,
}

#[derive(Debug, Deserialize)]
struct RawAssetPackManifest {
    format_version: u32,
    asset_set: String,
    file_count: usize,
    pack_id: Option<String>,
    display_name: Option<String>,
    #[serde(default)]
    origin: AssetPackOrigin,
    #[serde(default)]
    roles: BTreeSet<AssetPackRole>,
    asset_schema: Option<String>,
    content_fingerprint: Option<String>,
    files: Vec<RawAssetPackFile>,
}

#[derive(Debug, Deserialize)]
struct RawAssetPackFile {
    path: String,
    bytes: u64,
    compression: String,
    #[allow(dead_code)]
    sha256: Option<String>,
}

fn validate_raw_manifest(
    raw: RawAssetPackManifest,
    zip_entries: &BTreeMap<String, ZipEntry>,
    bytes: Arc<[u8]>,
    manifest_path: &str,
) -> AssetResult<PackedAssetSource> {
    if raw.format_version != PACK_FORMAT_VERSION {
        return Err(pack_error(format!(
            "unsupported format_version {}; expected {PACK_FORMAT_VERSION}",
            raw.format_version
        )));
    }
    if raw.asset_set.is_empty() {
        return Err(pack_error("manifest asset_set must not be empty"));
    }
    if raw
        .display_name
        .as_ref()
        .is_some_and(|display_name| display_name.trim().is_empty())
    {
        return Err(pack_error("manifest display_name must not be empty"));
    }
    if raw
        .asset_schema
        .as_ref()
        .is_some_and(|schema| schema.trim().is_empty())
    {
        return Err(pack_error("manifest asset_schema must not be empty"));
    }
    if raw
        .content_fingerprint
        .as_ref()
        .is_some_and(|fingerprint| fingerprint.trim().is_empty())
    {
        return Err(pack_error("manifest content_fingerprint must not be empty"));
    }
    if raw.file_count != raw.files.len() {
        return Err(pack_error(format!(
            "manifest file_count {} does not match {} file records",
            raw.file_count,
            raw.files.len()
        )));
    }

    let mut entries = BTreeMap::new();
    let mut declared_names = BTreeSet::from([manifest_path.to_owned()]);
    for file in raw.files {
        let path = AssetPath::try_new(file.path)?;
        if !declared_names.insert(path.as_str().to_owned()) {
            return Err(pack_error(format!(
                "duplicate manifest file path `{}`",
                path.as_str()
            )));
        }
        let entry = zip_entries.get(path.as_str()).ok_or_else(|| {
            pack_error(format!(
                "manifest file `{}` is missing from archive",
                path.as_str()
            ))
        })?;
        if u64::from(entry.uncompressed_size) != file.bytes {
            return Err(pack_error(format!(
                "manifest byte count for `{}` is {}, archive has {}",
                path.as_str(),
                file.bytes,
                entry.uncompressed_size
            )));
        }
        if compression_name(entry.method) != file.compression {
            return Err(pack_error(format!(
                "manifest compression for `{}` is `{}`, archive uses `{}`",
                path.as_str(),
                file.compression,
                compression_name(entry.method)
            )));
        }
        entries.insert(path, entry.clone());
    }

    for name in zip_entries.keys() {
        if !declared_names.contains(name) {
            return Err(pack_error(format!(
                "archive entry `{name}` is not declared in {manifest_path}"
            )));
        }
    }

    let manifest = AssetPackManifest {
        format_version: raw.format_version,
        asset_set: raw.asset_set,
        file_count: raw.file_count,
        pack_id: raw.pack_id.map(AssetPackId::try_new).transpose()?,
        display_name: raw.display_name,
        origin: raw.origin,
        roles: raw.roles,
        asset_schema: raw.asset_schema,
        content_fingerprint: raw.content_fingerprint,
    };
    Ok(PackedAssetSource {
        bytes,
        manifest,
        entries,
    })
}

fn parse_central_directory(bytes: &[u8]) -> AssetResult<BTreeMap<String, ZipEntry>> {
    let eocd = find_eocd(bytes)?;
    if read_u16(bytes, eocd + 4)? != 0 || read_u16(bytes, eocd + 6)? != 0 {
        return Err(pack_error("multi-disk ZIP packs are not supported"));
    }

    let disk_entry_count = read_u16(bytes, eocd + 8)?;
    let total_entry_count = read_u16(bytes, eocd + 10)?;
    let central_size = read_u32(bytes, eocd + 12)?;
    let central_offset = read_u32(bytes, eocd + 16)?;
    if disk_entry_count == ZIP64_SENTINEL_16
        || total_entry_count == ZIP64_SENTINEL_16
        || central_size == ZIP64_SENTINEL_32
        || central_offset == ZIP64_SENTINEL_32
    {
        return Err(pack_error("ZIP64 asset packs are not supported"));
    }
    if disk_entry_count != total_entry_count {
        return Err(pack_error(
            "split central directory entries are not supported",
        ));
    }

    let central_offset = central_offset as usize;
    let central_end = checked_add(central_offset, central_size as usize, "central directory")?;
    if central_end > bytes.len() {
        return Err(pack_error("central directory extends past end of pack"));
    }

    let mut entries = BTreeMap::new();
    let mut offset = central_offset;
    for _ in 0..total_entry_count {
        if read_u32(bytes, offset)? != CENTRAL_DIRECTORY_SIGNATURE {
            return Err(pack_error(format!(
                "bad central directory signature at byte {offset}"
            )));
        }
        let flags = read_u16(bytes, offset + 8)?;
        if flags & GENERAL_PURPOSE_ENCRYPTED != 0 {
            return Err(pack_error("encrypted ZIP entries are not supported"));
        }
        let method = read_u16(bytes, offset + 10)?;
        if method != METHOD_STORE && method != METHOD_DEFLATE {
            return Err(pack_error(format!(
                "unsupported ZIP compression method {method}"
            )));
        }
        let crc32 = read_u32(bytes, offset + 16)?;
        let compressed_size = read_u32(bytes, offset + 20)?;
        let uncompressed_size = read_u32(bytes, offset + 24)?;
        let file_name_len = read_u16(bytes, offset + 28)? as usize;
        let extra_len = read_u16(bytes, offset + 30)? as usize;
        let comment_len = read_u16(bytes, offset + 32)? as usize;
        let local_header_offset = read_u32(bytes, offset + 42)?;
        if compressed_size == ZIP64_SENTINEL_32
            || uncompressed_size == ZIP64_SENTINEL_32
            || local_header_offset == ZIP64_SENTINEL_32
        {
            return Err(pack_error("ZIP64 asset pack entries are not supported"));
        }

        let name_start = offset + CENTRAL_DIRECTORY_FIXED_LEN;
        let name_end = checked_add(name_start, file_name_len, "central file name")?;
        if name_end > central_end {
            return Err(pack_error(
                "central file name extends past central directory",
            ));
        }
        let name = std::str::from_utf8(&bytes[name_start..name_end])
            .map_err(|_| pack_error("ZIP entry names must be valid UTF-8"))?
            .to_owned();
        if name.ends_with('/') {
            return Err(pack_error(format!(
                "directory entry `{name}` is not allowed in asset packs"
            )));
        }
        if name != DEFAULT_PACK_MANIFEST_PATH {
            AssetPath::try_new(name.clone())?;
        }

        let next = checked_add(
            name_end,
            checked_add(extra_len, comment_len, "central extra/comment")?,
            "central entry",
        )?;
        if next > central_end {
            return Err(pack_error("central entry extends past central directory"));
        }
        let entry = ZipEntry {
            name: name.clone(),
            method,
            flags,
            crc32,
            compressed_size,
            uncompressed_size,
            local_header_offset,
        };
        if entries.insert(name.clone(), entry).is_some() {
            return Err(pack_error(format!("duplicate ZIP entry `{name}`")));
        }
        offset = next;
    }
    if offset != central_end {
        return Err(pack_error("central directory contains trailing bytes"));
    }

    Ok(entries)
}

fn read_zip_entry_data(bytes: &[u8], entry: &ZipEntry) -> AssetResult<Vec<u8>> {
    let payload = zip_entry_payload(bytes, entry)?;
    let mut data = match entry.method {
        METHOD_STORE => payload.to_vec(),
        METHOD_DEFLATE => {
            let mut decoder = DeflateDecoder::new(payload);
            let mut decoded = Vec::with_capacity(entry.uncompressed_size as usize);
            decoder.read_to_end(&mut decoded).map_err(AssetError::Io)?;
            decoded
        }
        method => {
            return Err(pack_error(format!(
                "unsupported ZIP compression method {method}"
            )));
        }
    };
    if data.len() != entry.uncompressed_size as usize {
        return Err(pack_error(format!(
            "entry `{}` decoded to {} bytes, expected {}",
            entry.name,
            data.len(),
            entry.uncompressed_size
        )));
    }
    let crc = crc32fast::hash(&data);
    if crc != entry.crc32 {
        data.clear();
        return Err(pack_error(format!("entry `{}` CRC mismatch", entry.name)));
    }
    Ok(data)
}

fn zip_entry_payload<'a>(bytes: &'a [u8], entry: &ZipEntry) -> AssetResult<&'a [u8]> {
    let offset = entry.local_header_offset as usize;
    if read_u32(bytes, offset)? != LOCAL_FILE_SIGNATURE {
        return Err(pack_error(format!(
            "bad local file header signature for `{}`",
            entry.name
        )));
    }
    let flags = read_u16(bytes, offset + 6)?;
    if flags != entry.flags {
        return Err(pack_error(format!(
            "local flags for `{}` do not match central directory",
            entry.name
        )));
    }
    let method = read_u16(bytes, offset + 8)?;
    if method != entry.method {
        return Err(pack_error(format!(
            "local compression method for `{}` does not match central directory",
            entry.name
        )));
    }
    let file_name_len = read_u16(bytes, offset + 26)? as usize;
    let extra_len = read_u16(bytes, offset + 28)? as usize;
    let name_start = offset + LOCAL_FILE_FIXED_LEN;
    let name_end = checked_add(name_start, file_name_len, "local file name")?;
    let data_start = checked_add(name_end, extra_len, "local extra")?;
    let data_end = checked_add(
        data_start,
        entry.compressed_size as usize,
        "local compressed payload",
    )?;
    if data_end > bytes.len() {
        return Err(pack_error(format!(
            "entry `{}` payload extends past end of pack",
            entry.name
        )));
    }
    let local_name = std::str::from_utf8(&bytes[name_start..name_end])
        .map_err(|_| pack_error("local ZIP entry names must be valid UTF-8"))?;
    if local_name != entry.name {
        return Err(pack_error(format!(
            "local file name `{local_name}` does not match central directory `{}`",
            entry.name
        )));
    }
    Ok(&bytes[data_start..data_end])
}

fn find_eocd(bytes: &[u8]) -> AssetResult<usize> {
    if bytes.len() < EOCD_FIXED_LEN {
        return Err(pack_error("asset pack is too small to be a ZIP archive"));
    }
    let min = bytes
        .len()
        .saturating_sub(EOCD_FIXED_LEN + u16::MAX as usize);
    for offset in (min..=bytes.len() - EOCD_FIXED_LEN).rev() {
        if read_u32(bytes, offset)? == EOCD_SIGNATURE {
            let comment_len = read_u16(bytes, offset + 20)? as usize;
            if offset + EOCD_FIXED_LEN + comment_len == bytes.len() {
                return Ok(offset);
            }
        }
    }
    Err(pack_error("could not find ZIP end of central directory"))
}

fn compression_name(method: u16) -> &'static str {
    match method {
        METHOD_STORE => "store",
        METHOD_DEFLATE => "deflate",
        _ => "unknown",
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> AssetResult<u16> {
    let chunk = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| pack_error("unexpected end of asset pack"))?;
    Ok(u16::from_le_bytes([chunk[0], chunk[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> AssetResult<u32> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| pack_error("unexpected end of asset pack"))?;
    Ok(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

fn checked_add(left: usize, right: usize, label: &str) -> AssetResult<usize> {
    left.checked_add(right)
        .ok_or_else(|| pack_error(format!("{label} offset overflow")))
}

fn pack_error(message: impl Into<String>) -> AssetError {
    AssetError::InvalidAssetPack(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    #[test]
    fn stored_pack_reads_manifest_declared_file() {
        let bytes = test_zip(vec![
            test_entry(
                DEFAULT_PACK_MANIFEST_PATH,
                manifest_bytes(vec![(
                    "assets/minecraft/blockstates/stone.json",
                    "{}",
                    "store",
                )]),
                METHOD_DEFLATE,
            ),
            test_entry(
                "assets/minecraft/blockstates/stone.json",
                b"{}".to_vec(),
                METHOD_STORE,
            ),
        ]);

        let source = PackedAssetSource::from_bytes(bytes).unwrap();

        assert_eq!(source.manifest().asset_set, "mclone-test");
        assert_eq!(source.manifest().pack_id, None);
        assert_eq!(source.manifest().origin, AssetPackOrigin::Unknown);
        assert!(source.manifest().roles.is_empty());
        assert_eq!(source.file_count(), 1);
        assert_eq!(
            source
                .read(&AssetPath::new("assets/minecraft/blockstates/stone.json"))
                .unwrap(),
            Some(b"{}".to_vec())
        );
    }

    #[test]
    fn manifest_parses_optional_pack_identity_and_provenance() {
        let manifest = br#"{
            "format_version": 1,
            "asset_set": "mclone-authored",
            "file_count": 0,
            "pack_id": "mclone-authored",
            "display_name": "Mclone Original Assets",
            "origin": "first_party",
            "roles": ["authored_override", "render_content"],
            "asset_schema": "mclone-visuals-v1",
            "content_fingerprint": "sha256:test",
            "files": []
        }"#
        .to_vec();
        let bytes = test_zip(vec![test_entry(
            DEFAULT_PACK_MANIFEST_PATH,
            manifest,
            METHOD_DEFLATE,
        )]);

        let source = PackedAssetSource::from_bytes(bytes).unwrap();
        let manifest = source.manifest();

        assert_eq!(manifest.pack_id, Some(AssetPackId::new("mclone-authored")));
        assert_eq!(
            manifest.display_name.as_deref(),
            Some("Mclone Original Assets")
        );
        assert_eq!(manifest.origin, AssetPackOrigin::FirstParty);
        assert_eq!(
            manifest.roles,
            BTreeSet::from([
                AssetPackRole::AuthoredOverride,
                AssetPackRole::RenderContent,
            ])
        );
        assert_eq!(manifest.asset_schema.as_deref(), Some("mclone-visuals-v1"));
        assert_eq!(manifest.content_fingerprint.as_deref(), Some("sha256:test"));
    }

    #[test]
    fn legacy_manifest_stays_unknown_without_trusted_discovery_identity() {
        let bytes = test_zip(vec![test_entry(
            DEFAULT_PACK_MANIFEST_PATH,
            manifest_bytes(vec![]),
            METHOD_DEFLATE,
        )]);
        let source = PackedAssetSource::from_bytes(bytes).unwrap();

        let untrusted = source
            .manifest()
            .descriptor(
                AssetPackDiscovery::untrusted(AssetPackId::new("legacy-pack"), "Legacy pack"),
                20,
            )
            .unwrap();
        assert_eq!(untrusted.origin, AssetPackOrigin::Unknown);

        let trusted = source
            .manifest()
            .descriptor(
                AssetPackDiscovery::trusted(
                    AssetPackId::new("minecraft-1.17.1-reference"),
                    "Minecraft 1.17.1 Reference",
                    AssetPackOrigin::MinecraftReference,
                    [AssetPackRole::ReferenceBase],
                ),
                20,
            )
            .unwrap();
        assert_eq!(trusted.origin, AssetPackOrigin::MinecraftReference);
        assert!(trusted.roles.contains(&AssetPackRole::ReferenceBase));
    }

    #[test]
    fn deflated_pack_reads_manifest_declared_file() {
        let bytes = test_zip(vec![
            test_entry(
                DEFAULT_PACK_MANIFEST_PATH,
                manifest_bytes(vec![(
                    "assets/minecraft/models/block/stone.json",
                    "{\"parent\":\"minecraft:block/cube_all\"}",
                    "deflate",
                )]),
                METHOD_DEFLATE,
            ),
            test_entry(
                "assets/minecraft/models/block/stone.json",
                br#"{"parent":"minecraft:block/cube_all"}"#.to_vec(),
                METHOD_DEFLATE,
            ),
        ]);

        let source = PackedAssetSource::from_bytes(bytes).unwrap();

        assert_eq!(
            source
                .read(&AssetPath::new("assets/minecraft/models/block/stone.json"))
                .unwrap(),
            Some(br#"{"parent":"minecraft:block/cube_all"}"#.to_vec())
        );
    }

    #[test]
    fn pack_lists_manifest_declared_files() {
        let bytes = test_zip(vec![
            test_entry(
                DEFAULT_PACK_MANIFEST_PATH,
                manifest_bytes(vec![
                    ("assets/minecraft/blockstates/dirt.json", "{}", "store"),
                    ("assets/minecraft/blockstates/stone.json", "{}", "store"),
                    ("assets/minecraft/models/block/stone.json", "{}", "store"),
                ]),
                METHOD_DEFLATE,
            ),
            test_entry(
                "assets/minecraft/blockstates/dirt.json",
                b"{}".to_vec(),
                METHOD_STORE,
            ),
            test_entry(
                "assets/minecraft/blockstates/stone.json",
                b"{}".to_vec(),
                METHOD_STORE,
            ),
            test_entry(
                "assets/minecraft/models/block/stone.json",
                b"{}".to_vec(),
                METHOD_STORE,
            ),
        ]);
        let source = PackedAssetSource::from_bytes(bytes).unwrap();

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
    fn pack_rejects_undeclared_archive_entries() {
        let bytes = test_zip(vec![
            test_entry(
                DEFAULT_PACK_MANIFEST_PATH,
                manifest_bytes(vec![]),
                METHOD_DEFLATE,
            ),
            test_entry(
                "assets/minecraft/blockstates/stone.json",
                b"{}".to_vec(),
                METHOD_STORE,
            ),
        ]);

        let error = PackedAssetSource::from_bytes(bytes)
            .unwrap_err()
            .to_string();

        assert!(error.contains("not declared"));
    }

    #[test]
    fn pack_rejects_bad_crc_on_read() {
        let mut bytes = test_zip(vec![
            test_entry(
                DEFAULT_PACK_MANIFEST_PATH,
                manifest_bytes(vec![(
                    "assets/minecraft/blockstates/stone.json",
                    "{}",
                    "store",
                )]),
                METHOD_DEFLATE,
            ),
            test_entry(
                "assets/minecraft/blockstates/stone.json",
                b"{}".to_vec(),
                METHOD_STORE,
            ),
        ]);
        let needle = b"{}";
        let offset = bytes
            .windows(needle.len())
            .position(|window| window == needle)
            .unwrap();
        bytes[offset] = b'{';
        bytes[offset + 1] = b']';
        let source = PackedAssetSource::from_bytes(bytes).unwrap();

        let error = source
            .read(&AssetPath::new("assets/minecraft/blockstates/stone.json"))
            .unwrap_err()
            .to_string();

        assert!(error.contains("CRC mismatch"));
    }

    #[derive(Clone)]
    struct TestEntry {
        name: String,
        data: Vec<u8>,
        method: u16,
    }

    fn test_entry(name: &str, data: Vec<u8>, method: u16) -> TestEntry {
        TestEntry {
            name: name.to_owned(),
            data,
            method,
        }
    }

    fn manifest_bytes(files: Vec<(&str, &str, &str)>) -> Vec<u8> {
        let files = files
            .into_iter()
            .map(|(path, data, compression)| {
                format!(
                    r#"{{"path":"{path}","bytes":{},"sha256":"test","compression":"{compression}"}}"#,
                    data.len()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"format_version":1,"asset_set":"mclone-test","file_count":{},"files":[{}]}}"#,
            if files.is_empty() {
                0
            } else {
                files.split("},{").count()
            },
            files
        )
        .into_bytes()
    }

    fn test_zip(entries: Vec<TestEntry>) -> Vec<u8> {
        let mut output = Vec::new();
        let mut central = Vec::new();
        for entry in entries {
            let payload = if entry.method == METHOD_DEFLATE {
                let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(&entry.data).unwrap();
                encoder.finish().unwrap()
            } else {
                entry.data.clone()
            };
            let local_offset = output.len() as u32;
            let crc32 = crc32fast::hash(&entry.data);
            write_u32(&mut output, LOCAL_FILE_SIGNATURE);
            write_u16(&mut output, 20);
            write_u16(&mut output, 0);
            write_u16(&mut output, entry.method);
            write_u16(&mut output, 0);
            write_u16(&mut output, 0);
            write_u32(&mut output, crc32);
            write_u32(&mut output, payload.len() as u32);
            write_u32(&mut output, entry.data.len() as u32);
            write_u16(&mut output, entry.name.len() as u16);
            write_u16(&mut output, 0);
            output.extend_from_slice(entry.name.as_bytes());
            output.extend_from_slice(&payload);

            write_u32(&mut central, CENTRAL_DIRECTORY_SIGNATURE);
            write_u16(&mut central, 20);
            write_u16(&mut central, 20);
            write_u16(&mut central, 0);
            write_u16(&mut central, entry.method);
            write_u16(&mut central, 0);
            write_u16(&mut central, 0);
            write_u32(&mut central, crc32);
            write_u32(&mut central, payload.len() as u32);
            write_u32(&mut central, entry.data.len() as u32);
            write_u16(&mut central, entry.name.len() as u16);
            write_u16(&mut central, 0);
            write_u16(&mut central, 0);
            write_u16(&mut central, 0);
            write_u16(&mut central, 0);
            write_u32(&mut central, 0o644 << 16);
            write_u32(&mut central, local_offset);
            central.extend_from_slice(entry.name.as_bytes());
        }

        let central_offset = output.len() as u32;
        let central_size = central.len() as u32;
        let entry_count = count_central_entries(&central) as u16;
        output.extend_from_slice(&central);
        write_u32(&mut output, EOCD_SIGNATURE);
        write_u16(&mut output, 0);
        write_u16(&mut output, 0);
        write_u16(&mut output, entry_count);
        write_u16(&mut output, entry_count);
        write_u32(&mut output, central_size);
        write_u32(&mut output, central_offset);
        write_u16(&mut output, 0);
        output
    }

    fn count_central_entries(central: &[u8]) -> usize {
        central
            .windows(4)
            .filter(|window| *window == CENTRAL_DIRECTORY_SIGNATURE.to_le_bytes())
            .count()
    }

    fn write_u16(output: &mut Vec<u8>, value: u16) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u32(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_le_bytes());
    }
}
