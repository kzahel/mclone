use mclone_app_runtime::world_catalog::{
    LOCAL_WORLD_CATALOG_SCHEMA_VERSION, LOCAL_WORLD_TARGET_MINECRAFT_VERSION, LocalWorldId,
    LocalWorldSummary,
};
use mclone_server::{StarterContentDescriptor, WorldGenerationProfile};

const MAGIC: &[u8; 4] = b"MCWC";
const VERSION: u8 = 2;
const MAX_STRING_BYTES: usize = u16::MAX as usize;

pub(crate) fn encode(summary: &LocalWorldSummary) -> Result<Vec<u8>, String> {
    let mut frame = Vec::new();
    frame.extend_from_slice(MAGIC);
    frame.push(VERSION);
    encode_string(&mut frame, summary.id.as_str(), "world id")?;
    encode_string(&mut frame, &summary.display_name, "display name")?;
    frame.extend_from_slice(&summary.seed.to_le_bytes());
    encode_string(
        &mut frame,
        summary.world_generation_profile.label(),
        "generation profile",
    )?;
    encode_string(&mut frame, summary.starter_content.id(), "starter content")?;
    frame.extend_from_slice(&summary.created_unix_millis.to_le_bytes());
    encode_optional_u64(&mut frame, summary.last_played_unix_millis);
    frame.extend_from_slice(&summary.storage_schema_version.to_le_bytes());
    encode_string(
        &mut frame,
        &summary.target_minecraft_version,
        "target Minecraft version",
    )?;
    encode_optional_string(
        &mut frame,
        summary.mclone_version.as_deref(),
        "Mclone version",
    )?;
    encode_optional_string(
        &mut frame,
        summary.backend_label.as_deref(),
        "backend label",
    )?;
    frame.push(u8::from(summary.locked));
    Ok(frame)
}

pub(crate) fn decode(frame: &[u8]) -> Result<LocalWorldSummary, String> {
    let mut decoder = Decoder::new(frame);
    if decoder.bytes(MAGIC.len())? != MAGIC {
        return Err("world catalog descriptor has invalid magic".to_owned());
    }
    let version = decoder.u8()?;
    if !(1..=VERSION).contains(&version) {
        return Err(format!(
            "world catalog descriptor version {version} is unsupported"
        ));
    }

    let id = LocalWorldId::new(decoder.string("world id")?)
        .map_err(|error| format!("world catalog descriptor: {}", error.message))?;
    let display_name = decoder.string("display name")?;
    let seed = decoder.i64()?;
    let generation_profile =
        WorldGenerationProfile::parse_label(&decoder.string("generation profile")?)
            .map_err(|error| format!("world catalog descriptor: {error}"))?;
    let starter_content = if version >= 2 {
        match decoder.string("starter content")?.as_str() {
            "wild" => StarterContentDescriptor::Wild,
            "intro-homestead-v1" => StarterContentDescriptor::IntroHomesteadV1,
            value => {
                return Err(format!(
                    "world catalog descriptor has unknown starter content `{value}`"
                ));
            }
        }
    } else {
        StarterContentDescriptor::Wild
    };
    let created_unix_millis = decoder.u64()?;
    let last_played_unix_millis = decoder.optional_u64("last played timestamp")?;
    let storage_schema_version = decoder.u32()?;
    let target_minecraft_version = decoder.string("target Minecraft version")?;
    let mclone_version = decoder.optional_string("Mclone version")?;
    let backend_label = decoder.optional_string("backend label")?;
    let locked = decoder.bool("locked")?;
    decoder.finish()?;

    let mut summary = LocalWorldSummary::new(id, display_name, seed, created_unix_millis)
        .map_err(|error| format!("world catalog descriptor: {}", error.message))?;
    summary.world_generation_profile = generation_profile;
    summary.starter_content = starter_content;
    summary.last_played_unix_millis = last_played_unix_millis;
    summary.storage_schema_version = storage_schema_version;
    summary.target_minecraft_version = target_minecraft_version;
    summary.mclone_version = mclone_version;
    summary.backend_label = backend_label;
    summary.locked = locked;
    summary.compatible = summary.storage_schema_version == LOCAL_WORLD_CATALOG_SCHEMA_VERSION
        && summary.target_minecraft_version == LOCAL_WORLD_TARGET_MINECRAFT_VERSION;
    Ok(summary)
}

fn encode_string(frame: &mut Vec<u8>, value: &str, label: &str) -> Result<(), String> {
    let length = u16::try_from(value.len()).map_err(|_| {
        format!("world catalog descriptor {label} exceeds {MAX_STRING_BYTES} UTF-8 bytes")
    })?;
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(value.as_bytes());
    Ok(())
}

fn encode_optional_string(
    frame: &mut Vec<u8>,
    value: Option<&str>,
    label: &str,
) -> Result<(), String> {
    match value {
        Some(value) => {
            frame.push(1);
            encode_string(frame, value, label)
        }
        None => {
            frame.push(0);
            Ok(())
        }
    }
}

fn encode_optional_u64(frame: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            frame.push(1);
            frame.extend_from_slice(&value.to_le_bytes());
        }
        None => frame.push(0),
    }
}

struct Decoder<'a> {
    frame: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    fn new(frame: &'a [u8]) -> Self {
        Self { frame, cursor: 0 }
    }

    fn bytes(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or_else(|| "world catalog descriptor length overflow".to_owned())?;
        let bytes = self
            .frame
            .get(self.cursor..end)
            .ok_or_else(|| "world catalog descriptor is truncated".to_owned())?;
        self.cursor = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            self.bytes(2)?.try_into().expect("fixed-width slice"),
        ))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?.try_into().expect("fixed-width slice"),
        ))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(
            self.bytes(8)?.try_into().expect("fixed-width slice"),
        ))
    }

    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(
            self.bytes(8)?.try_into().expect("fixed-width slice"),
        ))
    }

    fn bool(&mut self, label: &str) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(format!(
                "world catalog descriptor {label} flag {value} is invalid"
            )),
        }
    }

    fn string(&mut self, label: &str) -> Result<String, String> {
        let length = usize::from(self.u16()?);
        let bytes = self.bytes(length)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| format!("world catalog descriptor {label} is not valid UTF-8"))
    }

    fn optional_string(&mut self, label: &str) -> Result<Option<String>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.string(label).map(Some),
            value => Err(format!(
                "world catalog descriptor {label} presence flag {value} is invalid"
            )),
        }
    }

    fn optional_u64(&mut self, label: &str) -> Result<Option<u64>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.u64().map(Some),
            value => Err(format!(
                "world catalog descriptor {label} presence flag {value} is invalid"
            )),
        }
    }

    fn finish(self) -> Result<(), String> {
        if self.cursor == self.frame.len() {
            Ok(())
        } else {
            Err("world catalog descriptor has trailing bytes".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary() -> LocalWorldSummary {
        let mut summary = LocalWorldSummary::new(
            LocalWorldId::new("descriptor-world").unwrap(),
            "Descriptor World",
            i64::MIN + 17,
            9_007_199_254_740_993,
        )
        .unwrap();
        summary.world_generation_profile = WorldGenerationProfile::alpha_v1(true);
        summary.starter_content = StarterContentDescriptor::IntroHomesteadV1;
        summary.last_played_unix_millis = Some(u64::MAX - 7);
        summary.mclone_version = Some("0.1.0-test".to_owned());
        summary.backend_label = Some("web-indexeddb".to_owned());
        summary.locked = true;
        summary
    }

    #[test]
    fn descriptor_round_trips_exact_integer_and_optional_fields() {
        let summary = summary();
        assert_eq!(decode(&encode(&summary).unwrap()).unwrap(), summary);
    }

    #[test]
    fn descriptor_round_trips_mclone_overworld_v3() {
        let mut summary = summary();
        summary.world_generation_profile = WorldGenerationProfile::McloneOverworldV3;
        assert_eq!(decode(&encode(&summary).unwrap()).unwrap(), summary);
    }

    #[test]
    fn version_one_descriptor_defaults_to_wild_start() {
        let mut expected = summary();
        expected.starter_content = StarterContentDescriptor::Wild;
        let mut frame = encode(&expected).unwrap();
        let mut decoder = Decoder::new(&frame);
        decoder.bytes(MAGIC.len()).unwrap();
        decoder.u8().unwrap();
        decoder.string("world id").unwrap();
        decoder.string("display name").unwrap();
        decoder.i64().unwrap();
        decoder.string("generation profile").unwrap();
        let starter_start = decoder.cursor;
        decoder.string("starter content").unwrap();
        let starter_end = decoder.cursor;
        frame.drain(starter_start..starter_end);
        frame[MAGIC.len()] = 1;

        assert_eq!(decode(&frame).unwrap(), expected);
    }

    #[test]
    fn descriptor_recomputes_compatibility_instead_of_persisting_it() {
        let mut summary = summary();
        summary.storage_schema_version += 1;
        summary.compatible = true;
        let decoded = decode(&encode(&summary).unwrap()).unwrap();
        assert!(!decoded.compatible);
    }

    #[test]
    fn descriptor_rejects_bad_magic_version_truncation_and_trailing_bytes() {
        let frame = encode(&summary()).unwrap();

        let mut bad_magic = frame.clone();
        bad_magic[0] ^= 0xff;
        assert!(decode(&bad_magic).unwrap_err().contains("invalid magic"));

        let mut bad_version = frame.clone();
        bad_version[4] += 1;
        assert!(decode(&bad_version).unwrap_err().contains("unsupported"));

        assert!(
            decode(&frame[..frame.len() - 1])
                .unwrap_err()
                .contains("truncated")
        );

        let mut trailing = frame;
        trailing.push(0);
        assert!(decode(&trailing).unwrap_err().contains("trailing"));
    }

    #[test]
    fn descriptor_rejects_unknown_profile_and_invalid_flags() {
        let mut frame = encode(&summary()).unwrap();
        let id_length = usize::from(u16::from_le_bytes([frame[5], frame[6]]));
        let display_length_offset = 7 + id_length;
        let display_length = usize::from(u16::from_le_bytes([
            frame[display_length_offset],
            frame[display_length_offset + 1],
        ]));
        let profile_length_offset = display_length_offset + 2 + display_length + 8;
        let profile_offset = profile_length_offset + 2;
        frame[profile_offset] = b'x';
        assert!(decode(&frame).unwrap_err().contains("generation profile"));

        let mut invalid_locked = encode(&summary()).unwrap();
        *invalid_locked.last_mut().unwrap() = 2;
        assert!(decode(&invalid_locked).unwrap_err().contains("locked flag"));
    }
}
