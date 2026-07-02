use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use mclone_core::{
    AIR_BLOCK_STATE_ID, BlockStateId, LIGHT_DATA_LAYER_BYTE_COUNT, PackedLightSection,
    chunk_block_index, chunk_section_index,
};
use mclone_mesh::{
    TexturedChunkMeshInput, TexturedRenderSectionMesh, build_textured_render_sections,
};
use mclone_render::chunk::{ChunkCamera, TexturedSectionRenderOptions};
use mclone_render::headless::{
    HeadlessChunkOptions, HeadlessChunkReport, write_headless_textured_sections_png_with_options,
};

use crate::cli::TorchLightProbeOptions;
use crate::render_cache::load_textured_mesh_assets;

const STONE: BlockStateId = BlockStateId(1);
const TORCH: BlockStateId = BlockStateId(100);
const ROOM_MIN_X: i32 = 5;
const ROOM_MAX_X: i32 = 11;
const ROOM_MIN_Y: i32 = 0;
const ROOM_MAX_Y: i32 = 7;
const ROOM_MIN_Z: i32 = 5;
const ROOM_MAX_Z: i32 = 11;
const TORCH_POS: [i32; 3] = [8, 1, 8];
const UNIFORM_SKY_LIGHT: u8 = 6;
const SKYLIGHT_MIN_X: i32 = 7;
const SKYLIGHT_MAX_X: i32 = 9;
const SKYLIGHT_MIN_Z: i32 = 7;
const SKYLIGHT_MAX_Z: i32 = 9;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TorchLightProbeSample {
    pub(crate) label: &'static str,
    pub(crate) position: [i32; 3],
    pub(crate) block_light: u8,
    pub(crate) uniform_sky_light: u8,
    pub(crate) skylight_opening_sky_light: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TorchLightProbeFrameReport {
    pub(crate) label: &'static str,
    pub(crate) path: PathBuf,
    pub(crate) byte_len: usize,
    pub(crate) section_count: usize,
    pub(crate) vertex_count: u32,
    pub(crate) index_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TorchLightProbeReport {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frames: Vec<TorchLightProbeFrameReport>,
    pub(crate) samples: Vec<TorchLightProbeSample>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TorchLightProbeGeometry {
    Closed,
    SkylightOpening,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TorchLightProbeSky {
    None,
    Uniform,
    SkylightOpening,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TorchLightProbeSceneConfig {
    geometry: TorchLightProbeGeometry,
    sky: TorchLightProbeSky,
}

impl TorchLightProbeSceneConfig {
    const BLOCK_ONLY: Self = Self {
        geometry: TorchLightProbeGeometry::Closed,
        sky: TorchLightProbeSky::None,
    };
    const UNIFORM_SKY_MIX: Self = Self {
        geometry: TorchLightProbeGeometry::Closed,
        sky: TorchLightProbeSky::Uniform,
    };
    const SKYLIGHT_OPENING: Self = Self {
        geometry: TorchLightProbeGeometry::SkylightOpening,
        sky: TorchLightProbeSky::SkylightOpening,
    };
}

pub(crate) fn run_torch_light_probe(
    options: &TorchLightProbeOptions,
) -> Result<TorchLightProbeReport> {
    std::fs::create_dir_all(&options.directory).with_context(|| {
        format!(
            "failed to create torch light probe output directory {}",
            options.directory.display()
        )
    })?;

    let mesh_assets = load_textured_mesh_assets()?;
    let block_only_scene =
        build_torch_light_probe_scene(&mesh_assets.catalog, TorchLightProbeSceneConfig::BLOCK_ONLY)
            .context("failed to build block-only torch light probe scene")?;
    let uniform_sky_scene = build_torch_light_probe_scene(
        &mesh_assets.catalog,
        TorchLightProbeSceneConfig::UNIFORM_SKY_MIX,
    )
    .context("failed to build uniform-sky torch light probe scene")?;
    let skylight_opening_scene = build_torch_light_probe_scene(
        &mesh_assets.catalog,
        TorchLightProbeSceneConfig::SKYLIGHT_OPENING,
    )
    .context("failed to build skylight-opening torch light probe scene")?;

    let lit_path = options.directory.join("lit.png");
    let fullbright_path = options.directory.join("fullbright.png");
    let sky_mix_path = options.directory.join("sky_mix.png");
    let skylight_opening_path = options.directory.join("skylight_opening.png");
    let skylight_opening_fullbright_path =
        options.directory.join("skylight_opening_fullbright.png");
    let mut render_options = options.render_options;
    render_options.section_occlusion_culling = false;

    let mut frames = Vec::new();
    frames.push(write_labeled_probe_frame(
        "block_only",
        lit_path,
        options.width,
        options.height,
        &block_only_scene.sections,
        mesh_assets.atlas.as_upload(),
        render_options,
    )?);
    frames.push(write_labeled_probe_frame(
        "sky_mix",
        sky_mix_path,
        options.width,
        options.height,
        &uniform_sky_scene.sections,
        mesh_assets.atlas.as_upload(),
        render_options,
    )?);
    frames.push(write_labeled_probe_frame(
        "skylight_opening",
        skylight_opening_path,
        options.width,
        options.height,
        &skylight_opening_scene.sections,
        mesh_assets.atlas.as_upload(),
        render_options,
    )?);

    let mut fullbright_options = render_options;
    fullbright_options.force_fullbright = true;
    frames.push(write_labeled_probe_frame(
        "fullbright",
        fullbright_path,
        options.width,
        options.height,
        &block_only_scene.sections,
        mesh_assets.atlas.as_upload(),
        fullbright_options,
    )?);
    frames.push(write_labeled_probe_frame(
        "skylight_opening_fullbright",
        skylight_opening_fullbright_path,
        options.width,
        options.height,
        &skylight_opening_scene.sections,
        mesh_assets.atlas.as_upload(),
        fullbright_options,
    )?);

    if frames.is_empty() {
        bail!("torch light probe rendered no frames");
    }
    if frames
        .iter()
        .any(|frame| frame.section_count == 0 || frame.vertex_count == 0 || frame.index_count == 0)
    {
        bail!("torch light probe rendered an empty frame");
    }
    if frames
        .iter()
        .any(|frame| frame.byte_len != (options.width * options.height * 4) as usize)
    {
        bail!(
            "torch light probe frame byte count did not match {}x{} RGBA output",
            options.width,
            options.height
        );
    }

    Ok(TorchLightProbeReport {
        width: options.width,
        height: options.height,
        frames,
        samples: torch_light_probe_samples(),
    })
}

fn write_labeled_probe_frame(
    label: &'static str,
    path: PathBuf,
    width: u32,
    height: u32,
    sections: &[TexturedRenderSectionMesh],
    atlas: mclone_render::chunk::ChunkTextureAtlas<'_>,
    render_options: TexturedSectionRenderOptions,
) -> Result<TorchLightProbeFrameReport> {
    if sections.is_empty() {
        bail!("torch light probe frame `{label}` has no render sections");
    }
    let report = write_probe_frame(path.clone(), width, height, sections, atlas, render_options)
        .with_context(|| format!("failed to render `{label}` torch light probe frame"))?;

    Ok(TorchLightProbeFrameReport {
        label,
        path,
        byte_len: report.byte_len,
        section_count: sections.len(),
        vertex_count: report.vertex_count,
        index_count: report.index_count,
    })
}

fn write_probe_frame(
    path: PathBuf,
    width: u32,
    height: u32,
    sections: &[TexturedRenderSectionMesh],
    atlas: mclone_render::chunk::ChunkTextureAtlas<'_>,
    render_options: TexturedSectionRenderOptions,
) -> Result<HeadlessChunkReport> {
    write_headless_textured_sections_png_with_options(
        HeadlessChunkOptions {
            path,
            width,
            height,
            color: wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            camera: torch_light_probe_camera(),
        },
        sections,
        atlas,
        render_options,
    )
}

struct TorchLightProbeScene {
    sections: Vec<TexturedRenderSectionMesh>,
}

fn build_torch_light_probe_scene(
    catalog: &mclone_mesh::TexturedMeshCatalog,
    config: TorchLightProbeSceneConfig,
) -> Result<TorchLightProbeScene> {
    let blocks = torch_light_probe_blocks(config.geometry);
    let light_sections = torch_light_probe_light_sections(config.sky);
    let sections = build_textured_render_sections(
        &[TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks).with_light_sections(&light_sections)],
        catalog,
    )
    .context("failed to build torch light probe render sections")?;

    Ok(TorchLightProbeScene { sections })
}

fn torch_light_probe_blocks(geometry: TorchLightProbeGeometry) -> Vec<BlockStateId> {
    let mut blocks = vec![AIR_BLOCK_STATE_ID; 16 * 16 * 16];
    for y in ROOM_MIN_Y..=ROOM_MAX_Y {
        for z in ROOM_MIN_Z..=ROOM_MAX_Z {
            for x in ROOM_MIN_X..=ROOM_MAX_X {
                if is_room_shell(x, y, z) && !is_skylight_aperture(geometry, x, y, z) {
                    blocks[chunk_block_index(x, y, z)] = STONE;
                }
            }
        }
    }
    blocks[chunk_block_index(TORCH_POS[0], TORCH_POS[1], TORCH_POS[2])] = TORCH;
    blocks
}

fn torch_light_probe_light_sections(sky: TorchLightProbeSky) -> Vec<PackedLightSection> {
    let mut block = vec![0; LIGHT_DATA_LAYER_BYTE_COUNT];
    let mut sky_layer = match sky {
        TorchLightProbeSky::None => None,
        TorchLightProbeSky::Uniform | TorchLightProbeSky::SkylightOpening => {
            Some(vec![0; LIGHT_DATA_LAYER_BYTE_COUNT])
        }
    };
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let block_light = torch_light_probe_block_light_at(x, y, z);
                if block_light > 0 {
                    set_data_layer_value(&mut block, x, y, z, block_light);
                }
                if let Some(sky_layer) = sky_layer.as_mut() {
                    let sky_light = torch_light_probe_sky_light_at(sky, x, y, z);
                    if sky_light > 0 {
                        set_data_layer_value(sky_layer, x, y, z, sky_light);
                    }
                }
            }
        }
    }
    vec![PackedLightSection::new(0, sky_layer, Some(block))]
}

fn set_data_layer_value(bytes: &mut [u8], x: i32, y: i32, z: i32, value: u8) {
    debug_assert_eq!(bytes.len(), LIGHT_DATA_LAYER_BYTE_COUNT);
    debug_assert!(value <= 15);
    let index = chunk_section_index(x, y, z);
    let byte_index = index >> 1;
    let shift = 4 * (index & 1);
    let mask = !(15 << shift);
    bytes[byte_index] = (bytes[byte_index] & mask) | ((value & 15) << shift);
}

fn torch_light_probe_samples() -> Vec<TorchLightProbeSample> {
    [
        ("source", TORCH_POS),
        ("adjacent_air", [8, 1, 9]),
        ("above_air", [8, 2, 8]),
        ("two_blocks_out", [10, 1, 8]),
        ("under_skylight", [8, 6, 8]),
        ("outside_shell", [4, 1, 8]),
    ]
    .into_iter()
    .map(|(label, position)| TorchLightProbeSample {
        label,
        position,
        block_light: torch_light_probe_block_light_at(position[0], position[1], position[2]),
        uniform_sky_light: torch_light_probe_sky_light_at(
            TorchLightProbeSky::Uniform,
            position[0],
            position[1],
            position[2],
        ),
        skylight_opening_sky_light: torch_light_probe_sky_light_at(
            TorchLightProbeSky::SkylightOpening,
            position[0],
            position[1],
            position[2],
        ),
    })
    .collect()
}

fn torch_light_probe_block_light_at(x: i32, y: i32, z: i32) -> u8 {
    if !inside_room_bounds(x, y, z) || is_room_shell(x, y, z) {
        return 0;
    }
    let distance = x.abs_diff(TORCH_POS[0]) + y.abs_diff(TORCH_POS[1]) + z.abs_diff(TORCH_POS[2]);
    14_u8.saturating_sub(distance as u8)
}

fn torch_light_probe_sky_light_at(sky: TorchLightProbeSky, x: i32, y: i32, z: i32) -> u8 {
    match sky {
        TorchLightProbeSky::None => 0,
        TorchLightProbeSky::Uniform => {
            if inside_room_bounds(x, y, z) && !is_room_shell(x, y, z) {
                UNIFORM_SKY_LIGHT
            } else {
                0
            }
        }
        TorchLightProbeSky::SkylightOpening => skylight_opening_sky_light_at(x, y, z),
    }
}

fn skylight_opening_sky_light_at(x: i32, y: i32, z: i32) -> u8 {
    if !inside_room_bounds(x, y, z) {
        return 0;
    }
    if is_room_shell(x, y, z)
        && !is_skylight_aperture(TorchLightProbeGeometry::SkylightOpening, x, y, z)
    {
        return 0;
    }

    let dx = distance_to_range(x, SKYLIGHT_MIN_X, SKYLIGHT_MAX_X);
    let dz = distance_to_range(z, SKYLIGHT_MIN_Z, SKYLIGHT_MAX_Z);
    let vertical_drop = (ROOM_MAX_Y - y).max(0) as u32;
    15_u8.saturating_sub((dx + dz + vertical_drop / 2) as u8)
}

fn inside_room_bounds(x: i32, y: i32, z: i32) -> bool {
    (ROOM_MIN_X..=ROOM_MAX_X).contains(&x)
        && (ROOM_MIN_Y..=ROOM_MAX_Y).contains(&y)
        && (ROOM_MIN_Z..=ROOM_MAX_Z).contains(&z)
}

fn is_room_shell(x: i32, y: i32, z: i32) -> bool {
    inside_room_bounds(x, y, z)
        && (x == ROOM_MIN_X
            || x == ROOM_MAX_X
            || y == ROOM_MIN_Y
            || y == ROOM_MAX_Y
            || z == ROOM_MIN_Z
            || z == ROOM_MAX_Z)
}

fn is_skylight_aperture(geometry: TorchLightProbeGeometry, x: i32, y: i32, z: i32) -> bool {
    geometry == TorchLightProbeGeometry::SkylightOpening
        && y == ROOM_MAX_Y
        && (SKYLIGHT_MIN_X..=SKYLIGHT_MAX_X).contains(&x)
        && (SKYLIGHT_MIN_Z..=SKYLIGHT_MAX_Z).contains(&z)
}

fn distance_to_range(value: i32, min: i32, max: i32) -> u32 {
    if value < min {
        min.abs_diff(value)
    } else if value > max {
        value.abs_diff(max)
    } else {
        0
    }
}

fn torch_light_probe_camera() -> ChunkCamera {
    ChunkCamera {
        eye: [8.5, 4.2, 10.45],
        target: [8.5, 1.35, 8.05],
        up: [0.0, 1.0, 0.0],
        fov_y_radians: 58.0_f32.to_radians(),
        z_near: 0.03,
        z_far: 32.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torch_light_probe_samples_match_java_synthetic_case() {
        let samples = torch_light_probe_samples();

        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.block_light)
                .collect::<Vec<_>>(),
            [14, 13, 13, 12, 9, 0]
        );
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.uniform_sky_light)
                .collect::<Vec<_>>(),
            [6, 6, 6, 6, 6, 0]
        );
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.skylight_opening_sky_light)
                .collect::<Vec<_>>(),
            [12, 12, 13, 11, 15, 0]
        );
    }

    #[test]
    fn torch_light_probe_room_places_torch_inside_stone_shell() {
        let blocks = torch_light_probe_blocks(TorchLightProbeGeometry::Closed);

        assert_eq!(blocks[chunk_block_index(8, 1, 8)], TORCH);
        assert_eq!(blocks[chunk_block_index(8, 0, 8)], STONE);
        assert_eq!(blocks[chunk_block_index(8, 2, 8)], AIR_BLOCK_STATE_ID);
        assert_eq!(blocks[chunk_block_index(4, 1, 8)], AIR_BLOCK_STATE_ID);
    }

    #[test]
    fn torch_light_probe_skylight_opening_cuts_ceiling_aperture() {
        let blocks = torch_light_probe_blocks(TorchLightProbeGeometry::SkylightOpening);

        assert_eq!(
            blocks[chunk_block_index(SKYLIGHT_MIN_X, ROOM_MAX_Y, SKYLIGHT_MIN_Z)],
            AIR_BLOCK_STATE_ID
        );
        assert_eq!(
            blocks[chunk_block_index(SKYLIGHT_MIN_X - 1, ROOM_MAX_Y, SKYLIGHT_MIN_Z)],
            STONE
        );
    }
}
