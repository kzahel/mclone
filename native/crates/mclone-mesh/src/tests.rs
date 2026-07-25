use super::*;

#[test]
fn packed_textured_sections_round_trip_every_render_fact() {
    let sections = vec![
        TexturedRenderSectionMesh {
            key: RenderSectionKey::new(-7, 3, 11),
            mesh: TexturedVisibleChunkMesh {
                vertices: vec![TexturedChunkVertex {
                    position: [-1.25, 48.0, 9.5],
                    uv: [0.125, 0.875],
                    color: [0.1, 0.2, 0.3, 0.4],
                    packed_light: 0xfedc_ba98,
                }],
                indices: vec![0, 0, 0, 0, 0, 0],
                solid_index_count: 0,
                opaque_index_count: 6,
            },
            grass_patches: vec![GrassPatch {
                root: [-112, 49, 176],
                packed_tint: 0x1020_3040,
                packed_light: 0x5060_7080,
                seed: 0x90a0_b0c0,
                flags: 7,
                reserved: 13,
            }],
            visibility: VisibilitySet::from_bits(0x0000_000a_bcde_f012),
        },
        TexturedRenderSectionMesh {
            key: RenderSectionKey::new(-7, 4, 11),
            mesh: TexturedVisibleChunkMesh::default(),
            grass_patches: Vec::new(),
            visibility: VisibilitySet::all_visible(),
        },
    ];

    let packed = pack_textured_render_sections(&sections);
    let unpacked = unpack_textured_render_sections(&packed).unwrap();

    assert_eq!(unpacked, sections);
}

#[test]
fn packed_textured_sections_reject_truncated_and_trailing_payloads() {
    let packed = pack_textured_render_sections(&[TexturedRenderSectionMesh {
        key: RenderSectionKey::new(1, 2, 3),
        mesh: TexturedVisibleChunkMesh::default(),
        grass_patches: Vec::new(),
        visibility: VisibilitySet::all_visible(),
    }]);
    assert!(unpack_textured_render_sections(&packed[..packed.len() - 1]).is_err());

    let mut trailing = packed;
    trailing.push(0);
    assert!(unpack_textured_render_sections(&trailing).is_err());
}
use crate::builder::block_at_world_or_air;
use mclone_assets::{
    AssetPath, BlockModelLibrary, BlockStateAssetIndex, BlockStateRecord, BlockStateRegistry,
    MemoryAssetSource, ModelFaceDirection, ResourceLocation, TextureAtlasPlan, TextureMaterial,
};
use mclone_core::{AIR_BLOCK_STATE_ID, HorizontalTopology, PackedLightSection, chunk_block_index};
use mclone_light::{DataLayer, FULL_BRIGHT, pack_light};
use std::collections::BTreeSet;

fn chunk_blocks(height: i32, filled: &[(i32, i32, i32, u8)]) -> Vec<u8> {
    let mut blocks = vec![AIR_BLOCK_ID; height as usize * 16 * 16];
    for &(x, y, z, block_id) in filled {
        blocks[chunk_block_index(x, y, z)] = block_id;
    }
    blocks
}

fn assert_visibility(
    visibility: VisibilitySet,
    first: SectionFace,
    second: SectionFace,
    expected: bool,
) {
    assert_eq!(
        visibility.visibility_between(first, second),
        expected,
        "visibility between {first:?} and {second:?}"
    );
    assert_eq!(
        visibility.visibility_between(second, first),
        expected,
        "visibility between {second:?} and {first:?}"
    );
}

#[test]
fn textured_section_mesh_estimated_owned_bytes_tracks_vector_capacity() {
    let mut vertices = Vec::with_capacity(3);
    vertices.push(TexturedChunkVertex {
        position: [0.0, 0.0, 0.0],
        uv: [0.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        packed_light: 0,
    });
    let mut indices = Vec::with_capacity(7);
    indices.extend([0, 1, 2]);
    let mesh = TexturedVisibleChunkMesh {
        vertices,
        indices,
        solid_index_count: 3,
        opaque_index_count: 3,
    };
    let mesh_bytes =
        3 * std::mem::size_of::<TexturedChunkVertex>() + 7 * std::mem::size_of::<u32>();
    assert_eq!(mesh.estimated_owned_bytes(), mesh_bytes);

    let mut grass_patches = Vec::with_capacity(5);
    grass_patches.push(GrassPatch::default());
    let expected_bytes = mesh_bytes + 5 * std::mem::size_of::<GrassPatch>();

    let section = TexturedRenderSectionMesh {
        key: RenderSectionKey::new(0, 4, 0),
        mesh,
        grass_patches,
        visibility: VisibilitySet::all_visible(),
    };
    assert_eq!(section.estimated_owned_bytes(), expected_bytes);
}

#[test]
fn grass_patch_layout_is_compact_and_stable() {
    assert_eq!(std::mem::size_of::<GrassPatch>(), GrassPatch::BYTE_SIZE);
    assert_eq!(GrassPatch::BYTE_SIZE, 32);
    assert_eq!(std::mem::align_of::<GrassPatch>(), 4);
}

#[test]
fn grass_patch_discovery_is_request_gated_and_surface_correct() {
    let catalog = grass_textured_catalog();
    let blocks = textured_chunk_blocks(16, &[(3, 4, 5, BlockStateId(1))]);
    let input = TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks);
    let targets = BTreeSet::from([RenderSectionKey::new(0, 0, 0)]);

    let off = build_textured_render_sections_for_section_set_with_stats_and_options(
        &[input],
        &catalog,
        &targets,
        TexturedRenderSectionBuildOptions::OFF,
    )
    .unwrap();
    assert!(off.sections[0].grass_patches.is_empty());

    let enabled = build_textured_render_sections_for_section_set_with_stats_and_options(
        &[input],
        &catalog,
        &targets,
        TexturedRenderSectionBuildOptions::OFF.with_grass_patches(true),
    )
    .unwrap();
    let patch = enabled.sections[0].grass_patches.as_slice();
    assert_eq!(patch.len(), 1);
    assert_eq!(patch[0].root, [3, 5, 5]);
    assert_ne!(patch[0].packed_tint, 0);
    assert_eq!(patch[0].packed_light, FULL_BRIGHT);
    assert_ne!(patch[0].seed, 0);
    assert_eq!(patch[0].flags, 0);
    assert_eq!(patch[0].reserved, 0);
}

#[test]
fn covered_grass_at_a_section_boundary_emits_no_patch() {
    let catalog = grass_textured_catalog();
    let blocks = textured_chunk_blocks(
        32,
        &[(4, 15, 6, BlockStateId(1)), (4, 16, 6, BlockStateId(1))],
    );
    let input = TexturedChunkMeshInput::new(0, 0, 0, 32, &blocks);
    let targets = BTreeSet::from([RenderSectionKey::new(0, 0, 0)]);
    let report = build_textured_render_sections_for_section_set_with_stats_and_options(
        &[input],
        &catalog,
        &targets,
        TexturedRenderSectionBuildOptions::OFF.with_grass_patches(true),
    )
    .unwrap();

    assert_eq!(report.sections.len(), 1);
    assert!(report.sections[0].grass_patches.is_empty());
}

#[test]
fn periodic_aliases_share_canonical_grass_seed() {
    let catalog = grass_textured_catalog();
    let blocks = textured_chunk_blocks(16, &[(1, 3, 7, BlockStateId(1))]);
    let inputs = [
        TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
        TexturedChunkMeshInput::new(32, 0, 0, 16, &blocks),
    ];
    let targets = BTreeSet::from([
        RenderSectionKey::new(0, 0, 0),
        RenderSectionKey::new(32, 0, 0),
    ]);
    let report = build_textured_render_sections_for_section_set_with_stats_and_options(
        &inputs,
        &catalog,
        &targets,
        TexturedRenderSectionBuildOptions::OFF
            .with_grass_patches(true)
            .with_topology(HorizontalTopology::cylinder_x(0, 32)),
    )
    .unwrap();

    assert_eq!(report.sections.len(), 2);
    let first = report.sections[0].grass_patches[0];
    let alias = report.sections[1].grass_patches[0];
    assert_eq!(first.seed, alias.seed);
    assert_eq!(first.packed_tint, alias.packed_tint);
    assert_eq!(first.root, [1, 4, 7]);
    assert_eq!(alias.root, [513, 4, 7]);
}

fn textured_chunk_blocks(
    height: i32,
    filled: &[(i32, i32, i32, BlockStateId)],
) -> Vec<BlockStateId> {
    let mut blocks = vec![AIR_BLOCK_STATE_ID; height as usize * 16 * 16];
    for &(x, y, z, block_id) in filled {
        blocks[chunk_block_index(x, y, z)] = block_id;
    }
    blocks
}

fn stone_textured_catalog() -> TexturedMeshCatalog {
    stone_textured_catalog_with_ambient_occlusion(true)
}

fn stone_textured_catalog_with_ambient_occlusion(ambient_occlusion: bool) -> TexturedMeshCatalog {
    cube_textured_catalog("stone", ambient_occlusion)
}

fn grass_textured_catalog() -> TexturedMeshCatalog {
    cube_textured_catalog("grass_block", true)
}

fn cube_textured_catalog(block_path: &str, ambient_occlusion: bool) -> TexturedMeshCatalog {
    let mut registry = BlockStateRegistry::new();
    registry
        .register(BlockStateRecord::new(
            BlockStateId(1),
            ResourceLocation::new("minecraft", block_path).unwrap(),
            [] as [(&str, &str); 0],
        ))
        .unwrap();
    let mut source = MemoryAssetSource::new();
    source.insert_text(
        AssetPath::new(format!("assets/minecraft/blockstates/{block_path}.json")),
        format!(r#"{{"variants":{{"":{{"model":"minecraft:block/{block_path}"}}}}}}"#),
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/block.json"),
        "{}",
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube.json"),
        r##"{
          "parent":"minecraft:block/block",
          "elements":[{
            "from":[0,0,0],
            "to":[16,16,16],
            "faces":{
              "down":{"texture":"#down","cullface":"down"},
              "up":{"texture":"#up","cullface":"up"},
              "north":{"texture":"#north","cullface":"north"},
              "south":{"texture":"#south","cullface":"south"},
              "west":{"texture":"#west","cullface":"west"},
              "east":{"texture":"#east","cullface":"east"}
            }
          }]
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube_all.json"),
        r##"{
          "parent":"minecraft:block/cube",
          "textures":{
            "particle":"#all",
            "down":"#all",
            "up":"#all",
            "north":"#all",
            "east":"#all",
            "south":"#all",
            "west":"#all"
          }
        }"##,
    );
    source.insert_text(
        AssetPath::new(format!(
            "assets/minecraft/models/block/{block_path}.json"
        )),
        if ambient_occlusion {
            format!(
                r##"{{"parent":"minecraft:block/cube_all","textures":{{"all":"minecraft:block/{block_path}"}}}}"##
            )
        } else {
            format!(
                r##"{{"ambientocclusion":false,"parent":"minecraft:block/cube_all","textures":{{"all":"minecraft:block/{block_path}"}}}}"##
            )
        },
    );
    source.insert(
        AssetPath::new(format!("assets/minecraft/textures/block/{block_path}.png")),
        png_header(16, 16),
    );

    let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
    let library = BlockModelLibrary::load_for_blockstates(&source, &index).unwrap();
    let materials = library
        .collect_materials_for_models(
            index
                .assets()
                .flat_map(|asset| asset.model_refs.iter().cloned()),
        )
        .unwrap();
    let atlas = TextureAtlasPlan::build(&source, materials).unwrap();
    TexturedMeshCatalog::from_assets(&registry, &index, &library, &atlas).unwrap()
}

fn stone_and_leaves_textured_catalog() -> TexturedMeshCatalog {
    let mut registry = BlockStateRegistry::new();
    registry
        .register(BlockStateRecord::new(
            BlockStateId(1),
            ResourceLocation::parse("minecraft:stone").unwrap(),
            [] as [(&str, &str); 0],
        ))
        .unwrap();
    registry
        .register(BlockStateRecord::new(
            BlockStateId(2),
            ResourceLocation::parse("minecraft:oak_leaves").unwrap(),
            [] as [(&str, &str); 0],
        ))
        .unwrap();
    let mut source = MemoryAssetSource::new();
    source.insert_text(
        AssetPath::new("assets/minecraft/blockstates/stone.json"),
        r#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/blockstates/oak_leaves.json"),
        r#"{"variants":{"":{"model":"minecraft:block/oak_leaves"}}}"#,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/block.json"),
        "{}",
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube.json"),
        r##"{
          "parent":"minecraft:block/block",
          "elements":[{
            "from":[0,0,0],
            "to":[16,16,16],
            "faces":{
              "down":{"texture":"#down","cullface":"down"},
              "up":{"texture":"#up","cullface":"up"},
              "north":{"texture":"#north","cullface":"north"},
              "south":{"texture":"#south","cullface":"south"},
              "west":{"texture":"#west","cullface":"west"},
              "east":{"texture":"#east","cullface":"east"}
            }
          }]
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube_all.json"),
        r##"{
          "parent":"minecraft:block/cube",
          "textures":{
            "particle":"#all",
            "down":"#all",
            "up":"#all",
            "north":"#all",
            "east":"#all",
            "south":"#all",
            "west":"#all"
          }
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/stone.json"),
        r##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/oak_leaves.json"),
        r##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/oak_leaves"}}"##,
    );
    source.insert(
        AssetPath::new("assets/minecraft/textures/block/stone.png"),
        png_header(16, 16),
    );
    source.insert(
        AssetPath::new("assets/minecraft/textures/block/oak_leaves.png"),
        png_header(16, 16),
    );
    source.insert(
        AssetPath::new("assets/mclone/textures/derived/bushy_leaf/minecraft/block/oak_leaves.png"),
        png_header(32, 32),
    );

    let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
    let library = BlockModelLibrary::load_for_blockstates(&source, &index).unwrap();
    let mut materials = library
        .collect_materials_for_models(
            index
                .assets()
                .flat_map(|asset| asset.model_refs.iter().cloned()),
        )
        .unwrap();
    materials.insert(crate::catalog::bushy_leaf_material(
        &TextureMaterial::blocks(ResourceLocation::parse("minecraft:block/oak_leaves").unwrap()),
    ));
    let atlas = TextureAtlasPlan::build(&source, materials).unwrap();
    TexturedMeshCatalog::from_assets(&registry, &index, &library, &atlas).unwrap()
}

fn partial_and_stone_textured_catalog() -> TexturedMeshCatalog {
    let mut registry = BlockStateRegistry::new();
    registry
        .register(BlockStateRecord::new(
            BlockStateId(1),
            ResourceLocation::parse("minecraft:test_partial").unwrap(),
            [] as [(&str, &str); 0],
        ))
        .unwrap();
    registry
        .register(BlockStateRecord::new(
            BlockStateId(2),
            ResourceLocation::parse("minecraft:stone").unwrap(),
            [] as [(&str, &str); 0],
        ))
        .unwrap();
    let mut source = MemoryAssetSource::new();
    source.insert_text(
        AssetPath::new("assets/minecraft/blockstates/test_partial.json"),
        r#"{"variants":{"":{"model":"minecraft:block/test_partial"}}}"#,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/blockstates/stone.json"),
        r#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/block.json"),
        "{}",
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube.json"),
        r##"{
          "parent":"minecraft:block/block",
          "elements":[{
            "from":[0,0,0],
            "to":[16,16,16],
            "faces":{
              "down":{"texture":"#down","cullface":"down"},
              "up":{"texture":"#up","cullface":"up"},
              "north":{"texture":"#north","cullface":"north"},
              "south":{"texture":"#south","cullface":"south"},
              "west":{"texture":"#west","cullface":"west"},
              "east":{"texture":"#east","cullface":"east"}
            }
          }]
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube_all.json"),
        r##"{
          "parent":"minecraft:block/cube",
          "textures":{
            "particle":"#all",
            "down":"#all",
            "up":"#all",
            "north":"#all",
            "east":"#all",
            "south":"#all",
            "west":"#all"
          }
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/stone.json"),
        r##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/test_partial.json"),
        r##"{
          "textures":{"all":"minecraft:block/stone"},
          "elements":[{
            "from":[4,0,4],
            "to":[12,16,12],
            "faces":{"up":{"texture":"#all","cullface":"up"}}
          }]
        }"##,
    );
    source.insert(
        AssetPath::new("assets/minecraft/textures/block/stone.png"),
        png_header(16, 16),
    );

    let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
    let library = BlockModelLibrary::load_for_blockstates(&source, &index).unwrap();
    let materials = library
        .collect_materials_for_models(
            index
                .assets()
                .flat_map(|asset| asset.model_refs.iter().cloned()),
        )
        .unwrap();
    let atlas = TextureAtlasPlan::build(&source, materials).unwrap();
    TexturedMeshCatalog::from_assets(&registry, &index, &library, &atlas).unwrap()
}

fn log_axis_textured_catalog() -> TexturedMeshCatalog {
    let mut registry = BlockStateRegistry::new();
    for (id, axis) in [
        (BlockStateId(1), "y"),
        (BlockStateId(2), "z"),
        (BlockStateId(3), "x"),
    ] {
        registry
            .register(BlockStateRecord::new(
                id,
                ResourceLocation::parse("minecraft:oak_log").unwrap(),
                [("axis", axis)],
            ))
            .unwrap();
    }

    let mut source = MemoryAssetSource::new();
    source.insert_text(
        AssetPath::new("assets/minecraft/blockstates/oak_log.json"),
        r#"{
          "variants":{
            "axis=x":{"model":"minecraft:block/oak_log_horizontal","x":90,"y":90},
            "axis=y":{"model":"minecraft:block/oak_log"},
            "axis=z":{"model":"minecraft:block/oak_log_horizontal","x":90}
          }
        }"#,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/block.json"),
        "{}",
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube_column.json"),
        r##"{
          "parent":"minecraft:block/cube",
          "textures":{
            "particle":"#side",
            "down":"#end",
            "up":"#end",
            "north":"#side",
            "east":"#side",
            "south":"#side",
            "west":"#side"
          }
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube_column_horizontal.json"),
        r##"{
          "parent":"minecraft:block/block",
          "elements":[{
            "from":[0,0,0],
            "to":[16,16,16],
            "faces":{
              "down":{"texture":"#down","cullface":"down"},
              "up":{"texture":"#up","rotation":180,"cullface":"up"},
              "north":{"texture":"#north","cullface":"north"},
              "south":{"texture":"#south","cullface":"south"},
              "west":{"texture":"#west","cullface":"west"},
              "east":{"texture":"#east","cullface":"east"}
            }
          }],
          "textures":{
            "particle":"#side",
            "down":"#end",
            "up":"#end",
            "north":"#side",
            "east":"#side",
            "south":"#side",
            "west":"#side"
          }
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube.json"),
        r##"{
          "parent":"minecraft:block/block",
          "elements":[{
            "from":[0,0,0],
            "to":[16,16,16],
            "faces":{
              "down":{"texture":"#down","cullface":"down"},
              "up":{"texture":"#up","cullface":"up"},
              "north":{"texture":"#north","cullface":"north"},
              "south":{"texture":"#south","cullface":"south"},
              "west":{"texture":"#west","cullface":"west"},
              "east":{"texture":"#east","cullface":"east"}
            }
          }]
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/oak_log.json"),
        r##"{
          "parent":"minecraft:block/cube_column",
          "textures":{
            "end":"minecraft:block/oak_log_top",
            "side":"minecraft:block/oak_log"
          }
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/oak_log_horizontal.json"),
        r##"{
          "parent":"minecraft:block/cube_column_horizontal",
          "textures":{
            "end":"minecraft:block/oak_log_top",
            "side":"minecraft:block/oak_log"
          }
        }"##,
    );
    source.insert(
        AssetPath::new("assets/minecraft/textures/block/oak_log.png"),
        png_header(16, 16),
    );
    source.insert(
        AssetPath::new("assets/minecraft/textures/block/oak_log_top.png"),
        png_header(16, 16),
    );

    let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
    let library = BlockModelLibrary::load_for_blockstates(&source, &index).unwrap();
    let materials = library
        .collect_materials_for_models(
            index
                .assets()
                .flat_map(|asset| asset.model_refs.iter().cloned()),
        )
        .unwrap();
    let atlas = TextureAtlasPlan::build(&source, materials).unwrap();
    TexturedMeshCatalog::from_assets(&registry, &index, &library, &atlas).unwrap()
}

fn model_face_sprite(model: &TexturedBlockModel, direction: ModelFaceDirection) -> AtlasSpriteUv {
    model
        .faces
        .iter()
        .find(|face| face.direction == direction)
        .unwrap()
        .sprite
}

fn liquid_textured_catalog() -> TexturedMeshCatalog {
    let mut registry = BlockStateRegistry::new();
    registry
        .register(BlockStateRecord::new(
            BlockStateId(1),
            ResourceLocation::parse("minecraft:stone").unwrap(),
            [] as [(&str, &str); 0],
        ))
        .unwrap();
    for (id, block, properties) in [
        (BlockStateId(2), "minecraft:water", [("level", "0")]),
        (BlockStateId(9), "minecraft:lava", [("level", "0")]),
        (BlockStateId(72), "minecraft:water", [("level", "1")]),
    ] {
        registry
            .register(BlockStateRecord::new(
                id,
                ResourceLocation::parse(block).unwrap(),
                properties,
            ))
            .unwrap();
    }

    let mut source = MemoryAssetSource::new();
    source.insert_text(
        AssetPath::new("assets/minecraft/blockstates/water.json"),
        r#"{"variants":{"":{"model":"minecraft:block/water"}}}"#,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/blockstates/stone.json"),
        r#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/blockstates/lava.json"),
        r#"{"variants":{"":{"model":"minecraft:block/lava"}}}"#,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/block.json"),
        "{}",
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube.json"),
        r##"{
          "parent":"minecraft:block/block",
          "elements":[{
            "from":[0,0,0],
            "to":[16,16,16],
            "faces":{
              "down":{"texture":"#down","cullface":"down"},
              "up":{"texture":"#up","cullface":"up"},
              "north":{"texture":"#north","cullface":"north"},
              "south":{"texture":"#south","cullface":"south"},
              "west":{"texture":"#west","cullface":"west"},
              "east":{"texture":"#east","cullface":"east"}
            }
          }]
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/cube_all.json"),
        r##"{
          "parent":"minecraft:block/cube",
          "textures":{
            "particle":"#all",
            "down":"#all",
            "up":"#all",
            "north":"#all",
            "east":"#all",
            "south":"#all",
            "west":"#all"
          }
        }"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/stone.json"),
        r##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/water.json"),
        r##"{"parent":"minecraft:block/block","textures":{"particle":"minecraft:block/water_still"}}"##,
    );
    source.insert_text(
        AssetPath::new("assets/minecraft/models/block/lava.json"),
        r##"{"parent":"minecraft:block/block","textures":{"particle":"minecraft:block/lava_still"}}"##,
    );
    for texture in ["water_still", "water_flow", "lava_still", "lava_flow"] {
        source.insert(
            AssetPath::new(format!("assets/minecraft/textures/block/{texture}.png")),
            png_header(16, 16),
        );
    }
    source.insert(
        AssetPath::new("assets/minecraft/textures/block/stone.png"),
        png_header(16, 16),
    );

    let index = BlockStateAssetIndex::load_namespace(&source, "minecraft").unwrap();
    let library = BlockModelLibrary::load_for_blockstates(&source, &index).unwrap();
    let mut materials = library
        .collect_materials_for_models(
            index
                .assets()
                .flat_map(|asset| asset.model_refs.iter().cloned()),
        )
        .unwrap();
    for texture in [
        "minecraft:block/water_still",
        "minecraft:block/water_flow",
        "minecraft:block/lava_still",
        "minecraft:block/lava_flow",
    ] {
        materials.insert(TextureMaterial::blocks(
            ResourceLocation::parse(texture).unwrap(),
        ));
    }
    let atlas = TextureAtlasPlan::build(&source, materials).unwrap();
    TexturedMeshCatalog::from_assets(&registry, &index, &library, &atlas).unwrap()
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes.extend_from_slice(&13_u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

mod debug_mesh;
mod textured_catalog;
mod textured_mesh;
mod visibility;
