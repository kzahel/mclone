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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TorchLightProbeSample {
    pub(crate) label: &'static str,
    pub(crate) position: [i32; 3],
    pub(crate) block_light: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TorchLightProbeReport {
    pub(crate) lit_path: PathBuf,
    pub(crate) fullbright_path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) byte_len: usize,
    pub(crate) section_count: usize,
    pub(crate) vertex_count: u32,
    pub(crate) index_count: u32,
    pub(crate) samples: Vec<TorchLightProbeSample>,
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
    let scene = build_torch_light_probe_scene(&mesh_assets.catalog)?;
    if scene.sections.is_empty() {
        bail!("torch light probe built no render sections");
    }

    let lit_path = options.directory.join("lit.png");
    let fullbright_path = options.directory.join("fullbright.png");
    let mut render_options = options.render_options;
    render_options.section_occlusion_culling = false;

    let lit = write_probe_frame(
        lit_path.clone(),
        options.width,
        options.height,
        &scene.sections,
        mesh_assets.atlas.as_upload(),
        render_options,
    )
    .context("failed to render lit torch light probe")?;

    let mut fullbright_options = render_options;
    fullbright_options.force_fullbright = true;
    let fullbright = write_probe_frame(
        fullbright_path.clone(),
        options.width,
        options.height,
        &scene.sections,
        mesh_assets.atlas.as_upload(),
        fullbright_options,
    )
    .context("failed to render fullbright torch light probe")?;
    if fullbright.width != lit.width || fullbright.height != lit.height {
        bail!(
            "torch light probe frame size mismatch: lit={}x{} fullbright={}x{}",
            lit.width,
            lit.height,
            fullbright.width,
            fullbright.height
        );
    }

    Ok(TorchLightProbeReport {
        lit_path,
        fullbright_path,
        width: lit.width,
        height: lit.height,
        byte_len: lit.byte_len,
        section_count: scene.sections.len(),
        vertex_count: lit.vertex_count,
        index_count: lit.index_count,
        samples: scene.samples,
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
    samples: Vec<TorchLightProbeSample>,
}

fn build_torch_light_probe_scene(
    catalog: &mclone_mesh::TexturedMeshCatalog,
) -> Result<TorchLightProbeScene> {
    let blocks = torch_light_probe_blocks();
    let light_sections = torch_light_probe_light_sections();
    let sections = build_textured_render_sections(
        &[TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks).with_light_sections(&light_sections)],
        catalog,
    )
    .context("failed to build torch light probe render sections")?;

    Ok(TorchLightProbeScene {
        sections,
        samples: torch_light_probe_samples(),
    })
}

fn torch_light_probe_blocks() -> Vec<BlockStateId> {
    let mut blocks = vec![AIR_BLOCK_STATE_ID; 16 * 16 * 16];
    for y in ROOM_MIN_Y..=ROOM_MAX_Y {
        for z in ROOM_MIN_Z..=ROOM_MAX_Z {
            for x in ROOM_MIN_X..=ROOM_MAX_X {
                if is_room_shell(x, y, z) {
                    blocks[chunk_block_index(x, y, z)] = STONE;
                }
            }
        }
    }
    blocks[chunk_block_index(TORCH_POS[0], TORCH_POS[1], TORCH_POS[2])] = TORCH;
    blocks
}

fn torch_light_probe_light_sections() -> Vec<PackedLightSection> {
    let mut block = vec![0; LIGHT_DATA_LAYER_BYTE_COUNT];
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let light = torch_light_probe_block_light_at(x, y, z);
                if light > 0 {
                    set_data_layer_value(&mut block, x, y, z, light);
                }
            }
        }
    }
    vec![PackedLightSection::new(0, None, Some(block))]
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
        ("outside_shell", [4, 1, 8]),
    ]
    .into_iter()
    .map(|(label, position)| TorchLightProbeSample {
        label,
        position,
        block_light: torch_light_probe_block_light_at(position[0], position[1], position[2]),
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
            [14, 13, 13, 12, 0]
        );
    }

    #[test]
    fn torch_light_probe_room_places_torch_inside_stone_shell() {
        let blocks = torch_light_probe_blocks();

        assert_eq!(blocks[chunk_block_index(8, 1, 8)], TORCH);
        assert_eq!(blocks[chunk_block_index(8, 0, 8)], STONE);
        assert_eq!(blocks[chunk_block_index(8, 2, 8)], AIR_BLOCK_STATE_ID);
        assert_eq!(blocks[chunk_block_index(4, 1, 8)], AIR_BLOCK_STATE_ID);
    }
}
