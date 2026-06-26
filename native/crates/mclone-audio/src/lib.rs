#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};

use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use lewton::inside_ogg::OggStreamReader;
use mclone_assets::{AssetPath, AssetSource};

const COMMAND_QUEUE_CAPACITY: usize = 128;
const DEFAULT_MAX_VOICES: usize = 32;
const LANDING_BIG_IMPACT_SPEED: f64 = 6.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SoundKey(&'static str);

impl SoundKey {
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

pub const LANDING_SMALL: SoundKey = SoundKey("mclone:landing_small");
pub const LANDING_BIG: SoundKey = SoundKey("mclone:landing_big");

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

pub struct AudioEngine {
    commands: SyncSender<AudioCommand>,
    _stream: cpal::Stream,
}

impl AudioEngine {
    pub fn new(assets: &impl AssetSource, settings: AudioSettings) -> Result<Self> {
        let bank = SoundBank::load(assets).context("failed to load audio sound bank")?;
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
            bank,
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
            _stream: stream,
        })
    }

    pub fn play(&self, sound: SoundKey, gain: f32) {
        let gain = gain.clamp(0.0, 1.0);
        if gain <= 0.0 {
            return;
        }
        match self.commands.try_send(AudioCommand::Play { sound, gain }) {
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum AudioCommand {
    Play { sound: SoundKey, gain: f32 },
    Settings(AudioSettings),
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
    samples: HashMap<SoundKey, Arc<SampleData>>,
}

impl SoundBank {
    fn load(assets: &impl AssetSource) -> Result<Self> {
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
        Ok(Self { samples })
    }

    #[cfg(test)]
    fn from_samples(samples: impl IntoIterator<Item = (SoundKey, SampleData)>) -> Self {
        Self {
            samples: samples
                .into_iter()
                .map(|(key, sample)| (key, Arc::new(sample)))
                .collect(),
        }
    }

    fn sample(&self, sound: SoundKey) -> Option<Arc<SampleData>> {
        self.samples.get(&sound).cloned()
    }
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

struct Voice {
    sample: Arc<SampleData>,
    frame_position: f64,
    gain: f32,
}

impl Voice {
    fn new(sample: Arc<SampleData>, gain: f32) -> Self {
        Self {
            sample,
            frame_position: 0.0,
            gain,
        }
    }

    fn advance(&mut self, output_sample_rate: u32) {
        self.frame_position += f64::from(self.sample.sample_rate) / f64::from(output_sample_rate);
    }

    fn is_finished(&self) -> bool {
        self.frame_position >= self.sample.frame_count() as f64
    }
}

struct Mixer {
    bank: SoundBank,
    commands: Receiver<AudioCommand>,
    settings: AudioSettings,
    output_sample_rate: u32,
    output_channels: usize,
    max_voices: usize,
    voices: Vec<Voice>,
}

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
                Ok(AudioCommand::Play { sound, gain }) => self.start_voice(sound, gain),
                Ok(AudioCommand::Settings(settings)) => self.settings = settings,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn start_voice(&mut self, sound: SoundKey, gain: f32) {
        if !self.settings.enabled || self.voices.len() >= self.max_voices {
            return;
        }
        let Some(sample) = self.bank.sample(sound) else {
            return;
        };
        self.voices.push(Voice::new(sample, gain.clamp(0.0, 1.0)));
    }
}

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

#[cfg(test)]
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

    #[test]
    fn mixer_renders_sample_and_stops_at_end() {
        let (sender, mut mixer) = test_mixer([(LANDING_SMALL, test_sample(&[0.25, -0.5]))], 4);
        sender
            .try_send(AudioCommand::Play {
                sound: LANDING_SMALL,
                gain: 1.0,
            })
            .unwrap();
        let mut output = [1.0; 4];

        mixer.render(&mut output, 1);

        assert_eq!(output, [0.25, -0.5, 0.0, 0.0]);
        assert!(mixer.voices.is_empty());
    }

    #[test]
    fn mixer_overlapping_voices_sum() {
        let (sender, mut mixer) = test_mixer([(LANDING_SMALL, test_sample(&[0.25]))], 4);
        for _ in 0..2 {
            sender
                .try_send(AudioCommand::Play {
                    sound: LANDING_SMALL,
                    gain: 1.0,
                })
                .unwrap();
        }
        let mut output = [0.0; 1];

        mixer.render(&mut output, 1);

        assert_eq!(output, [0.5]);
    }

    #[test]
    fn mixer_missing_sound_is_silent_noop() {
        let (sender, mut mixer) = test_mixer([], 4);
        sender
            .try_send(AudioCommand::Play {
                sound: LANDING_SMALL,
                gain: 1.0,
            })
            .unwrap();
        let mut output = [1.0; 2];

        mixer.render(&mut output, 1);

        assert_eq!(output, [0.0, 0.0]);
        assert!(mixer.voices.is_empty());
    }

    #[test]
    fn mixer_respects_voice_capacity() {
        let (sender, mut mixer) = test_mixer([(LANDING_SMALL, test_sample(&[0.25]))], 1);
        for _ in 0..2 {
            sender
                .try_send(AudioCommand::Play {
                    sound: LANDING_SMALL,
                    gain: 1.0,
                })
                .unwrap();
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
}
