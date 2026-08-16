use mclone_core::{AxisTopology, HorizontalTopology};
use mclone_protocol::{ClientIdentity, PlayerProfileId};
use mclone_server::{
    AuthoredWorldFixtureKind, LocalAuthorityStartConfig, SimulationCadenceConfig,
    StarterContentDescriptor, WorldBehaviorProfile, WorldGenerationProfile,
};

const STARTUP_MAGIC: [u8; 4] = *b"MCSI";
const STARTUP_VERSION: u16 = 5;
const TOPOLOGY_PLANE: u8 = 0;
const TOPOLOGY_CYLINDER_X: u8 = 1;
const FLAG_FREEZE_SCHEDULED_FLUID_TICKS: u8 = 1 << 0;
const FLAG_DEBUG_PASSIVE_SHOWCASE: u8 = 1 << 1;
const FLAG_DEBUG_AUXILIARY_PLAYER_SCRIPT: u8 = 1 << 2;
const FLAG_OBSERVER_ONLY: u8 = 1 << 3;
const FLAG_DAY_TIME_FROZEN: u8 = 1 << 4;
const FLAG_LIGHTING_ENABLED: u8 = 1 << 5;
const FLAG_ADAPTIVE_CHUNK_PUBLICATION: u8 = 1 << 6;
const KNOWN_FLAGS: u8 = FLAG_FREEZE_SCHEDULED_FLUID_TICKS
    | FLAG_DEBUG_PASSIVE_SHOWCASE
    | FLAG_DEBUG_AUXILIARY_PLAYER_SCRIPT
    | FLAG_OBSERVER_ONLY
    | FLAG_DAY_TIME_FROZEN
    | FLAG_LIGHTING_ENABLED
    | FLAG_ADAPTIVE_CHUNK_PUBLICATION;
const MAX_DISPLAY_NAME_BYTES: usize = 1_024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WebIntegratedServerStartupConfig {
    pub authority: LocalAuthorityStartConfig,
    pub transient_authored_fixture: Option<AuthoredWorldFixtureKind>,
    pub transient_playable_showcase: Option<mclone_server::PlayableShowcaseId>,
}

impl WebIntegratedServerStartupConfig {
    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let light_status_batch_size = u32::try_from(self.authority.light_status_batch_size)
            .map_err(|_| "integrated-server light batch size exceeds u32".to_owned())?;
        let local_player_identity =
            self.authority
                .local_player_identity
                .as_ref()
                .ok_or_else(|| {
                    "integrated-server startup requires a local player identity".to_owned()
                })?;
        let display_name = local_player_identity.display_name.as_bytes();
        let display_name_len = u32::try_from(display_name.len())
            .map_err(|_| "integrated-server display name exceeds u32".to_owned())?;

        let mut frame = Vec::with_capacity(48 + display_name.len());
        frame.extend_from_slice(&STARTUP_MAGIC);
        frame.extend_from_slice(&STARTUP_VERSION.to_le_bytes());
        frame.push(generation_profile_tag(
            self.authority.world_generation_profile,
        ));
        frame.push(starter_content_tag(self.authority.starter_content));
        encode_topology(self.authority.world_topology, &mut frame)?;
        frame.push(behavior_profile_tag(self.authority.world_behavior_profile));
        frame.push(authored_fixture_tag(self.transient_authored_fixture));
        frame.push(playable_showcase_tag(self.transient_playable_showcase));
        frame.extend_from_slice(&self.authority.day_time.unwrap_or(u64::MAX).to_le_bytes());
        let mut flags = 0;
        if self.authority.scheduled_fluid_ticks_frozen {
            flags |= FLAG_FREEZE_SCHEDULED_FLUID_TICKS;
        }
        if self.authority.debug_passive_showcase {
            flags |= FLAG_DEBUG_PASSIVE_SHOWCASE;
        }
        if self.authority.debug_auxiliary_player_script {
            flags |= FLAG_DEBUG_AUXILIARY_PLAYER_SCRIPT;
        }
        if self.authority.observer_only {
            flags |= FLAG_OBSERVER_ONLY;
        }
        if self.authority.day_time_frozen {
            flags |= FLAG_DAY_TIME_FROZEN;
        }
        if self.authority.lighting_enabled {
            flags |= FLAG_LIGHTING_ENABLED;
        }
        if self.authority.adaptive_chunk_publication_budget {
            flags |= FLAG_ADAPTIVE_CHUNK_PUBLICATION;
        }
        frame.push(flags);
        frame.extend_from_slice(&light_status_batch_size.to_le_bytes());
        frame.extend_from_slice(&self.authority.cadence.host_rate_hz.to_le_bytes());
        frame.extend_from_slice(&self.authority.cadence.gameplay_rate_hz.to_le_bytes());
        frame.extend_from_slice(&self.authority.cadence.physics_rate_hz.to_le_bytes());
        frame.extend_from_slice(
            &self
                .authority
                .cadence
                .max_catch_up_host_frames
                .to_le_bytes(),
        );
        frame.extend_from_slice(&self.authority.seed.to_le_bytes());
        frame.extend_from_slice(&local_player_identity.profile_id.bytes());
        frame.extend_from_slice(&display_name_len.to_le_bytes());
        frame.extend_from_slice(display_name);
        Ok(frame)
    }

    pub(crate) fn decode(frame: &[u8]) -> Result<Self, String> {
        let mut decoder = StartupDecoder::new(frame);
        if decoder.take(STARTUP_MAGIC.len())? != STARTUP_MAGIC {
            return Err("integrated-server startup frame has invalid magic".to_owned());
        }
        let version = decoder.u16()?;
        if !(2..=STARTUP_VERSION).contains(&version) {
            return Err(format!(
                "unsupported integrated-server startup frame version {version}"
            ));
        }
        let world_generation_profile = generation_profile_from_tag(decoder.u8()?)?;
        let starter_content = if version >= 3 {
            starter_content_from_tag(decoder.u8()?)?
        } else {
            StarterContentDescriptor::Wild
        };
        let world_topology = decode_topology(&mut decoder)?;
        let world_behavior_profile = behavior_profile_from_tag(decoder.u8()?)?;
        let transient_authored_fixture = authored_fixture_from_tag(decoder.u8()?)?;
        let transient_playable_showcase = if version >= 4 {
            playable_showcase_from_tag(decoder.u8()?)?
        } else {
            None
        };
        let day_time = if version >= 4 {
            match decoder.u64()? {
                u64::MAX => None,
                value => Some(value),
            }
        } else {
            None
        };
        let flags = decoder.u8()?;
        if flags & !KNOWN_FLAGS != 0 {
            return Err(format!(
                "integrated-server startup frame has unknown flags 0x{:02x}",
                flags & !KNOWN_FLAGS
            ));
        }
        let light_status_batch_size = usize::try_from(decoder.u32()?)
            .map_err(|_| "integrated-server light batch size exceeds usize".to_owned())?;
        let cadence = if version >= 5 {
            SimulationCadenceConfig::new(decoder.u32()?, decoder.u32()?, decoder.u32()?)
                .with_max_catch_up_host_frames(decoder.u32()?)
        } else {
            SimulationCadenceConfig::default()
        };
        let seed = decoder.i64()?;
        let profile_id: [u8; 16] = decoder
            .take(16)?
            .try_into()
            .map_err(|_| "integrated-server profile UUID is not 16 bytes".to_owned())?;
        let display_name_len = usize::try_from(decoder.u32()?)
            .map_err(|_| "integrated-server display name length exceeds usize".to_owned())?;
        if display_name_len > MAX_DISPLAY_NAME_BYTES {
            return Err(format!(
                "integrated-server display name is {display_name_len} bytes; maximum is {MAX_DISPLAY_NAME_BYTES}"
            ));
        }
        let display_name = std::str::from_utf8(decoder.take(display_name_len)?)
            .map_err(|error| format!("integrated-server display name is not UTF-8: {error}"))?
            .to_owned();
        decoder.finish()?;
        let local_player_identity =
            ClientIdentity::new(PlayerProfileId::new(profile_id), display_name)
                .map_err(|error| format!("invalid integrated-server player identity: {error}"))?;
        let mut authority = LocalAuthorityStartConfig::new(seed);
        authority.world_generation_profile = world_generation_profile;
        authority.starter_content = starter_content;
        authority.world_topology = world_topology;
        authority.world_behavior_profile = world_behavior_profile;
        authority.day_time = day_time;
        authority.day_time_frozen = flags & FLAG_DAY_TIME_FROZEN != 0;
        authority.scheduled_fluid_ticks_frozen = flags & FLAG_FREEZE_SCHEDULED_FLUID_TICKS != 0;
        authority.debug_passive_showcase = flags & FLAG_DEBUG_PASSIVE_SHOWCASE != 0;
        authority.debug_auxiliary_player_script = flags & FLAG_DEBUG_AUXILIARY_PLAYER_SCRIPT != 0;
        authority.light_status_batch_size = light_status_batch_size;
        authority.local_player_identity = Some(local_player_identity);
        authority.observer_only = flags & FLAG_OBSERVER_ONLY != 0;
        authority.lighting_enabled = version < 5 || flags & FLAG_LIGHTING_ENABLED != 0;
        authority.adaptive_chunk_publication_budget =
            version >= 5 && flags & FLAG_ADAPTIVE_CHUNK_PUBLICATION != 0;
        authority.cadence = cadence;
        let config = Self {
            authority,
            transient_authored_fixture,
            transient_playable_showcase,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        self.authority.validate()?;
        validate_browser_topology(self.authority.world_topology)?;
        if let Some(fixture) = self.transient_authored_fixture {
            if self.authority.seed != fixture.seed() {
                return Err(format!(
                    "authored fixture {} requires seed {}, got {}",
                    fixture.fixture_id(),
                    fixture.seed(),
                    self.authority.seed
                ));
            }
            if self.authority.world_generation_profile != WorldGenerationProfile::authored_only() {
                return Err(format!(
                    "authored fixture {} requires the authored-only generation profile",
                    fixture.fixture_id()
                ));
            }
        }
        if self.transient_authored_fixture.is_some() && self.transient_playable_showcase.is_some() {
            return Err("integrated-server startup cannot select both an authored fixture and a playable showcase".to_owned());
        }
        if let Some(showcase) = self.transient_playable_showcase {
            let manifest = mclone_server::playable_showcase_manifest(showcase)
                .map_err(|error| error.to_string())?;
            if self.authority.seed != manifest.seed
                || self.authority.world_generation_profile != manifest.world_generation_profile
            {
                return Err(format!(
                    "playable showcase {} requires seed {} and profile {}, got seed {} and profile {}",
                    showcase.label(),
                    manifest.seed,
                    manifest.world_generation_profile.label(),
                    self.authority.seed,
                    self.authority.world_generation_profile.label(),
                ));
            }
        }
        let local_player_identity =
            self.authority
                .local_player_identity
                .as_ref()
                .ok_or_else(|| {
                    "integrated-server startup requires a local player identity".to_owned()
                })?;
        let display_name_len = local_player_identity.display_name.len();
        if display_name_len > MAX_DISPLAY_NAME_BYTES {
            return Err(format!(
                "integrated-server display name is {display_name_len} bytes; maximum is {MAX_DISPLAY_NAME_BYTES}"
            ));
        }
        Ok(())
    }
}

const fn starter_content_tag(starter_content: StarterContentDescriptor) -> u8 {
    match starter_content {
        StarterContentDescriptor::Wild => 0,
        StarterContentDescriptor::IntroHomesteadV1 => 1,
    }
}

fn starter_content_from_tag(tag: u8) -> Result<StarterContentDescriptor, String> {
    match tag {
        0 => Ok(StarterContentDescriptor::Wild),
        1 => Ok(StarterContentDescriptor::IntroHomesteadV1),
        _ => Err(format!(
            "integrated-server startup frame has unknown starter content {tag}"
        )),
    }
}

fn generation_profile_tag(profile: WorldGenerationProfile) -> u8 {
    match profile {
        WorldGenerationProfile::Overworld => 0,
        WorldGenerationProfile::FlatGrassV1 => 1,
        WorldGenerationProfile::SmallIslandV1 => 2,
        WorldGenerationProfile::McloneOverworldV1 => 3,
        WorldGenerationProfile::AlphaV1 { winter: false } => 4,
        WorldGenerationProfile::AlphaV1 { winter: true } => 5,
        WorldGenerationProfile::BetaV1 => 6,
        WorldGenerationProfile::AuthoredOnly { .. } => 7,
        WorldGenerationProfile::TopologyProbeV1 => 8,
    }
}

fn generation_profile_from_tag(tag: u8) -> Result<WorldGenerationProfile, String> {
    match tag {
        0 => Ok(WorldGenerationProfile::Overworld),
        1 => Ok(WorldGenerationProfile::FlatGrassV1),
        2 => Ok(WorldGenerationProfile::SmallIslandV1),
        3 => Ok(WorldGenerationProfile::McloneOverworldV1),
        4 => Ok(WorldGenerationProfile::alpha_v1(false)),
        5 => Ok(WorldGenerationProfile::alpha_v1(true)),
        6 => Ok(WorldGenerationProfile::BetaV1),
        7 => Ok(WorldGenerationProfile::authored_only()),
        8 => Ok(WorldGenerationProfile::TopologyProbeV1),
        _ => Err(format!(
            "integrated-server startup frame has unknown generation profile {tag}"
        )),
    }
}

fn behavior_profile_tag(profile: WorldBehaviorProfile) -> u8 {
    match profile {
        WorldBehaviorProfile::Mutable => 0,
        WorldBehaviorProfile::ProtectedLobby => 1,
    }
}

fn behavior_profile_from_tag(tag: u8) -> Result<WorldBehaviorProfile, String> {
    match tag {
        0 => Ok(WorldBehaviorProfile::Mutable),
        1 => Ok(WorldBehaviorProfile::ProtectedLobby),
        _ => Err(format!(
            "integrated-server startup frame has unknown behavior profile {tag}"
        )),
    }
}

const fn authored_fixture_tag(fixture: Option<AuthoredWorldFixtureKind>) -> u8 {
    match fixture {
        None => 0,
        Some(AuthoredWorldFixtureKind::Table) => 1,
        Some(AuthoredWorldFixtureKind::Island) => 2,
        Some(AuthoredWorldFixtureKind::LobbyTableV2) => 3,
        Some(AuthoredWorldFixtureKind::LobbyIslandV2) => 4,
        Some(AuthoredWorldFixtureKind::MallardWetland) => 5,
        Some(AuthoredWorldFixtureKind::DeerForestEdge) => 6,
        Some(AuthoredWorldFixtureKind::BeeFloweringMeadow) => 7,
        Some(AuthoredWorldFixtureKind::WheatFarming) => 8,
        Some(AuthoredWorldFixtureKind::RabbitMeadow) => 9,
    }
}

fn authored_fixture_from_tag(tag: u8) -> Result<Option<AuthoredWorldFixtureKind>, String> {
    match tag {
        0 => Ok(None),
        1 => Ok(Some(AuthoredWorldFixtureKind::Table)),
        2 => Ok(Some(AuthoredWorldFixtureKind::Island)),
        3 => Ok(Some(AuthoredWorldFixtureKind::LobbyTableV2)),
        4 => Ok(Some(AuthoredWorldFixtureKind::LobbyIslandV2)),
        5 => Ok(Some(AuthoredWorldFixtureKind::MallardWetland)),
        6 => Ok(Some(AuthoredWorldFixtureKind::DeerForestEdge)),
        7 => Ok(Some(AuthoredWorldFixtureKind::BeeFloweringMeadow)),
        8 => Ok(Some(AuthoredWorldFixtureKind::WheatFarming)),
        9 => Ok(Some(AuthoredWorldFixtureKind::RabbitMeadow)),
        _ => Err(format!(
            "integrated-server startup frame has unknown authored fixture {tag}"
        )),
    }
}

const fn playable_showcase_tag(showcase: Option<mclone_server::PlayableShowcaseId>) -> u8 {
    match showcase {
        None => 0,
        Some(mclone_server::PlayableShowcaseId::MallardEcology) => 1,
        Some(mclone_server::PlayableShowcaseId::DeerForestEdge) => 2,
        Some(mclone_server::PlayableShowcaseId::BeePollination) => 3,
        Some(mclone_server::PlayableShowcaseId::WheatFarming) => 4,
        Some(mclone_server::PlayableShowcaseId::KitchenGarden) => 5,
        Some(mclone_server::PlayableShowcaseId::RabbitBurrow) => 6,
    }
}

fn playable_showcase_from_tag(
    tag: u8,
) -> Result<Option<mclone_server::PlayableShowcaseId>, String> {
    match tag {
        0 => Ok(None),
        1 => Ok(Some(mclone_server::PlayableShowcaseId::MallardEcology)),
        2 => Ok(Some(mclone_server::PlayableShowcaseId::DeerForestEdge)),
        3 => Ok(Some(mclone_server::PlayableShowcaseId::BeePollination)),
        4 => Ok(Some(mclone_server::PlayableShowcaseId::WheatFarming)),
        5 => Ok(Some(mclone_server::PlayableShowcaseId::KitchenGarden)),
        6 => Ok(Some(mclone_server::PlayableShowcaseId::RabbitBurrow)),
        _ => Err(format!(
            "integrated-server startup frame has unknown playable showcase {tag}"
        )),
    }
}

fn encode_topology(topology: HorizontalTopology, frame: &mut Vec<u8>) -> Result<(), String> {
    match (topology.x, topology.z) {
        (AxisTopology::Unbounded, AxisTopology::Unbounded) => frame.push(TOPOLOGY_PLANE),
        (
            AxisTopology::Periodic {
                minimum_chunk: 0,
                period_chunks,
            },
            AxisTopology::Unbounded,
        ) => {
            frame.push(TOPOLOGY_CYLINDER_X);
            frame.extend_from_slice(&period_chunks.to_le_bytes());
        }
        _ => {
            return Err(format!(
                "browser startup does not yet expose topology {topology:?}"
            ));
        }
    }
    Ok(())
}

fn decode_topology(decoder: &mut StartupDecoder<'_>) -> Result<HorizontalTopology, String> {
    match decoder.u8()? {
        TOPOLOGY_PLANE => Ok(HorizontalTopology::UNBOUNDED),
        TOPOLOGY_CYLINDER_X => Ok(HorizontalTopology::cylinder_x(0, decoder.u32()?)),
        tag => Err(format!(
            "integrated-server startup frame has unknown topology {tag}"
        )),
    }
}

fn validate_browser_topology(topology: HorizontalTopology) -> Result<(), String> {
    topology
        .validate()
        .map_err(|error| format!("invalid browser world topology: {error}"))?;
    match (topology.x, topology.z) {
        (AxisTopology::Unbounded, AxisTopology::Unbounded)
        | (
            AxisTopology::Periodic {
                minimum_chunk: 0, ..
            },
            AxisTopology::Unbounded,
        ) => Ok(()),
        _ => Err(format!(
            "browser startup does not yet expose topology {topology:?}"
        )),
    }
}

struct StartupDecoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> StartupDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or_else(|| "integrated-server startup frame cursor overflowed".to_owned())?;
        let value = self.bytes.get(self.cursor..end).ok_or_else(|| {
            format!(
                "integrated-server startup frame ended at byte {}; needed {len} more bytes",
                self.cursor
            )
        })?;
        self.cursor = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn finish(self) -> Result<(), String> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "integrated-server startup frame has {} trailing bytes",
                self.bytes.len() - self.cursor
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> WebIntegratedServerStartupConfig {
        let mut authority = LocalAuthorityStartConfig::new(-42);
        authority.world_generation_profile = WorldGenerationProfile::FlatGrassV1;
        authority.starter_content = StarterContentDescriptor::IntroHomesteadV1;
        authority.world_topology = HorizontalTopology::cylinder_x(0, 32);
        authority.world_behavior_profile = WorldBehaviorProfile::ProtectedLobby;
        authority.day_time = Some(6_000);
        authority.day_time_frozen = true;
        authority.scheduled_fluid_ticks_frozen = true;
        authority.debug_auxiliary_player_script = true;
        authority.light_status_batch_size = 17;
        authority.cadence =
            SimulationCadenceConfig::new(45, 15, 90).with_max_catch_up_host_frames(7);
        authority.adaptive_chunk_publication_budget = true;
        authority.local_player_identity =
            Some(ClientIdentity::new(PlayerProfileId::new([7; 16]), "Startup Player").unwrap());
        authority.observer_only = true;
        WebIntegratedServerStartupConfig {
            authority,
            transient_authored_fixture: None,
            transient_playable_showcase: None,
        }
    }

    #[test]
    fn startup_frame_roundtrips_every_authority_field() {
        let expected = config();
        let frame = expected.encode().unwrap();
        assert_eq!(
            WebIntegratedServerStartupConfig::decode(&frame),
            Ok(expected)
        );
    }

    #[test]
    fn version_three_startup_defaults_new_showcase_fields() {
        let mut expected = config();
        expected.transient_playable_showcase = None;
        expected.authority.day_time = None;
        expected.authority.day_time_frozen = false;
        expected.authority.lighting_enabled = true;
        expected.authority.adaptive_chunk_publication_budget = false;
        expected.authority.cadence = SimulationCadenceConfig::default();
        let mut frame = expected.encode().unwrap();
        frame[4..6].copy_from_slice(&3_u16.to_le_bytes());
        let showcase_index = 4 + 2 + 1 + 1 + 1 + 4 + 1 + 1;
        frame.drain(showcase_index..showcase_index + 1 + 8);
        let cadence_index = showcase_index + 1 + 4;
        frame.drain(cadence_index..cadence_index + 4 * 4);

        assert_eq!(
            WebIntegratedServerStartupConfig::decode(&frame),
            Ok(expected)
        );
    }

    #[test]
    fn startup_frame_roundtrips_transient_authored_fixture() {
        let mut expected = config();
        expected.authority.seed = AuthoredWorldFixtureKind::LobbyTableV2.seed();
        expected.authority.world_generation_profile = WorldGenerationProfile::authored_only();
        expected.transient_authored_fixture = Some(AuthoredWorldFixtureKind::LobbyTableV2);
        let frame = expected.encode().unwrap();
        assert_eq!(
            WebIntegratedServerStartupConfig::decode(&frame),
            Ok(expected)
        );
    }

    #[test]
    fn startup_frame_roundtrips_transient_playable_showcase() {
        let mut expected = config();
        let manifest = mclone_server::playable_showcase_manifest(
            mclone_server::PlayableShowcaseId::MallardEcology,
        )
        .unwrap();
        expected.authority.seed = manifest.seed;
        expected.authority.world_generation_profile = manifest.world_generation_profile;
        expected.transient_playable_showcase = Some(manifest.id);
        let frame = expected.encode().unwrap();
        assert_eq!(
            WebIntegratedServerStartupConfig::decode(&frame),
            Ok(expected)
        );
    }

    #[test]
    fn startup_frame_rejects_version_flags_truncation_and_trailing_data() {
        let frame = config().encode().unwrap();

        let mut bad_version = frame.clone();
        bad_version[4..6].copy_from_slice(&(STARTUP_VERSION + 1).to_le_bytes());
        assert!(WebIntegratedServerStartupConfig::decode(&bad_version).is_err());

        let mut bad_flags = frame.clone();
        let flags_index = 4 + 2 + 1 + 1 + 1 + 4 + 1 + 1 + 1 + 8;
        bad_flags[flags_index] |= 1 << 7;
        assert!(WebIntegratedServerStartupConfig::decode(&bad_flags).is_err());

        assert!(WebIntegratedServerStartupConfig::decode(&frame[..frame.len() - 1]).is_err());

        let mut trailing = frame;
        trailing.push(0);
        assert!(WebIntegratedServerStartupConfig::decode(&trailing).is_err());
    }

    #[test]
    fn startup_frame_rejects_unsupported_or_invalid_topology() {
        let mut unsupported = config();
        unsupported.authority.world_topology = HorizontalTopology::new(
            AxisTopology::Finite {
                minimum_chunk: 0,
                maximum_chunk_exclusive: 16,
            },
            AxisTopology::Unbounded,
        );
        assert!(unsupported.encode().is_err());

        let mut invalid = config();
        invalid.authority.world_topology = HorizontalTopology::cylinder_x(0, 0);
        assert!(invalid.encode().is_err());
    }
}
