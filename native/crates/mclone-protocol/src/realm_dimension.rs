use std::error::Error;
use std::fmt;
use std::str::FromStr;

use mclone_core::ChunkPos;

pub const MAX_DIMENSION_KEY_BYTES: usize = 255;
pub const OVERWORLD_DIMENSION_KEY: &str = "minecraft:overworld";

/// Stable opaque identity for one durable realm/save.
///
/// Realm ids deliberately carry no storage path, process, catalog, or scene
/// meaning. Platform catalogs may associate their own keys with this id, but
/// those keys are not interchangeable with it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RealmId([u8; 16]);

impl RealmId {
    /// Compatibility identity for saves created before realm metadata exists.
    /// Slice 3 replaces this fallback with a persisted per-realm id.
    pub const LEGACY_SINGLE_REALM: Self = Self([
        0x6d, 0x63, 0x6c, 0x6f, 0x6e, 0x65, 0x00, 0x01, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01,
    ]);

    pub fn new(bytes: [u8; 16]) -> Result<Self, RealmIdError> {
        if bytes == [0; 16] {
            return Err(RealmIdError::Nil);
        }
        Ok(Self(bytes))
    }

    pub const fn bytes(self) -> [u8; 16] {
        self.0
    }
}

impl TryFrom<[u8; 16]> for RealmId {
    type Error = RealmIdError;

    fn try_from(bytes: [u8; 16]) -> Result<Self, Self::Error> {
        Self::new(bytes)
    }
}

impl fmt::Display for RealmId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.0;
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5],
            bytes[6],
            bytes[7],
            bytes[8],
            bytes[9],
            bytes[10],
            bytes[11],
            bytes[12],
            bytes[13],
            bytes[14],
            bytes[15],
        )
    }
}

impl FromStr for RealmId {
    type Err = RealmIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 36
            || value.as_bytes().get(8) != Some(&b'-')
            || value.as_bytes().get(13) != Some(&b'-')
            || value.as_bytes().get(18) != Some(&b'-')
            || value.as_bytes().get(23) != Some(&b'-')
        {
            return Err(RealmIdError::InvalidFormat);
        }
        let mut bytes = [0_u8; 16];
        let mut byte_index = 0;
        let mut high_nibble = None;
        for ch in value.bytes() {
            if ch == b'-' {
                continue;
            }
            let nibble = hex_nibble(ch).ok_or(RealmIdError::InvalidFormat)?;
            if let Some(high) = high_nibble.take() {
                bytes[byte_index] = (high << 4) | nibble;
                byte_index += 1;
            } else {
                high_nibble = Some(nibble);
            }
        }
        if byte_index != bytes.len() || high_nibble.is_some() {
            return Err(RealmIdError::InvalidFormat);
        }
        Self::new(bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealmIdError {
    Nil,
    InvalidFormat,
}

impl fmt::Display for RealmIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => f.write_str("realm id must not be nil"),
            Self::InvalidFormat => f.write_str("realm id must be a canonical UUID"),
        }
    }
}

impl Error for RealmIdError {}

/// Validated namespaced identity for one dimension instance within a realm.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DimensionKey(String);

impl DimensionKey {
    pub fn new(
        namespace: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> Result<Self, DimensionKeyError> {
        let namespace = namespace.as_ref();
        let path = path.as_ref();
        validate_dimension_key_parts(namespace, path)?;
        Ok(Self(format!("{namespace}:{path}")))
    }

    pub fn parse(value: impl AsRef<str>) -> Result<Self, DimensionKeyError> {
        let value = value.as_ref();
        if value.len() > MAX_DIMENSION_KEY_BYTES {
            return Err(DimensionKeyError::TooLong {
                len: value.len(),
                max: MAX_DIMENSION_KEY_BYTES,
            });
        }
        let Some((namespace, path)) = value.split_once(':') else {
            return Err(DimensionKeyError::MissingNamespace);
        };
        if path.contains(':') {
            return Err(DimensionKeyError::MultipleSeparators);
        }
        Self::new(namespace, path)
    }

    pub fn overworld() -> Self {
        Self(OVERWORLD_DIMENSION_KEY.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn namespace(&self) -> &str {
        self.0
            .split_once(':')
            .expect("validated dimension key must contain a namespace")
            .0
    }

    pub fn path(&self) -> &str {
        self.0
            .split_once(':')
            .expect("validated dimension key must contain a path")
            .1
    }
}

impl AsRef<str> for DimensionKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for DimensionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DimensionKey {
    type Err = DimensionKeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<String> for DimensionKey {
    type Error = DimensionKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DimensionKeyError {
    MissingNamespace,
    MultipleSeparators,
    EmptyNamespace,
    EmptyPath,
    InvalidNamespaceCharacter(char),
    InvalidPathCharacter(char),
    TooLong { len: usize, max: usize },
}

impl fmt::Display for DimensionKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNamespace => {
                f.write_str("dimension key must contain an explicit namespace")
            }
            Self::MultipleSeparators => {
                f.write_str("dimension key must contain exactly one ':' separator")
            }
            Self::EmptyNamespace => f.write_str("dimension key namespace must not be empty"),
            Self::EmptyPath => f.write_str("dimension key path must not be empty"),
            Self::InvalidNamespaceCharacter(ch) => {
                write!(f, "invalid dimension namespace character `{ch}`")
            }
            Self::InvalidPathCharacter(ch) => {
                write!(f, "invalid dimension path character `{ch}`")
            }
            Self::TooLong { len, max } => {
                write!(f, "dimension key is {len} bytes; maximum is {max}")
            }
        }
    }
}

impl Error for DimensionKeyError {}

/// A cross-dimension chunk address. Hot scheduler internals continue to use
/// plain `ChunkPos` within the owning dimension runtime.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DimensionChunkPos {
    pub dimension: DimensionKey,
    pub pos: ChunkPos,
}

impl DimensionChunkPos {
    pub const fn new(dimension: DimensionKey, pos: ChunkPos) -> Self {
        Self { dimension, pos }
    }
}

fn validate_dimension_key_parts(namespace: &str, path: &str) -> Result<(), DimensionKeyError> {
    if namespace.is_empty() {
        return Err(DimensionKeyError::EmptyNamespace);
    }
    if path.is_empty() {
        return Err(DimensionKeyError::EmptyPath);
    }
    if let Some(ch) = namespace.chars().find(|ch| !valid_namespace_char(*ch)) {
        return Err(DimensionKeyError::InvalidNamespaceCharacter(ch));
    }
    if let Some(ch) = path.chars().find(|ch| !valid_path_char(*ch)) {
        return Err(DimensionKeyError::InvalidPathCharacter(ch));
    }
    let len = namespace.len() + 1 + path.len();
    if len > MAX_DIMENSION_KEY_BYTES {
        return Err(DimensionKeyError::TooLong {
            len,
            max: MAX_DIMENSION_KEY_BYTES,
        });
    }
    Ok(())
}

fn valid_namespace_char(value: char) -> bool {
    matches!(value, '_' | '-' | '.' | 'a'..='z' | '0'..='9')
}

fn valid_path_char(value: char) -> bool {
    matches!(value, '_' | '-' | '/' | '.' | 'a'..='z' | '0'..='9')
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realm_id_roundtrips_canonical_uuid_text() {
        let id = RealmId::new([
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x81, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef,
        ])
        .unwrap();
        let encoded = "12345678-9abc-def0-8123-456789abcdef";
        assert_eq!(id.to_string(), encoded);
        assert_eq!(encoded.parse::<RealmId>().unwrap(), id);
        assert!(
            "00000000-0000-0000-0000-000000000000"
                .parse::<RealmId>()
                .is_err()
        );
    }

    #[test]
    fn dimension_key_requires_explicit_vanilla_shaped_namespace() {
        let overworld = DimensionKey::parse(OVERWORLD_DIMENSION_KEY).unwrap();
        assert_eq!(overworld, DimensionKey::overworld());
        assert_eq!(overworld.namespace(), "minecraft");
        assert_eq!(overworld.path(), "overworld");
        assert_eq!(
            DimensionKey::parse("mclone:planets/red_mars")
                .unwrap()
                .as_str(),
            "mclone:planets/red_mars"
        );

        assert_eq!(
            DimensionKey::parse("overworld"),
            Err(DimensionKeyError::MissingNamespace)
        );
        assert!(DimensionKey::parse("Minecraft:overworld").is_err());
        assert!(DimensionKey::parse("minecraft:the nether").is_err());
        assert!(DimensionKey::parse("minecraft:overworld:copy").is_err());
    }

    #[test]
    fn qualified_chunk_address_keeps_dimension_outside_hot_position() {
        let address = DimensionChunkPos::new(DimensionKey::overworld(), ChunkPos::new(3, -7));
        assert_eq!(address.dimension, DimensionKey::overworld());
        assert_eq!(address.pos, ChunkPos::new(3, -7));
    }
}
