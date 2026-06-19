#![forbid(unsafe_code)]

mod ambient_occlusion;
mod builder;
mod catalog;
mod data;
mod render_facts;
mod visibility;

use mclone_core::BlockStateId;

pub use builder::{
    ChunkMeshInput, TexturedChunkMeshInput, build_textured_render_sections,
    build_textured_render_sections_for_chunk_set,
    build_textured_render_sections_for_chunk_set_with_stats,
    build_textured_render_sections_for_section_set_with_stats,
    build_textured_render_sections_with_stats, build_textured_visible_chunk_area_mesh,
    build_textured_visible_chunk_mesh, build_visible_chunk_area_mesh, build_visible_chunk_mesh,
};
pub use catalog::{
    AtlasSpriteUv, TexturedBlockFace, TexturedBlockModel, TexturedMeshCatalog, TexturedMeshError,
};
pub use data::{
    ChunkVertex, RenderSectionKey, SectionMeshStats, TexturedChunkVertex,
    TexturedRenderSectionBuildReport, TexturedRenderSectionMesh, TexturedVisibleChunkMesh,
    VisibilityGraphBuildStats, VisibleChunkMesh, quad_face_count_from_indices,
};
pub use mclone_core::{CHUNK_WIDTH, SECTION_HEIGHT as RENDER_SECTION_HEIGHT};
pub use visibility::{SectionFace, VisGraph, VisibilitySet};

pub const QUAD_FACE_INDEX_COUNT: u32 = 6;
pub const AIR_BLOCK_ID: u8 = 0;
pub const CAVE_AIR_BLOCK_ID: u8 = 71;
pub const CAVE_AIR_BLOCK_STATE_ID: BlockStateId = BlockStateId(71);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{block_at_world_or_air, data_layer_value};
    use mclone_assets::{
        AssetPath, BlockModelLibrary, BlockStateAssetIndex, BlockStateRecord, BlockStateRegistry,
        MemoryAssetSource, ResourceLocation, TextureAtlasPlan,
    };
    use mclone_core::{
        AIR_BLOCK_STATE_ID, LIGHT_DATA_LAYER_BYTE_COUNT, PackedLightSection, chunk_block_index,
    };
    use mclone_light::{DataLayer, FULL_BRIGHT, pack_light};
    use std::collections::BTreeSet;

    fn chunk_blocks(height: i32, filled: &[(i32, i32, i32, u8)]) -> Vec<u8> {
        let mut blocks = vec![AIR_BLOCK_ID; height as usize * 16 * 16];
        for &(x, y, z, block_id) in filled {
            blocks[chunk_block_index(x, y, z)] = block_id;
        }
        blocks
    }

    #[test]
    fn empty_mesh_stats_are_zero() {
        assert_eq!(SectionMeshStats::default().vertex_count, 0);
        assert_eq!(SectionMeshStats::default().index_count, 0);
        assert_eq!(SectionMeshStats::default().face_count(), 0);
    }

    #[test]
    fn single_block_emits_six_faces() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(mesh.stats().vertex_count, 24);
        assert_eq!(mesh.stats().index_count, 36);
        assert_eq!(mesh.stats().face_count(), 6);
    }

    #[test]
    fn adjacent_blocks_cull_shared_face() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1), (1, 0, 0, 1)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(mesh.stats().vertex_count, 40);
        assert_eq!(mesh.stats().index_count, 60);
        assert_eq!(mesh.stats().face_count(), 10);
    }

    #[test]
    fn solid_section_culls_internal_faces_in_debug_mesh() {
        let blocks = vec![1; (16 * 16 * 16) as usize];
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(mesh.stats().face_count(), 6 * 16 * 16);
        assert_eq!(mesh.stats().vertex_count, 6 * 16 * 16 * 4);
        assert_eq!(mesh.stats().index_count, 6 * 16 * 16 * 6);
    }

    #[test]
    fn cave_air_is_invisible_and_non_occluding_in_debug_mesh() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1), (1, 0, 0, CAVE_AIR_BLOCK_ID)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(0, 0, 0, 16, &blocks));

        assert_eq!(mesh.stats().vertex_count, 24);
        assert_eq!(mesh.stats().index_count, 36);
    }

    #[test]
    fn mesh_positions_use_world_chunk_offset() {
        let blocks = chunk_blocks(16, &[(0, 0, 0, 1)]);
        let mesh = build_visible_chunk_mesh(ChunkMeshInput::new(2, -1, 4, 16, &blocks));

        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.position == [32.0, 4.0, -16.0])
        );
    }

    #[test]
    fn area_mesh_culls_faces_across_chunk_boundaries() {
        let left = chunk_blocks(16, &[(15, 0, 0, 1)]);
        let right = chunk_blocks(16, &[(0, 0, 0, 1)]);
        let mesh = build_visible_chunk_area_mesh(&[
            ChunkMeshInput::new(0, 0, 0, 16, &left),
            ChunkMeshInput::new(1, 0, 0, 16, &right),
        ]);

        assert_eq!(mesh.stats().vertex_count, 40);
        assert_eq!(mesh.stats().index_count, 60);
    }

    #[test]
    fn area_mesh_uses_euclidean_chunk_coordinates_for_negative_world_positions() {
        let chunk = chunk_blocks(16, &[(15, 0, 15, 1)]);
        assert_eq!(
            block_at_world_or_air(&[ChunkMeshInput::new(-1, -1, 0, 16, &chunk)], -1, 0, -1),
            1
        );
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
    fn vis_graph_index_packing_matches_minecraft() {
        assert_eq!(VisGraph::index(0, 0, 0), 0);
        assert_eq!(VisGraph::index(1, 0, 0), 1);
        assert_eq!(VisGraph::index(0, 0, 1), 16);
        assert_eq!(VisGraph::index(0, 1, 0), 256);
        assert_eq!(VisGraph::index(15, 15, 15), 4095);
    }

    #[test]
    fn vis_graph_empty_and_lightly_opaque_sections_are_all_visible() {
        let empty = VisGraph::new().resolve();
        for first in SectionFace::ALL {
            for second in SectionFace::ALL {
                assert_visibility(empty, first, second, true);
            }
        }

        let mut graph = VisGraph::new();
        for x in 0..15 {
            for z in 0..15 {
                graph.set_opaque_local(x, 0, z);
            }
        }
        assert_eq!(graph.opaque_count(), 225);
        let light = graph.resolve();
        for first in SectionFace::ALL {
            for second in SectionFace::ALL {
                assert_visibility(light, first, second, true);
            }
        }
    }

    #[test]
    fn vis_graph_solid_section_has_no_face_visibility() {
        let mut graph = VisGraph::new();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    graph.set_opaque_local(x, y, z);
                }
            }
        }

        let visibility = graph.resolve();
        for first in SectionFace::ALL {
            for second in SectionFace::ALL {
                assert_visibility(visibility, first, second, false);
            }
        }
    }

    #[test]
    fn vis_graph_solid_wall_splits_west_from_east() {
        let mut graph = VisGraph::new();
        for y in 0..16 {
            for z in 0..16 {
                graph.set_opaque_local(8, y, z);
            }
        }

        let visibility = graph.resolve();
        assert_visibility(visibility, SectionFace::West, SectionFace::East, false);
        assert_visibility(visibility, SectionFace::West, SectionFace::Down, true);
        assert_visibility(visibility, SectionFace::East, SectionFace::Down, true);
        assert_visibility(visibility, SectionFace::North, SectionFace::South, true);
        assert_visibility(visibility, SectionFace::Down, SectionFace::Up, true);
    }

    #[test]
    fn vis_graph_tunnel_through_solid_section_only_connects_tunnel_faces() {
        let mut graph = VisGraph::new();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    if y != 8 || z != 8 {
                        graph.set_opaque_local(x, y, z);
                    }
                }
            }
        }

        let visibility = graph.resolve();
        assert_visibility(visibility, SectionFace::West, SectionFace::East, true);
        assert_visibility(visibility, SectionFace::West, SectionFace::Down, false);
        assert_visibility(visibility, SectionFace::East, SectionFace::Up, false);
        assert_visibility(visibility, SectionFace::North, SectionFace::South, false);
    }

    #[test]
    fn vis_graph_enclosed_cavity_has_no_outer_face_visibility() {
        let mut graph = VisGraph::new();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    if x != 8 || y != 8 || z != 8 {
                        graph.set_opaque_local(x, y, z);
                    }
                }
            }
        }

        let visibility = graph.resolve();
        for first in SectionFace::ALL {
            for second in SectionFace::ALL {
                assert_visibility(visibility, first, second, false);
            }
        }
    }

    #[test]
    fn vis_graph_matches_java_oracle_synthetic_cases() {
        let fixture = include_str!("../../../../test/fixtures/render/visgraph-synthetic.json");
        let json: serde_json::Value = serde_json::from_str(fixture).unwrap();
        assert_eq!(json["module"], "visgraph");
        assert_eq!(
            json["faceOrder"],
            serde_json::json!(["down", "up", "north", "south", "west", "east"])
        );

        let cases = json["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 6);
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let mut graph = VisGraph::new();
            for opaque in case["opaque"].as_array().unwrap() {
                let cell = opaque.as_array().unwrap();
                graph.set_opaque_local(
                    cell[0].as_u64().unwrap() as usize,
                    cell[1].as_u64().unwrap() as usize,
                    cell[2].as_u64().unwrap() as usize,
                );
            }
            assert_eq!(
                graph.opaque_count(),
                case["opaqueCount"].as_u64().unwrap() as usize,
                "opaque count for {name}"
            );

            let visibility = graph.resolve();
            let rows = case["visibility"].as_array().unwrap();
            for first in SectionFace::ALL {
                let columns = rows[first.index()].as_array().unwrap();
                for second in SectionFace::ALL {
                    assert_eq!(
                        visibility.visibility_between(first, second),
                        columns[second.index()].as_bool().unwrap(),
                        "visibility mismatch for case {name}, {first:?} -> {second:?}"
                    );
                }
            }
        }
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

    fn stone_textured_catalog_with_ambient_occlusion(
        ambient_occlusion: bool,
    ) -> TexturedMeshCatalog {
        let mut registry = BlockStateRegistry::new();
        registry
            .register(BlockStateRecord::new(
                BlockStateId(1),
                ResourceLocation::parse("minecraft:stone").unwrap(),
                [] as [(&str, &str); 0],
            ))
            .unwrap();
        let mut source = MemoryAssetSource::new();
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
            if ambient_occlusion {
                r##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##
                    .to_owned()
            } else {
                r##"{"ambientocclusion":false,"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##
                    .to_owned()
            },
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
            r##"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"##,
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

    #[test]
    fn textured_single_block_uses_baked_model_faces_and_atlas_uvs() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(16, &[(0, 0, 0, BlockStateId(1))]);
        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();

        assert_eq!(mesh.stats().vertex_count, 24);
        assert_eq!(mesh.stats().index_count, 36);
        assert!(catalog.occludes(BlockStateId(1)));
        assert!(!catalog.occludes(AIR_BLOCK_STATE_ID));
        assert!(mesh.vertices.iter().all(
            |vertex| (0.0..=1.0).contains(&vertex.uv[0]) && (0.0..=1.0).contains(&vertex.uv[1])
        ));
    }

    #[test]
    fn catalog_applies_java_render_facts_to_full_cube_leaves() {
        let catalog = stone_and_leaves_textured_catalog();
        let stone = catalog.get(BlockStateId(1)).unwrap();
        let leaves = catalog.get(BlockStateId(2)).unwrap();

        assert!(stone.occludes);
        assert_eq!(stone.light_block, 15);
        assert!(stone.view_blocking);
        assert!(stone.solid_render);

        assert!(!leaves.occludes);
        assert_eq!(leaves.light_block, 1);
        assert!(!leaves.view_blocking);
        assert!(!leaves.solid_render);
        assert!(leaves.collision_shape_full_block);
    }

    #[test]
    fn textured_mesh_samples_packed_light_from_face_neighbor() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(16, &[(8, 0, 8, BlockStateId(1))]);
        let mut sky = DataLayer::new();
        sky.set(8, 1, 8, 15);
        let light_sections = [PackedLightSection::new(0, sky.into_bytes(), None)];
        let input =
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks).with_light_sections(&light_sections);

        let mesh = build_textured_visible_chunk_mesh(input, &catalog).unwrap();

        assert_eq!(mesh.stats().face_count(), 6);
        let top_face_light = pack_light(0, 15);
        assert!(mesh.vertices.chunks_exact(4).any(|face| {
            face.iter()
                .all(|vertex| vertex.packed_light == top_face_light)
        }));
        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.packed_light != FULL_BRIGHT)
        );
    }

    #[test]
    fn textured_mesh_treats_missing_sky_section_above_data_as_open_sky() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(32, &[(8, 15, 8, BlockStateId(1))]);
        let mut sky = DataLayer::new();
        sky.get_data();
        let light_sections = [PackedLightSection::new(0, sky.into_bytes(), None)];
        let input =
            TexturedChunkMeshInput::new(0, 0, 0, 32, &blocks).with_light_sections(&light_sections);

        let mesh = build_textured_visible_chunk_mesh(input, &catalog).unwrap();

        let top_face_light = pack_light(0, 15);
        assert!(mesh.vertices.chunks_exact(4).any(|face| {
            face.iter()
                .all(|vertex| vertex.packed_light == top_face_light)
        }));
    }

    #[test]
    fn textured_mesh_climbs_to_next_sky_layer_for_missing_sections() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(48, &[(8, 15, 8, BlockStateId(1))]);
        let mut sky = DataLayer::new();
        sky.set(8, 0, 8, 4);
        let light_sections = [PackedLightSection::new(2, sky.into_bytes(), None)];
        let input =
            TexturedChunkMeshInput::new(0, 0, 0, 48, &blocks).with_light_sections(&light_sections);

        let mesh = build_textured_visible_chunk_mesh(input, &catalog).unwrap();

        let top_face_light = pack_light(0, 4);
        assert!(mesh.vertices.chunks_exact(4).any(|face| {
            face.iter()
                .all(|vertex| vertex.packed_light == top_face_light)
        }));
    }

    #[test]
    fn textured_mesh_uses_fullbright_without_light_payload() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(16, &[(0, 0, 0, BlockStateId(1))]);
        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();

        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.packed_light == FULL_BRIGHT)
        );
    }

    #[test]
    fn textured_mesh_applies_ambient_occlusion_on_full_cube_faces() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            16,
            &[(0, 0, 0, BlockStateId(1)), (1, 1, 0, BlockStateId(1))],
        );

        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();
        let top_face = mesh
            .vertices
            .chunks_exact(4)
            .find(|face| {
                face.iter().all(|vertex| {
                    (vertex.position[1] - 1.0).abs() < 0.0001
                        && (0.0..=1.0).contains(&vertex.position[0])
                        && (0.0..=1.0).contains(&vertex.position[2])
                })
            })
            .expect("top face for origin block should be emitted");
        let west_brightness = top_face
            .iter()
            .filter(|vertex| (vertex.position[0] - 0.0).abs() < 0.0001)
            .map(|vertex| vertex.color[0])
            .sum::<f32>();
        let east_brightness = top_face
            .iter()
            .filter(|vertex| (vertex.position[0] - 1.0).abs() < 0.0001)
            .map(|vertex| vertex.color[0])
            .sum::<f32>();

        assert!(
            east_brightness < west_brightness,
            "east vertices beside the occluder should be darker"
        );
    }

    #[test]
    fn textured_mesh_respects_model_ambient_occlusion_flag() {
        let catalog = stone_textured_catalog_with_ambient_occlusion(false);
        let blocks = textured_chunk_blocks(
            16,
            &[(0, 0, 0, BlockStateId(1)), (1, 1, 0, BlockStateId(1))],
        );

        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();
        let top_face = mesh
            .vertices
            .chunks_exact(4)
            .find(|face| {
                face.iter().all(|vertex| {
                    (vertex.position[1] - 1.0).abs() < 0.0001
                        && (0.0..=1.0).contains(&vertex.position[0])
                        && (0.0..=1.0).contains(&vertex.position[2])
                })
            })
            .expect("top face for origin block should be emitted");

        assert!(
            top_face
                .iter()
                .all(|vertex| (vertex.color[0] - top_face[0].color[0]).abs() < 0.0001),
            "ambientocclusion=false should keep flat face brightness"
        );
    }

    #[test]
    fn textured_mesh_applies_ambient_occlusion_on_partial_faces() {
        let catalog = partial_and_stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            16,
            &[(0, 0, 0, BlockStateId(1)), (1, 1, 0, BlockStateId(2))],
        );

        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();
        let partial_top_face = mesh
            .vertices
            .chunks_exact(4)
            .find(|face| {
                face.iter().all(|vertex| {
                    (vertex.position[1] - 1.0).abs() < 0.0001
                        && (0.25..=0.75).contains(&vertex.position[0])
                        && (0.25..=0.75).contains(&vertex.position[2])
                })
            })
            .expect("partial block top face should be emitted");
        let min_brightness = partial_top_face
            .iter()
            .map(|vertex| vertex.color[0])
            .fold(f32::INFINITY, f32::min);
        let max_brightness = partial_top_face
            .iter()
            .map(|vertex| vertex.color[0])
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(
            min_brightness < max_brightness,
            "partial faces should receive per-vertex AO instead of flat brightness"
        );
    }

    #[test]
    fn cave_air_is_invisible_and_non_occluding_in_textured_mesh() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            16,
            &[
                (0, 0, 0, BlockStateId(1)),
                (1, 0, 0, CAVE_AIR_BLOCK_STATE_ID),
            ],
        );
        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();

        assert_eq!(mesh.stats().vertex_count, 24);
        assert_eq!(mesh.stats().index_count, 36);
        assert!(!catalog.occludes(CAVE_AIR_BLOCK_STATE_ID));
    }

    #[test]
    fn textured_adjacent_blocks_cull_shared_face() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            16,
            &[(0, 0, 0, BlockStateId(1)), (1, 0, 0, BlockStateId(1))],
        );
        let mesh = build_textured_visible_chunk_mesh(
            TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks),
            &catalog,
        )
        .unwrap();

        assert_eq!(mesh.stats().vertex_count, 40);
        assert_eq!(mesh.stats().index_count, 60);
        assert_eq!(mesh.stats().face_count(), 10);
    }

    #[test]
    fn solid_textured_section_culls_internal_model_faces() {
        let catalog = stone_textured_catalog();
        let blocks = vec![BlockStateId(1); (16 * 16 * 16) as usize];
        let sections = build_textured_render_sections(
            &[TexturedChunkMeshInput::new(0, 0, 0, 16, &blocks)],
            &catalog,
        )
        .unwrap();

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].stats().face_count(), 6 * 16 * 16);
        assert_eq!(sections[0].stats().vertex_count, 6 * 16 * 16 * 4);
        assert_eq!(sections[0].stats().index_count, 6 * 16 * 16 * 6);
        assert_visibility(
            sections[0].visibility,
            SectionFace::North,
            SectionFace::South,
            false,
        );
    }

    #[test]
    fn adjacent_solid_textured_chunks_cull_shared_boundary_side() {
        let catalog = stone_textured_catalog();
        let left = vec![BlockStateId(1); (16 * 16 * 16) as usize];
        let right = vec![BlockStateId(1); (16 * 16 * 16) as usize];
        let sections = build_textured_render_sections(
            &[
                TexturedChunkMeshInput::new(0, 0, 0, 16, &left),
                TexturedChunkMeshInput::new(1, 0, 0, 16, &right),
            ],
            &catalog,
        )
        .unwrap();

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].stats().face_count(), 5 * 16 * 16);
        assert_eq!(sections[1].stats().face_count(), 5 * 16 * 16);
        assert_eq!(
            sections
                .iter()
                .map(|section| section.stats().face_count())
                .sum::<u32>(),
            2 * (32 * 16 + 32 * 16 + 16 * 16)
        );
    }

    #[test]
    fn selective_textured_section_build_uses_neighbors_for_culling() {
        let catalog = stone_textured_catalog();
        let left = vec![BlockStateId(1); (16 * 16 * 16) as usize];
        let right = vec![BlockStateId(1); (16 * 16 * 16) as usize];
        let sections = build_textured_render_sections_for_chunk_set(
            &[
                TexturedChunkMeshInput::new(0, 0, 0, 16, &left),
                TexturedChunkMeshInput::new(1, 0, 0, 16, &right),
            ],
            &catalog,
            &BTreeSet::from([(1, 0)]),
        )
        .unwrap();

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].key, RenderSectionKey::new(1, 0, 0));
        assert_eq!(sections[0].stats().face_count(), 5 * 16 * 16);
    }

    #[test]
    fn data_layer_value_uses_java_nibble_order() {
        let mut bytes = vec![0; LIGHT_DATA_LAYER_BYTE_COUNT];
        bytes[0] = 0xA3;

        assert_eq!(data_layer_value(&bytes, 0), 3);
        assert_eq!(data_layer_value(&bytes, 1), 10);
    }

    #[test]
    fn textured_render_sections_split_by_vertical_section() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            32,
            &[(0, 0, 0, BlockStateId(1)), (0, 16, 0, BlockStateId(1))],
        );
        let sections = build_textured_render_sections(
            &[TexturedChunkMeshInput::new(2, -3, -16, 32, &blocks)],
            &catalog,
        )
        .unwrap();

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].key, RenderSectionKey::new(2, -1, -3));
        assert_eq!(sections[1].key, RenderSectionKey::new(2, 0, -3));
        assert_eq!(sections[0].stats().index_count, 36);
        assert_eq!(sections[1].stats().index_count, 36);
    }

    #[test]
    fn textured_render_sections_cull_across_section_boundary() {
        let catalog = stone_textured_catalog();
        let blocks = textured_chunk_blocks(
            32,
            &[(0, 15, 0, BlockStateId(1)), (0, 16, 0, BlockStateId(1))],
        );
        let sections = build_textured_render_sections(
            &[TexturedChunkMeshInput::new(0, 0, 0, 32, &blocks)],
            &catalog,
        )
        .unwrap();
        let combined = build_textured_visible_chunk_area_mesh(
            &[TexturedChunkMeshInput::new(0, 0, 0, 32, &blocks)],
            &catalog,
        )
        .unwrap();

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].stats().index_count, 30);
        assert_eq!(sections[1].stats().index_count, 30);
        assert_eq!(combined.stats().index_count, 60);
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
}
