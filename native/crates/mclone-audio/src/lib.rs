#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Mutex,
    mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
};

use anyhow::{Context, Result, bail};
#[cfg(not(target_arch = "wasm32"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(not(target_arch = "wasm32"))]
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use lewton::inside_ogg::OggStreamReader;
use mclone_assets::{AssetPath, AssetSource};
use serde::Deserialize;

#[cfg(not(target_arch = "wasm32"))]
const COMMAND_QUEUE_CAPACITY: usize = 128;
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_MAX_VOICES: usize = 32;
const LANDING_BIG_IMPACT_SPEED: f64 = 6.0;
pub const FIRST_PARTY_SOUND_BANK_PATH: &str = "assets/mclone/audio/sound-bank.v1.json";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SoundKey(&'static str);

impl SoundKey {
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

pub const LANDING_SMALL: SoundKey = SoundKey("mclone:landing_small");
pub const LANDING_BIG: SoundKey = SoundKey("mclone:landing_big");
pub const FOOTSTEP_CARPET: SoundKey = SoundKey("mclone:footstep_carpet");
pub const FOOTSTEP_GRASS: SoundKey = SoundKey("mclone:footstep_grass");
pub const FOOTSTEP_NEUTRAL: SoundKey = SoundKey("mclone:footstep_neutral");
pub const FOOTSTEP_SNOW: SoundKey = SoundKey("mclone:footstep_snow");
pub const FOOTSTEP_STONE: SoundKey = SoundKey("mclone:footstep_stone");
pub const FOOTSTEP_WOOD: SoundKey = SoundKey("mclone:footstep_wood");
pub const LANDING_CARPET: SoundKey = SoundKey("mclone:landing_carpet");
pub const LANDING_GRASS: SoundKey = SoundKey("mclone:landing_grass");
pub const LANDING_NEUTRAL: SoundKey = SoundKey("mclone:landing_neutral");
pub const LANDING_SNOW: SoundKey = SoundKey("mclone:landing_snow");
pub const LANDING_STONE: SoundKey = SoundKey("mclone:landing_stone");
pub const LANDING_WOOD: SoundKey = SoundKey("mclone:landing_wood");
pub const BREAK_GLASS: SoundKey = SoundKey("mclone:break_glass");
pub const BREAK_METAL: SoundKey = SoundKey("mclone:break_metal");
pub const BREAK_SOFT: SoundKey = SoundKey("mclone:break_soft");
pub const BREAK_STONE: SoundKey = SoundKey("mclone:break_stone");
pub const BREAK_WOOD: SoundKey = SoundKey("mclone:break_wood");
pub const PLACE_GLASS: SoundKey = SoundKey("mclone:place_glass");
pub const PLACE_METAL: SoundKey = SoundKey("mclone:place_metal");
pub const PLACE_SOFT: SoundKey = SoundKey("mclone:place_soft");
pub const PLACE_STONE: SoundKey = SoundKey("mclone:place_stone");
pub const PLACE_WOOD: SoundKey = SoundKey("mclone:place_wood");
pub const IMPACT_GLASS: SoundKey = SoundKey("mclone:impact_glass");
pub const IMPACT_METAL: SoundKey = SoundKey("mclone:impact_metal");
pub const IMPACT_WOOD: SoundKey = SoundKey("mclone:impact_wood");
pub const CLOTH_MOVE: SoundKey = SoundKey("mclone:cloth_move");
pub const ITEM_PICKUP: SoundKey = SoundKey("mclone:item_pickup");
pub const MALLARD_CALL: SoundKey = SoundKey("mclone:mallard_call");
pub const DEER_CONTACT: SoundKey = SoundKey("mclone:deer_contact");
pub const DEER_ALARM: SoundKey = SoundKey("mclone:deer_alarm");
pub const DEER_IMPACT: SoundKey = SoundKey("mclone:deer_impact");
pub const BEE_BUZZ: SoundKey = SoundKey("mclone:bee_buzz");
pub const WOOD_CREAK: SoundKey = SoundKey("mclone:wood_creak");
pub const UI_BACK: SoundKey = SoundKey("mclone:ui_back");
pub const UI_CONFIRM: SoundKey = SoundKey("mclone:ui_confirm");
pub const UI_ERROR: SoundKey = SoundKey("mclone:ui_error");
pub const UI_OPEN: SoundKey = SoundKey("mclone:ui_open");
pub const UI_SELECT: SoundKey = SoundKey("mclone:ui_select");

const CATALOG_SOUND_KEYS: &[SoundKey] = &[
    FOOTSTEP_CARPET,
    FOOTSTEP_GRASS,
    FOOTSTEP_NEUTRAL,
    FOOTSTEP_SNOW,
    FOOTSTEP_STONE,
    FOOTSTEP_WOOD,
    LANDING_CARPET,
    LANDING_GRASS,
    LANDING_NEUTRAL,
    LANDING_SNOW,
    LANDING_STONE,
    LANDING_WOOD,
    BREAK_GLASS,
    BREAK_METAL,
    BREAK_SOFT,
    BREAK_STONE,
    BREAK_WOOD,
    PLACE_GLASS,
    PLACE_METAL,
    PLACE_SOFT,
    PLACE_STONE,
    PLACE_WOOD,
    IMPACT_GLASS,
    IMPACT_METAL,
    IMPACT_WOOD,
    CLOTH_MOVE,
    ITEM_PICKUP,
    MALLARD_CALL,
    DEER_CONTACT,
    DEER_ALARM,
    DEER_IMPACT,
    BEE_BUZZ,
    WOOD_CREAK,
    UI_BACK,
    UI_CONFIRM,
    UI_ERROR,
    UI_OPEN,
    UI_SELECT,
];

impl SoundKey {
    fn parse(value: &str) -> Option<Self> {
        CATALOG_SOUND_KEYS
            .iter()
            .copied()
            .find(|key| key.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackParams {
    pub gain: f32,
    pub pitch: f32,
    pub pan: f32,
    pub seed: u64,
}

impl PlaybackParams {
    pub const fn with_gain(gain: f32) -> Self {
        Self {
            gain,
            pitch: 1.0,
            pan: 0.0,
            seed: 0,
        }
    }
}

impl Default for PlaybackParams {
    fn default() -> Self {
        Self::with_gain(1.0)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AcousticMaterial {
    Carpet,
    Glass,
    Grass,
    Metal,
    Snow,
    Soft,
    Stone,
    Wood,
    #[default]
    Neutral,
}

impl AcousticMaterial {
    pub fn from_block_path(block: &str) -> Self {
        if block.contains("carpet") || block.contains("wool") {
            Self::Carpet
        } else if block.contains("glass") || block.contains("ice") {
            Self::Glass
        } else if block.contains("snow") {
            Self::Snow
        } else if block.contains("rail")
            || block.contains("iron")
            || block.contains("gold")
            || block.contains("copper")
            || block.contains("anvil")
        {
            Self::Metal
        } else if block.contains("log")
            || block.contains("wood")
            || block.contains("plank")
            || block.contains("bookshelf")
            || block.contains("chest")
            || block.contains("door")
            || block.contains("fence")
        {
            Self::Wood
        } else if block.contains("grass")
            || block.contains("leaves")
            || block.contains("fern")
            || block.contains("flower")
            || block.contains("moss")
            || block.contains("vine")
        {
            Self::Grass
        } else if block.contains("dirt")
            || block.contains("sand")
            || block.contains("gravel")
            || block.contains("clay")
            || block.contains("farmland")
            || block.contains("mud")
        {
            Self::Soft
        } else if block.contains("stone")
            || block.contains("ore")
            || block.contains("brick")
            || block.contains("terracotta")
            || block.contains("concrete")
            || block.contains("bedrock")
            || block.contains("basalt")
            || block.contains("tuff")
        {
            Self::Stone
        } else {
            Self::Neutral
        }
    }

    pub const fn footstep_sound(self) -> SoundKey {
        match self {
            Self::Carpet => FOOTSTEP_CARPET,
            Self::Grass => FOOTSTEP_GRASS,
            Self::Snow => FOOTSTEP_SNOW,
            Self::Wood => FOOTSTEP_WOOD,
            Self::Stone | Self::Glass | Self::Metal => FOOTSTEP_STONE,
            Self::Soft | Self::Neutral => FOOTSTEP_NEUTRAL,
        }
    }

    pub const fn landing_sound(self) -> SoundKey {
        match self {
            Self::Carpet => LANDING_CARPET,
            Self::Grass => LANDING_GRASS,
            Self::Snow => LANDING_SNOW,
            Self::Wood => LANDING_WOOD,
            Self::Stone | Self::Glass | Self::Metal => LANDING_STONE,
            Self::Soft | Self::Neutral => LANDING_NEUTRAL,
        }
    }

    pub const fn break_sound(self) -> SoundKey {
        match self {
            Self::Glass => BREAK_GLASS,
            Self::Metal => BREAK_METAL,
            Self::Carpet | Self::Grass | Self::Snow | Self::Soft => BREAK_SOFT,
            Self::Stone => BREAK_STONE,
            Self::Wood => BREAK_WOOD,
            Self::Neutral => BREAK_STONE,
        }
    }

    pub const fn place_sound(self) -> SoundKey {
        match self {
            Self::Glass => PLACE_GLASS,
            Self::Metal => PLACE_METAL,
            Self::Carpet | Self::Grass | Self::Snow | Self::Soft => PLACE_SOFT,
            Self::Stone => PLACE_STONE,
            Self::Wood => PLACE_WOOD,
            Self::Neutral => PLACE_STONE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioSettings {
    pub enabled: bool,
    pub master_gain: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            master_gain: 1.0,
        }
    }
}

pub fn landing_playback_for_impact(impact_speed: f64) -> (SoundKey, f32) {
    let sound = if impact_speed >= LANDING_BIG_IMPACT_SPEED {
        LANDING_BIG
    } else {
        LANDING_SMALL
    };
    let gain = ((impact_speed / LANDING_BIG_IMPACT_SPEED) as f32).clamp(0.18, 1.0);
    (sound, gain)
}

#[cfg(not(target_arch = "wasm32"))]
pub struct AudioEngine {
    commands: SyncSender<AudioCommand>,
    bank: SoundBank,
    selector: Mutex<VariantSelector>,
    _stream: cpal::Stream,
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioEngine {
    pub fn new(assets: &impl AssetSource, settings: AudioSettings) -> Result<Self> {
        Self::from_prepared(PreparedAudioAssets::load(assets)?, settings)
    }

    pub fn from_prepared(assets: PreparedAudioAssets, settings: AudioSettings) -> Result<Self> {
        let bank = assets.bank;
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no default audio output device available")?;
        let supported_config = device
            .default_output_config()
            .context("failed to query default audio output config")?;
        let sample_format = supported_config.sample_format();
        let config: cpal::StreamConfig = supported_config.into();
        let output_sample_rate = config.sample_rate;
        let output_channels = usize::from(config.channels);
        let (commands, receiver) = sync_channel(COMMAND_QUEUE_CAPACITY);
        let mixer = Mixer::new(
            bank.clone(),
            receiver,
            settings,
            output_sample_rate,
            DEFAULT_MAX_VOICES,
        );
        let stream = build_output_stream(&device, &config, sample_format, mixer)
            .context("failed to build audio output stream")?;
        stream
            .play()
            .context("failed to start audio output stream")?;
        log::info!(
            "audio output started: sample_rate={} channels={} format={:?}",
            output_sample_rate,
            output_channels,
            sample_format
        );
        Ok(Self {
            commands,
            bank,
            selector: Mutex::new(VariantSelector::default()),
            _stream: stream,
        })
    }

    pub fn play(&self, sound: SoundKey, gain: f32) {
        self.play_with(sound, PlaybackParams::with_gain(gain));
    }

    pub fn play_with(&self, sound: SoundKey, params: PlaybackParams) {
        let Some(playback) = self
            .selector
            .lock()
            .expect("audio variant selector poisoned")
            .resolve(&self.bank, sound, params)
        else {
            return;
        };
        match self.commands.try_send(AudioCommand::Play(playback)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                log::debug!("audio command queue full; dropping {}", sound.as_str());
            }
            Err(TrySendError::Disconnected(_)) => {
                log::warn!(
                    "audio command queue disconnected; dropping {}",
                    sound.as_str()
                );
            }
        }
    }

    pub fn set_settings(&self, settings: AudioSettings) {
        match self.commands.try_send(AudioCommand::Settings(settings)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                log::debug!("audio command queue full; dropping settings update");
            }
            Err(TrySendError::Disconnected(_)) => {
                log::warn!("audio command queue disconnected; dropping settings update");
            }
        }
    }
}

/// Optional audio output attached by a platform host. Shared scene code emits
/// neutral sound commands and requests a replacement capability at an asset
/// epoch boundary; output-device creation remains inside the implementation.
pub trait AudioOutput: Send {
    fn play_with(&self, sound: SoundKey, params: PlaybackParams);

    fn replacement(&self, assets: PreparedAudioAssets) -> Result<Box<dyn AudioOutput>>;
}

#[cfg(not(target_arch = "wasm32"))]
impl AudioOutput for AudioEngine {
    fn play_with(&self, sound: SoundKey, params: PlaybackParams) {
        AudioEngine::play_with(self, sound, params);
    }

    fn replacement(&self, assets: PreparedAudioAssets) -> Result<Box<dyn AudioOutput>> {
        Ok(Box::new(AudioEngine::from_prepared(
            assets,
            AudioSettings::default(),
        )?))
    }
}

/// Explicit audio availability at the scene-host boundary.
pub enum AudioOutputCapability {
    Unavailable,
    Available(Box<dyn AudioOutput>),
}

impl AudioOutputCapability {
    pub fn available(output: impl AudioOutput + 'static) -> Self {
        Self::Available(Box::new(output))
    }

    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available(_))
    }

    pub fn play(&self, sound: SoundKey, gain: f32) {
        self.play_with(sound, PlaybackParams::with_gain(gain));
    }

    pub fn play_with(&self, sound: SoundKey, params: PlaybackParams) {
        if let Self::Available(output) = self {
            output.play_with(sound, params);
        }
    }

    pub fn replacement(&self, assets: PreparedAudioAssets) -> Result<Self> {
        match self {
            Self::Unavailable => Ok(Self::Unavailable),
            Self::Available(output) => output.replacement(assets).map(Self::Available),
        }
    }
}

impl Default for AudioOutputCapability {
    fn default() -> Self {
        Self::Unavailable
    }
}

impl std::fmt::Debug for AudioOutputCapability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioOutputCapability")
            .field("available", &self.is_available())
            .finish()
    }
}

#[derive(Clone)]
pub struct PreparedAudioAssets {
    bank: SoundBank,
}

impl PreparedAudioAssets {
    pub fn load(assets: &impl AssetSource) -> Result<Self> {
        Ok(Self {
            bank: SoundBank::load(assets).context("failed to load audio sound bank")?,
        })
    }

    pub fn load_first_party(assets: &impl AssetSource) -> Result<Self> {
        Ok(Self {
            bank: SoundBank::load_first_party(assets)
                .context("failed to load first-party audio sound bank")?,
        })
    }

    pub fn silent() -> Self {
        Self {
            bank: SoundBank {
                samples: Vec::new(),
                families: HashMap::new(),
            },
        }
    }

    pub fn sound_count(&self) -> usize {
        self.bank.samples.len()
    }

    pub fn is_silent(&self) -> bool {
        self.bank.families.is_empty()
    }

    pub fn family_count(&self) -> usize {
        self.bank.families.len()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq)]
enum AudioCommand {
    Play(ResolvedPlayback),
    Settings(AudioSettings),
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ResolvedPlayback {
    sample_index: u16,
    gain: f32,
    pitch: f32,
    pan: f32,
}

#[derive(Clone)]
struct SampleData {
    channels: usize,
    sample_rate: u32,
    data: Vec<f32>,
}

impl SampleData {
    fn new(channels: usize, sample_rate: u32, data: Vec<f32>) -> Result<Self> {
        if channels == 0 || sample_rate == 0 {
            bail!("invalid sample format: channels={channels} sample_rate={sample_rate}");
        }
        if data.len() % channels != 0 {
            bail!(
                "sample data length {} is not divisible by channel count {channels}",
                data.len()
            );
        }
        Ok(Self {
            channels,
            sample_rate,
            data,
        })
    }

    fn frame_count(&self) -> usize {
        self.data.len() / self.channels
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn value_at(&self, frame_position: f64, output_channel: usize, output_channels: usize) -> f32 {
        let frame = frame_position.floor();
        if frame < 0.0 {
            return 0.0;
        }
        let frame = frame as usize;
        let frame_count = self.frame_count();
        if frame >= frame_count {
            return 0.0;
        }

        let next_frame = (frame + 1).min(frame_count - 1);
        let fraction = (frame_position - frame as f64) as f32;
        let a = self.frame_channel_value(frame, output_channel, output_channels);
        let b = self.frame_channel_value(next_frame, output_channel, output_channels);
        a + (b - a) * fraction
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn frame_channel_value(
        &self,
        frame: usize,
        output_channel: usize,
        output_channels: usize,
    ) -> f32 {
        if output_channels == 1 && self.channels > 1 {
            let offset = frame * self.channels;
            let total: f32 = self.data[offset..offset + self.channels].iter().sum();
            return total / self.channels as f32;
        }
        let source_channel = if self.channels == 1 {
            0
        } else {
            output_channel.min(self.channels - 1)
        };
        self.data[frame * self.channels + source_channel]
    }
}

#[derive(Clone)]
struct SoundBank {
    samples: Vec<Arc<SampleData>>,
    families: HashMap<SoundKey, PreparedFamily>,
}

#[derive(Clone, Debug)]
struct PreparedFamily {
    variants: Vec<u16>,
    gain: f32,
    pitch_min: f32,
    pitch_max: f32,
    no_immediate_repeat: bool,
}

#[derive(Debug, Deserialize)]
struct RawSoundCatalog {
    schema_version: u32,
    license: String,
    selected_file_count: usize,
    families: Vec<RawSoundFamily>,
}

#[derive(Debug, Deserialize)]
struct RawSoundFamily {
    key: String,
    gain: f32,
    pitch_min: f32,
    pitch_max: f32,
    no_immediate_repeat: bool,
    variants: Vec<String>,
}

impl SoundBank {
    fn load(assets: &impl AssetSource) -> Result<Self> {
        let catalog_path = AssetPath::new(FIRST_PARTY_SOUND_BANK_PATH);
        if let Some(bytes) = assets
            .read(&catalog_path)
            .context("failed to read first-party sound catalog")?
        {
            return Self::from_catalog(assets, &bytes);
        }
        Self::load_legacy(assets)
    }

    fn load_first_party(assets: &impl AssetSource) -> Result<Self> {
        let catalog_path = AssetPath::new(FIRST_PARTY_SOUND_BANK_PATH);
        let bytes = assets
            .read(&catalog_path)
            .context("failed to read first-party sound catalog")?
            .with_context(|| {
                format!("required sound catalog {FIRST_PARTY_SOUND_BANK_PATH} is missing")
            })?;
        Self::from_catalog(assets, &bytes)
    }

    fn from_catalog(assets: &impl AssetSource, bytes: &[u8]) -> Result<Self> {
        let raw: RawSoundCatalog =
            serde_json::from_slice(bytes).context("invalid first-party sound catalog JSON")?;
        if raw.schema_version != 1 {
            bail!(
                "unsupported first-party sound catalog schema {}",
                raw.schema_version
            );
        }
        if raw.license != "CC0-1.0" {
            bail!("first-party sound catalog must declare CC0-1.0");
        }
        let mut family_keys = HashSet::new();
        let mut selected_paths = HashSet::new();
        for family in &raw.families {
            if !family_keys.insert(family.key.as_str()) {
                bail!("duplicate first-party sound family {}", family.key);
            }
            if SoundKey::parse(&family.key).is_none() {
                bail!("unknown first-party sound family {}", family.key);
            }
            if family.variants.is_empty() {
                bail!("first-party sound family {} has no variants", family.key);
            }
            if !family.gain.is_finite() || !(0.0..=1.0).contains(&family.gain) {
                bail!("first-party sound family {} has invalid gain", family.key);
            }
            if !family.pitch_min.is_finite()
                || !family.pitch_max.is_finite()
                || !(0.5..=2.0).contains(&family.pitch_min)
                || !(0.5..=2.0).contains(&family.pitch_max)
                || family.pitch_min > family.pitch_max
            {
                bail!(
                    "first-party sound family {} has invalid pitch range",
                    family.key
                );
            }
            let mut variants = HashSet::new();
            for path in &family.variants {
                if !path.starts_with("assets/mclone/sounds/") || !path.ends_with(".ogg") {
                    bail!(
                        "first-party sound family {} has invalid path {path}",
                        family.key
                    );
                }
                if !variants.insert(path.as_str()) {
                    bail!("first-party sound family {} repeats {path}", family.key);
                }
                selected_paths.insert(path.clone());
            }
        }
        if selected_paths.len() != raw.selected_file_count {
            bail!(
                "first-party catalog declares {} files but references {}",
                raw.selected_file_count,
                selected_paths.len()
            );
        }

        let mut samples = Vec::with_capacity(selected_paths.len());
        let mut sample_indexes = HashMap::new();
        let mut ordered_paths = selected_paths.into_iter().collect::<Vec<_>>();
        ordered_paths.sort_unstable();
        for path in ordered_paths {
            let asset_path = AssetPath::new(&path);
            let Some(bytes) = assets
                .read(&asset_path)
                .with_context(|| format!("failed to read sound asset {path}"))?
            else {
                log::warn!("missing first-party sound asset {path}");
                continue;
            };
            let sample = decode_ogg(&bytes)
                .with_context(|| format!("failed to decode sound asset {path}"))?;
            let index = u16::try_from(samples.len()).context("sound bank has too many samples")?;
            sample_indexes.insert(path, index);
            samples.push(Arc::new(sample));
        }

        let mut families = HashMap::new();
        for raw_family in raw.families {
            let key = SoundKey::parse(&raw_family.key)
                .expect("catalog family keys were validated before decoding");
            let variants = raw_family
                .variants
                .iter()
                .filter_map(|path| sample_indexes.get(path.as_str()).copied())
                .collect::<Vec<_>>();
            if variants.is_empty() {
                log::warn!(
                    "sound family {} has no decoded variants; playback will be silent",
                    key.as_str()
                );
                continue;
            }
            families.insert(
                key,
                PreparedFamily {
                    variants,
                    gain: raw_family.gain,
                    pitch_min: raw_family.pitch_min,
                    pitch_max: raw_family.pitch_max,
                    no_immediate_repeat: raw_family.no_immediate_repeat,
                },
            );
        }
        log::info!(
            "loaded first-party sound bank: {} samples across {} families",
            samples.len(),
            families.len()
        );
        Ok(Self { samples, families })
    }

    fn load_legacy(assets: &impl AssetSource) -> Result<Self> {
        let mut samples = HashMap::new();
        for def in SOUND_DEFS {
            let path = AssetPath::new(def.path);
            let Some(bytes) = assets
                .read(&path)
                .with_context(|| format!("failed to read sound asset {}", def.path))?
            else {
                log::warn!(
                    "missing sound asset {} for {}; playback will be silent",
                    def.path,
                    def.key.as_str()
                );
                continue;
            };
            let sample = decode_ogg(&bytes)
                .with_context(|| format!("failed to decode sound asset {}", def.path))?;
            log::info!(
                "loaded sound {} from {} ({} Hz, {} channels, {} frames)",
                def.key.as_str(),
                def.path,
                sample.sample_rate,
                sample.channels,
                sample.frame_count()
            );
            samples.insert(def.key, Arc::new(sample));
        }
        let mut ordered = samples.into_iter().collect::<Vec<_>>();
        ordered.sort_by_key(|(key, _)| key.as_str());
        let mut samples = Vec::with_capacity(ordered.len());
        let mut families = HashMap::new();
        for (key, sample) in ordered {
            let sample_index =
                u16::try_from(samples.len()).context("legacy sound bank has too many samples")?;
            samples.push(sample);
            families.insert(
                key,
                PreparedFamily {
                    variants: vec![sample_index],
                    gain: 1.0,
                    pitch_min: 1.0,
                    pitch_max: 1.0,
                    no_immediate_repeat: false,
                },
            );
        }
        Ok(Self { samples, families })
    }

    #[cfg(test)]
    fn from_samples(samples: impl IntoIterator<Item = (SoundKey, SampleData)>) -> Self {
        let mut bank = Self {
            samples: Vec::new(),
            families: HashMap::new(),
        };
        for (key, sample) in samples {
            let sample_index = u16::try_from(bank.samples.len()).unwrap();
            bank.samples.push(Arc::new(sample));
            bank.families.insert(
                key,
                PreparedFamily {
                    variants: vec![sample_index],
                    gain: 1.0,
                    pitch_min: 1.0,
                    pitch_max: 1.0,
                    no_immediate_repeat: false,
                },
            );
        }
        bank
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn sample(&self, sample_index: u16) -> Option<Arc<SampleData>> {
        self.samples.get(usize::from(sample_index)).cloned()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct VariantSelector {
    sequence: u64,
    last_variants: HashMap<SoundKey, usize>,
}

#[cfg(not(target_arch = "wasm32"))]
impl VariantSelector {
    fn resolve(
        &mut self,
        bank: &SoundBank,
        sound: SoundKey,
        params: PlaybackParams,
    ) -> Option<ResolvedPlayback> {
        let family = bank.families.get(&sound)?;
        if !params.gain.is_finite() || params.gain <= 0.0 {
            return None;
        }
        let hash =
            mix64(params.seed ^ self.sequence ^ stable_sound_hash(sound.as_str()).rotate_left(17));
        self.sequence = self.sequence.wrapping_add(1);
        let mut variant = hash as usize % family.variants.len();
        if family.no_immediate_repeat
            && family.variants.len() > 1
            && self.last_variants.get(&sound) == Some(&variant)
        {
            variant = (variant + 1 + (hash >> 32) as usize % (family.variants.len() - 1))
                % family.variants.len();
        }
        self.last_variants.insert(sound, variant);
        let pitch_unit =
            (mix64(hash ^ 0xa076_1d64_78bd_642f) >> 40) as f32 / ((1_u32 << 24) - 1) as f32;
        let family_pitch = family.pitch_min + (family.pitch_max - family.pitch_min) * pitch_unit;
        let requested_pitch = if params.pitch.is_finite() {
            params.pitch
        } else {
            1.0
        };
        let pan = if params.pan.is_finite() {
            params.pan.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        Some(ResolvedPlayback {
            sample_index: family.variants[variant],
            gain: (family.gain * params.gain).clamp(0.0, 1.0),
            pitch: (family_pitch * requested_pitch).clamp(0.5, 2.0),
            pan,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn stable_sound_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

struct SoundDef {
    key: SoundKey,
    path: &'static str,
}

const SOUND_DEFS: &[SoundDef] = &[
    SoundDef {
        key: LANDING_SMALL,
        path: "assets/minecraft/sounds/damage/fallsmall.ogg",
    },
    SoundDef {
        key: LANDING_BIG,
        path: "assets/minecraft/sounds/damage/fallbig.ogg",
    },
];

fn decode_ogg(bytes: &[u8]) -> Result<SampleData> {
    let cursor = Cursor::new(bytes);
    let mut reader = OggStreamReader::new(cursor).context("invalid OGG stream")?;
    let channels = usize::from(reader.ident_hdr.audio_channels);
    let sample_rate = reader.ident_hdr.audio_sample_rate;
    let mut data = Vec::new();
    while let Some(packet) = reader
        .read_dec_packet_itl()
        .context("failed to decode OGG packet")?
    {
        data.extend(packet.into_iter().map(|sample| f32::from(sample) / 32768.0));
    }
    SampleData::new(channels, sample_rate, data)
}

#[cfg(not(target_arch = "wasm32"))]
struct Voice {
    sample: Arc<SampleData>,
    frame_position: f64,
    gain: f32,
    pitch: f32,
    pan: f32,
}

#[cfg(not(target_arch = "wasm32"))]
impl Voice {
    fn new(sample: Arc<SampleData>, gain: f32, pitch: f32, pan: f32) -> Self {
        Self {
            sample,
            frame_position: 0.0,
            gain,
            pitch,
            pan,
        }
    }

    fn advance(&mut self, output_sample_rate: u32) {
        self.frame_position += f64::from(self.sample.sample_rate) / f64::from(output_sample_rate)
            * f64::from(self.pitch);
    }

    fn is_finished(&self) -> bool {
        self.frame_position >= self.sample.frame_count() as f64
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct Mixer {
    bank: SoundBank,
    commands: Receiver<AudioCommand>,
    settings: AudioSettings,
    output_sample_rate: u32,
    output_channels: usize,
    max_voices: usize,
    voices: Vec<Voice>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Mixer {
    fn new(
        bank: SoundBank,
        commands: Receiver<AudioCommand>,
        settings: AudioSettings,
        output_sample_rate: u32,
        max_voices: usize,
    ) -> Self {
        let max_voices = max_voices.max(1);
        Self {
            bank,
            commands,
            settings,
            output_sample_rate: output_sample_rate.max(1),
            output_channels: 1,
            max_voices,
            voices: Vec::with_capacity(max_voices),
        }
    }

    #[cfg(test)]
    fn render(&mut self, output: &mut [f32], channels: usize) {
        self.render_typed(output, channels);
    }

    fn render_typed<T>(&mut self, output: &mut [T], channels: usize)
    where
        T: Sample + FromSample<f32>,
    {
        let channels = channels.max(1);
        self.output_channels = channels;
        self.drain_commands();

        if !self.settings.enabled {
            for sample in output {
                *sample = T::EQUILIBRIUM;
            }
            return;
        }

        for frame in output.chunks_mut(channels) {
            for (channel, sample) in frame.iter_mut().enumerate() {
                let mut mixed = 0.0;
                for voice in &self.voices {
                    mixed +=
                        voice
                            .sample
                            .value_at(voice.frame_position, channel, self.output_channels)
                            * voice.gain
                            * pan_gain(voice.pan, channel, self.output_channels)
                            * self.settings.master_gain;
                }
                *sample = T::from_sample(mixed.clamp(-1.0, 1.0));
            }
            for voice in &mut self.voices {
                voice.advance(self.output_sample_rate);
            }
        }
        self.voices.retain(|voice| !voice.is_finished());
    }

    fn drain_commands(&mut self) {
        loop {
            match self.commands.try_recv() {
                Ok(AudioCommand::Play(playback)) => self.start_voice(playback),
                Ok(AudioCommand::Settings(settings)) => self.settings = settings,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn start_voice(&mut self, playback: ResolvedPlayback) {
        if !self.settings.enabled || self.voices.len() >= self.max_voices {
            return;
        }
        let Some(sample) = self.bank.sample(playback.sample_index) else {
            return;
        };
        self.voices.push(Voice::new(
            sample,
            playback.gain.clamp(0.0, 1.0),
            playback.pitch.clamp(0.5, 2.0),
            playback.pan.clamp(-1.0, 1.0),
        ));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn pan_gain(pan: f32, output_channel: usize, output_channels: usize) -> f32 {
    if output_channels < 2 {
        return 1.0;
    }
    match output_channel {
        0 => 1.0 - pan.max(0.0),
        1 => 1.0 + pan.min(0.0),
        _ => 1.0,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn build_output_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    mixer: Mixer,
) -> Result<cpal::Stream> {
    match sample_format {
        SampleFormat::F32 => build_typed_output_stream::<f32>(device, config, mixer),
        SampleFormat::I16 => build_typed_output_stream::<i16>(device, config, mixer),
        SampleFormat::U16 => build_typed_output_stream::<u16>(device, config, mixer),
        other => bail!("unsupported audio output sample format {other:?}"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn build_typed_output_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut mixer: Mixer,
) -> Result<cpal::Stream>
where
    T: Sample + SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let stream = device.build_output_stream(
        config.clone(),
        move |data: &mut [T], _| mixer.render_typed(data, channels),
        move |err| log::warn!("audio output stream error: {err}"),
        None,
    )?;
    Ok(stream)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn test_sample(values: &[f32]) -> SampleData {
        SampleData::new(1, 4, values.to_vec()).unwrap()
    }

    fn test_mixer(
        samples: impl IntoIterator<Item = (SoundKey, SampleData)>,
        max_voices: usize,
    ) -> (SyncSender<AudioCommand>, Mixer) {
        let (sender, receiver) = sync_channel(8);
        let bank = SoundBank::from_samples(samples);
        (
            sender,
            Mixer::new(bank, receiver, AudioSettings::default(), 4, max_voices),
        )
    }

    fn playback(gain: f32) -> ResolvedPlayback {
        ResolvedPlayback {
            sample_index: 0,
            gain,
            pitch: 1.0,
            pan: 0.0,
        }
    }

    #[test]
    fn mixer_renders_sample_and_stops_at_end() {
        let (sender, mut mixer) = test_mixer([(LANDING_SMALL, test_sample(&[0.25, -0.5]))], 4);
        sender.try_send(AudioCommand::Play(playback(1.0))).unwrap();
        let mut output = [1.0; 4];

        mixer.render(&mut output, 1);

        assert_eq!(output, [0.25, -0.5, 0.0, 0.0]);
        assert!(mixer.voices.is_empty());
    }

    #[test]
    fn mixer_overlapping_voices_sum() {
        let (sender, mut mixer) = test_mixer([(LANDING_SMALL, test_sample(&[0.25]))], 4);
        for _ in 0..2 {
            sender.try_send(AudioCommand::Play(playback(1.0))).unwrap();
        }
        let mut output = [0.0; 1];

        mixer.render(&mut output, 1);

        assert_eq!(output, [0.5]);
    }

    #[test]
    fn mixer_missing_sound_is_silent_noop() {
        let (sender, mut mixer) = test_mixer([], 4);
        sender.try_send(AudioCommand::Play(playback(1.0))).unwrap();
        let mut output = [1.0; 2];

        mixer.render(&mut output, 1);

        assert_eq!(output, [0.0, 0.0]);
        assert!(mixer.voices.is_empty());
    }

    #[test]
    fn mixer_respects_voice_capacity() {
        let (sender, mut mixer) = test_mixer([(LANDING_SMALL, test_sample(&[0.25]))], 1);
        for _ in 0..2 {
            sender.try_send(AudioCommand::Play(playback(1.0))).unwrap();
        }
        let mut output = [0.0; 1];

        mixer.render(&mut output, 1);

        assert_eq!(output, [0.25]);
    }

    #[test]
    fn landing_impact_selects_small_or_big_sample() {
        assert_eq!(landing_playback_for_impact(1.0).0, LANDING_SMALL);
        assert_eq!(landing_playback_for_impact(10.0).0, LANDING_BIG);
        assert!(landing_playback_for_impact(10.0).1 <= 1.0);
    }

    #[test]
    fn mixer_applies_pitch_and_stereo_pan() {
        let (sender, mut mixer) =
            test_mixer([(LANDING_SMALL, test_sample(&[0.25, 0.5, 0.75, 1.0]))], 4);
        sender
            .try_send(AudioCommand::Play(ResolvedPlayback {
                sample_index: 0,
                gain: 1.0,
                pitch: 2.0,
                pan: 1.0,
            }))
            .unwrap();
        let mut output = [0.0; 4];

        mixer.render(&mut output, 2);

        assert_eq!(output, [0.0, 0.25, 0.0, 0.75]);
    }

    #[test]
    fn variant_selector_is_stable_and_never_repeats_adjacent_variant() {
        let bank = SoundBank {
            samples: vec![
                Arc::new(test_sample(&[0.1])),
                Arc::new(test_sample(&[0.2])),
                Arc::new(test_sample(&[0.3])),
            ],
            families: HashMap::from([(
                FOOTSTEP_NEUTRAL,
                PreparedFamily {
                    variants: vec![0, 1, 2],
                    gain: 0.5,
                    pitch_min: 0.9,
                    pitch_max: 1.1,
                    no_immediate_repeat: true,
                },
            )]),
        };
        let sequence = |selector: &mut VariantSelector| {
            (0..16)
                .map(|seed| {
                    selector
                        .resolve(
                            &bank,
                            FOOTSTEP_NEUTRAL,
                            PlaybackParams {
                                gain: 0.8,
                                pitch: 1.0,
                                pan: 0.25,
                                seed,
                            },
                        )
                        .unwrap()
                })
                .collect::<Vec<_>>()
        };

        let first = sequence(&mut VariantSelector::default());
        let second = sequence(&mut VariantSelector::default());

        assert_eq!(first, second);
        assert!(
            first
                .windows(2)
                .all(|pair| pair[0].sample_index != pair[1].sample_index)
        );
        assert!(first.iter().all(|playback| {
            playback.gain == 0.4 && (0.9..=1.1).contains(&playback.pitch) && playback.pan == 0.25
        }));
    }

    #[test]
    fn checked_in_first_party_catalog_loads_all_selected_samples() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let source = mclone_assets::FilesystemAssetSource::new(repo_root);

        let prepared = PreparedAudioAssets::load_first_party(&source).unwrap();

        assert_eq!(prepared.sound_count(), 129);
        assert_eq!(prepared.family_count(), 38);
    }

    #[test]
    fn checked_in_ogg_family_renders_nonzero_panned_pcm() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let source = mclone_assets::FilesystemAssetSource::new(repo_root);
        let prepared = PreparedAudioAssets::load_first_party(&source).unwrap();
        let bank = prepared.bank;
        let playback = VariantSelector::default()
            .resolve(
                &bank,
                FOOTSTEP_WOOD,
                PlaybackParams {
                    pan: 1.0,
                    seed: 42,
                    ..PlaybackParams::default()
                },
            )
            .unwrap();
        let (sender, receiver) = sync_channel(1);
        let mut mixer = Mixer::new(bank, receiver, AudioSettings::default(), 48_000, 4);
        sender.try_send(AudioCommand::Play(playback)).unwrap();
        let mut output = vec![0.0; 96_000];

        mixer.render(&mut output, 2);

        assert!(output.chunks_exact(2).all(|frame| frame[0] == 0.0));
        assert!(output.chunks_exact(2).any(|frame| frame[1].abs() > 1.0e-5));
    }
}

#[cfg(test)]
mod capability_tests {
    use super::*;

    #[test]
    fn absent_audio_output_stays_silent_across_asset_replacement() {
        let output = AudioOutputCapability::Unavailable;
        assert!(!output.is_available());
        output.play(LANDING_SMALL, 1.0);
        assert!(matches!(
            output.replacement(PreparedAudioAssets::silent()).unwrap(),
            AudioOutputCapability::Unavailable
        ));
    }

    #[test]
    fn acoustic_materials_keep_snow_and_sand_semantically_distinct() {
        assert_eq!(
            AcousticMaterial::from_block_path("snow_block"),
            AcousticMaterial::Snow
        );
        assert_eq!(
            AcousticMaterial::from_block_path("sand"),
            AcousticMaterial::Soft
        );
        assert_eq!(
            AcousticMaterial::from_block_path("oak_planks"),
            AcousticMaterial::Wood
        );
        assert_eq!(
            AcousticMaterial::from_block_path("iron_ore"),
            AcousticMaterial::Metal
        );
        assert_eq!(AcousticMaterial::Soft.footstep_sound(), FOOTSTEP_NEUTRAL);
        assert_eq!(AcousticMaterial::Snow.footstep_sound(), FOOTSTEP_SNOW);
    }
}
