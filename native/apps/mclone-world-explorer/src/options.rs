use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mclone_view_control::{WorldViewMode, WorldViewProjection, WorldViewState};
use mclone_world_explorer::{WorldExplorerCompositionMode, WorldExplorerExactAnchor};

pub const DEFAULT_WIDTH: u32 = 1280;
pub const DEFAULT_HEIGHT: u32 = 720;
pub const DEFAULT_SEED: i64 = 12_345;
pub const DEFAULT_BLOCKS_ACROSS: u32 = 4_096;
pub const DEFAULT_EXACT_RADIUS: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerAssetProfile {
    Original,
    GeneratedFallback,
}

impl ExplorerAssetProfile {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "original" => Ok(Self::Original),
            "generated-fallback" | "fallback" => Ok(Self::GeneratedFallback),
            _ => bail!(
                "unsupported asset profile {value:?}; expected original or \
                 generated-fallback"
            ),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::GeneratedFallback => "generated-fallback",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExplorerOptions {
    pub width: u32,
    pub height: u32,
    pub seed: i64,
    pub center_x: i32,
    pub center_z: i32,
    pub blocks_across: u32,
    pub view: WorldViewMode,
    pub yaw_radians: f32,
    pub pitch_radians: f32,
    pub projection: WorldViewProjection,
    pub composition: WorldExplorerCompositionMode,
    pub source_colors: bool,
    pub exact_radius: u32,
    pub exact_anchor: WorldExplorerExactAnchor,
    pub exact_delay_ms: u64,
    pub asset_root: PathBuf,
    pub asset_profile: ExplorerAssetProfile,
    pub capture: Option<PathBuf>,
    pub window_capture: Option<PathBuf>,
    pub smoke_dir: Option<PathBuf>,
    pub window_smoke_dir: Option<PathBuf>,
}

impl Default for ExplorerOptions {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            seed: DEFAULT_SEED,
            center_x: 0,
            center_z: 0,
            blocks_across: DEFAULT_BLOCKS_ACROSS,
            view: WorldViewMode::Orbit,
            yaw_radians: std::f32::consts::FRAC_PI_4,
            pitch_radians: 0.52,
            projection: WorldViewProjection::Perspective,
            composition: WorldExplorerCompositionMode::Composed,
            source_colors: false,
            exact_radius: DEFAULT_EXACT_RADIUS,
            exact_anchor: WorldExplorerExactAnchor::Focus,
            exact_delay_ms: 0,
            asset_root: default_asset_root(),
            asset_profile: ExplorerAssetProfile::Original,
            capture: None,
            window_capture: None,
            smoke_dir: None,
            window_smoke_dir: None,
        }
    }
}

impl ExplorerOptions {
    pub fn parse() -> Result<Option<Self>> {
        let mut options = Self::default();
        let mut arguments = env::args_os();
        let _program = arguments.next();
        while let Some(argument) = arguments.next() {
            if argument == "--help" || argument == "-h" {
                print_help();
                return Ok(None);
            }
            let argument = argument
                .into_string()
                .map_err(|_| anyhow::anyhow!("arguments must be valid UTF-8"))?;
            let (name, inline_value) = argument
                .split_once('=')
                .map_or((argument.as_str(), None), |(name, value)| {
                    (name, Some(value.to_owned()))
                });
            let value = |arguments: &mut env::ArgsOs| -> Result<OsString> {
                if let Some(value) = inline_value.as_ref() {
                    return Ok(OsString::from(value));
                }
                arguments
                    .next()
                    .with_context(|| format!("{name} requires a value"))
            };
            match name {
                "--width" => options.width = parse_value(value(&mut arguments)?, name)?,
                "--height" => options.height = parse_value(value(&mut arguments)?, name)?,
                "--seed" => options.seed = parse_value(value(&mut arguments)?, name)?,
                "--center-x" => options.center_x = parse_value(value(&mut arguments)?, name)?,
                "--center-z" => options.center_z = parse_value(value(&mut arguments)?, name)?,
                "--blocks-across" => {
                    options.blocks_across = parse_value(value(&mut arguments)?, name)?
                }
                "--yaw" => options.yaw_radians = parse_value(value(&mut arguments)?, name)?,
                "--pitch" => options.pitch_radians = parse_value(value(&mut arguments)?, name)?,
                "--view" => {
                    let value = utf8_value(value(&mut arguments)?, name)?;
                    options.view = match value.as_str() {
                        "map" => WorldViewMode::Map,
                        "3d" => WorldViewMode::Orbit,
                        _ => bail!("unsupported view {value:?}; expected map or 3d"),
                    };
                }
                "--projection" => {
                    let value = utf8_value(value(&mut arguments)?, name)?;
                    options.projection = match value.as_str() {
                        "orthographic" => WorldViewProjection::Orthographic,
                        "perspective" => WorldViewProjection::Perspective,
                        _ => bail!(
                            "unsupported projection {value:?}; expected orthographic or perspective"
                        ),
                    };
                }
                "--composition" => {
                    options.composition = WorldExplorerCompositionMode::parse_label(&utf8_value(
                        value(&mut arguments)?,
                        name,
                    )?)
                    .map_err(anyhow::Error::msg)?
                }
                "--source-colors" => options.source_colors = true,
                "--exact-radius" => {
                    options.exact_radius = parse_value(value(&mut arguments)?, name)?
                }
                "--exact-anchor" => {
                    options.exact_anchor = WorldExplorerExactAnchor::parse_label(&utf8_value(
                        value(&mut arguments)?,
                        name,
                    )?)
                    .map_err(anyhow::Error::msg)?
                }
                "--exact-delay-ms" => {
                    options.exact_delay_ms = parse_value(value(&mut arguments)?, name)?
                }
                "--asset-root" => options.asset_root = PathBuf::from(value(&mut arguments)?),
                "--asset-profile" => {
                    options.asset_profile =
                        ExplorerAssetProfile::parse(&utf8_value(value(&mut arguments)?, name)?)?
                }
                "--capture" => options.capture = Some(PathBuf::from(value(&mut arguments)?)),
                "--window-capture" => {
                    options.window_capture = Some(PathBuf::from(value(&mut arguments)?))
                }
                "--smoke" => options.smoke_dir = Some(PathBuf::from(value(&mut arguments)?)),
                _ => bail!("unknown World Explorer option {name:?}; use --help"),
            }
        }
        options.validate()?;
        Ok(Some(options))
    }

    fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            bail!("World Explorer dimensions must be non-zero");
        }
        if !self.yaw_radians.is_finite() || !self.pitch_radians.is_finite() {
            bail!("World Explorer camera angles must be finite");
        }
        if self.asset_root.as_os_str().is_empty() {
            bail!("World Explorer asset root must not be empty");
        }
        if self.exact_radius > 8 {
            bail!("World Explorer exact radius must be at most 8 chunks");
        }
        if self.source_colors && self.composition == WorldExplorerCompositionMode::Horizon {
            bail!("--source-colors requires exact, composed, or coverage composition");
        }
        if usize::from(self.capture.is_some())
            + usize::from(self.window_capture.is_some())
            + usize::from(self.smoke_dir.is_some())
            > 1
        {
            bail!("--capture, --window-capture, and --smoke are mutually exclusive");
        }
        Ok(())
    }

    pub fn view_label(&self) -> &'static str {
        match self.view {
            WorldViewMode::Map => "map",
            WorldViewMode::Orbit => "3d",
        }
    }

    pub fn initial_view_state(&self) -> WorldViewState {
        WorldViewState {
            mode: self.view,
            focus_x: f64::from(self.center_x),
            focus_z: f64::from(self.center_z),
            blocks_across: f64::from(self.blocks_across),
            yaw_radians: f64::from(self.yaw_radians),
            pitch_radians: f64::from(self.pitch_radians),
            projection: self.projection,
        }
    }

    pub fn title(&self) -> String {
        format!(
            "Mclone World Explorer — {} — seed {} — ({}, {}) — {} blocks — {}",
            self.composition.label(),
            self.seed,
            self.center_x,
            self.center_z,
            self.blocks_across,
            self.view_label()
        )
    }

    pub fn asset_bytes(&self) -> Result<u64> {
        let mut bytes = file_bytes(&self.asset_root.join("mclone-generated-fallback.pbp"))?;
        if self.asset_profile == ExplorerAssetProfile::Original {
            bytes = bytes.saturating_add(file_bytes(&self.asset_root.join("mclone-authored.pbp"))?);
        }
        let diagnostic = self.asset_root.join("mclone-diagnostic-missing.pbp");
        if diagnostic.is_file() {
            bytes = bytes.saturating_add(file_bytes(&diagnostic)?);
        }
        Ok(bytes)
    }
}

fn parse_value<T>(value: OsString, name: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let value = utf8_value(value, name)?;
    value
        .parse()
        .map_err(|error| anyhow::anyhow!("invalid {name} value {value:?}: {error}"))
}

fn utf8_value(value: OsString, name: &str) -> Result<String> {
    value
        .into_string()
        .map_err(|_| anyhow::anyhow!("{name} value must be valid UTF-8"))
}

fn file_bytes(path: &Path) -> Result<u64> {
    Ok(std::fs::metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?
        .len())
}

fn default_asset_root() -> PathBuf {
    if let Some(path) = env::var_os("MCLONE_WORLD_EXPLORER_ASSET_ROOT") {
        return PathBuf::from(path);
    }
    let from_working_directory =
        PathBuf::from("generated-assets/first-party-stage/first-party-packs");
    if from_working_directory.is_dir() {
        return from_working_directory;
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("generated-assets/first-party-stage/first-party-packs")
}

fn print_help() {
    println!(
        "\
Mclone World Explorer

Usage: mclone-world-explorer [options]

  --seed N                    terrain seed (default {DEFAULT_SEED})
  --center-x N                view center X (default 0)
  --center-z N                view center Z (default 0)
  --blocks-across N           horizontal footprint (default {DEFAULT_BLOCKS_ACROSS})
  --view map|3d               camera mode (default 3d)
  --projection TYPE           perspective or orthographic
  --composition MODE          horizon, exact, composed (default), or coverage
  --source-colors             render exact geometry in diagnostic magenta
  --exact-radius N            exact near-field chunk radius (default {DEFAULT_EXACT_RADIUS})
  --exact-anchor MODE         focus (default) or viewer-forward
  --exact-delay-ms N          diagnostic delay before each exact batch
  --yaw RADIANS               3D yaw
  --pitch RADIANS             3D pitch
  --width N                   physical width (default {DEFAULT_WIDTH})
  --height N                  physical height (default {DEFAULT_HEIGHT})
  --asset-root PATH           directory containing first-party .pbp files
  --asset-profile PROFILE     original or generated-fallback
  --capture PATH              render an offscreen PNG instead of opening a window
  --window-capture PATH       capture the ready native surface and exit
  --smoke DIR                 run offscreen and native-surface movement smoke
  -h, --help                  show this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_defaults_to_composed_terrain() {
        assert_eq!(
            ExplorerOptions::default().composition,
            WorldExplorerCompositionMode::Composed
        );
    }
}
