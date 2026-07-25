#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};
use mclone_assets::{
    AssetPackId, AssetPackOrigin, AssetPackSelection, AssetSourceChain, PackedAssetSource,
    ProvenanceTrackingAssetSource,
};
use mclone_core::{AIR_BLOCK_STATE_ID, BlockPos, BlockStateId, CHUNK_WIDTH, chunk_block_index};
use mclone_mesh::{
    TexturedChunkMeshInput, TexturedChunkVertex, TexturedVisibleChunkMesh,
    build_textured_visible_chunk_area_mesh, load_first_party_textured_terrain_assets,
};
use mclone_worldgen::block::AIR;
use mclone_worldgen::structure_json::{
    CanonicalSocketFacing, CanonicalStructureRecord, load_canonical_structure_json,
};
use mclone_worldgen::structure_template::{
    StructurePlaceSettings, TemplateBlockState, TemplateMirror, TemplateRotation,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const STRUCTURE_PREVIEW_SCHEMA_VERSION: u32 = 1;
pub const STRUCTURE_PREVIEW_COMPILER_ID: &str = "mclone-structure-preview-v1";
pub const AUTHORED_PACK_FILE: &str = "mclone-authored.pbp";
pub const GENERATED_PACK_FILE: &str = "mclone-generated-fallback.pbp";

#[derive(Debug)]
pub struct StructurePreviewCompiler {
    assets: mclone_mesh::TexturedTerrainAssets,
    atlas_png: Vec<u8>,
    atlas_sha256: String,
    packs: Vec<PackReceipt>,
    provenance: ProvenanceSummaryReceipt,
}

impl StructurePreviewCompiler {
    pub fn load(asset_pack_root: impl AsRef<Path>) -> Result<Self> {
        let asset_pack_root = asset_pack_root.as_ref();
        let authored_path = asset_pack_root.join(AUTHORED_PACK_FILE);
        let generated_path = asset_pack_root.join(GENERATED_PACK_FILE);
        let authored = load_pack(&authored_path, AssetPackOrigin::FirstParty)?;
        let generated = load_pack(&generated_path, AssetPackOrigin::FirstPartyProvisional)?;
        let authored_id = pack_id(&authored, &authored_path)?;
        let generated_id = pack_id(&generated, &generated_path)?;
        let packs = vec![
            pack_receipt(&authored_path, &authored, &authored_id)?,
            pack_receipt(&generated_path, &generated, &generated_id)?,
        ];

        let mut chain = AssetSourceChain::new();
        chain.push_named(authored_id.clone(), AssetPackOrigin::FirstParty, authored);
        chain.push_named(
            generated_id.clone(),
            AssetPackOrigin::FirstPartyProvisional,
            generated,
        );
        let tracker = ProvenanceTrackingAssetSource::new(&chain);
        let assets = load_first_party_textured_terrain_assets(&tracker)
            .context("failed to prepare first-party structure preview assets")?;
        let report = tracker.report(
            1,
            AssetPackSelection::new([authored_id.clone(), generated_id.clone()]),
        );
        if !report.allows_proprietary_free_claim() {
            bail!("structure preview resolved Minecraft-reference or unknown assets");
        }
        let summary = report.summary();
        let provenance = ProvenanceSummaryReceipt {
            first_party: summary.first_party,
            generated: summary.generated,
            minecraft_reference: summary.minecraft_reference,
            unknown: summary.unknown,
            missing: summary.missing,
        };
        let atlas_png = encode_png(assets.atlas.width, assets.atlas.height, assets.atlas.rgba())?;
        let atlas_sha256 = sha256_hex(&atlas_png);
        Ok(Self {
            assets,
            atlas_png,
            atlas_sha256,
            packs,
            provenance,
        })
    }

    pub fn compile_json_file(
        &self,
        structure_json: impl AsRef<Path>,
        out_root: impl AsRef<Path>,
    ) -> Result<PreviewReceipt> {
        let structure_json = structure_json.as_ref();
        let json = fs::read_to_string(structure_json)
            .with_context(|| format!("failed to read {}", structure_json.display()))?;
        let structure = load_canonical_structure_json(&json)
            .with_context(|| format!("failed to load {}", structure_json.display()))?;
        self.compile(&structure, out_root)
    }

    pub fn compile(
        &self,
        structure: &CanonicalStructureRecord,
        out_root: impl AsRef<Path>,
    ) -> Result<PreviewReceipt> {
        let out_root = out_root.as_ref();
        let theme = structure.default_theme().with_context(|| {
            format!(
                "structure `{}` has no resolvable default theme",
                structure.template.id()
            )
        })?;
        let placed = structure.template.place(&StructurePlaceSettings {
            origin: BlockPos::new(0, 0, 0),
            rotation: TemplateRotation::None,
            mirror: TemplateMirror::None,
            theme,
        })?;
        let mesh = mesh_placed_structure(structure.template.size(), &placed.blocks, &self.assets)?;
        if mesh.indices.is_empty() {
            bail!(
                "structure `{}` produced an empty preview",
                structure.template.id()
            );
        }
        let assignment = block_component_assignment(structure);
        let grouped = group_mesh_indices(&mesh, &assignment)?;
        let atlas_file = format!("terrain-{}.png", self.atlas_sha256);
        let atlas_relative = format!("atlases/{atlas_file}");
        let mesh_relative = format!("meshes/{}.glb", structure.template.id());
        let receipt_relative = format!("receipts/{}.preview.json", structure.template.id());
        let (glb, geometry) = build_glb(
            structure.template.id(),
            structure.template.size(),
            &mesh,
            &grouped,
            &format!("../atlases/{atlas_file}"),
        )?;
        let mesh_sha256 = sha256_hex(&glb);
        let receipt = PreviewReceipt::new(
            structure,
            geometry,
            mesh_relative.clone(),
            mesh_sha256,
            atlas_relative.clone(),
            self.atlas_sha256.clone(),
            self.assets.atlas.width,
            self.assets.atlas.height,
            self.assets.atlas_sprite_count,
            self.packs.clone(),
            self.provenance.clone(),
        );

        write_if_changed(&out_root.join(&mesh_relative), &glb)?;
        write_if_changed(&out_root.join(&atlas_relative), &self.atlas_png)?;
        let receipt_json = serde_json::to_vec_pretty(&receipt)?;
        let mut receipt_json_with_newline = receipt_json;
        receipt_json_with_newline.push(b'\n');
        write_if_changed(&out_root.join(receipt_relative), &receipt_json_with_newline)?;
        Ok(receipt)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewReceipt {
    pub schema_version: u32,
    pub compiler_id: &'static str,
    pub structure_id: String,
    pub label: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub family: Option<FamilyReceipt>,
    pub size: [i32; 3],
    pub authored_operation_count: usize,
    pub placed_block_count: usize,
    pub material_bill: Vec<MaterialBillEntry>,
    pub components: Vec<ComponentReceipt>,
    pub markers: Vec<MarkerReceipt>,
    pub sockets: Vec<SocketReceipt>,
    pub default_theme: String,
    pub geometry: GeometryReceipt,
    pub artifacts: ArtifactReceipt,
    pub source: SourceReceipt,
    pub lighting: LightingReceipt,
    pub asset_packs: Vec<PackReceipt>,
    pub asset_provenance: ProvenanceSummaryReceipt,
}

impl PreviewReceipt {
    #[allow(clippy::too_many_arguments)]
    fn new(
        structure: &CanonicalStructureRecord,
        geometry: GeometryReceipt,
        mesh_path: String,
        mesh_sha256: String,
        atlas_path: String,
        atlas_sha256: String,
        atlas_width: u32,
        atlas_height: u32,
        atlas_sprite_count: usize,
        asset_packs: Vec<PackReceipt>,
        asset_provenance: ProvenanceSummaryReceipt,
    ) -> Self {
        let material_bill = material_bill(structure);
        let placed_block_count = material_bill.iter().map(|entry| entry.count).sum();
        Self {
            schema_version: STRUCTURE_PREVIEW_SCHEMA_VERSION,
            compiler_id: STRUCTURE_PREVIEW_COMPILER_ID,
            structure_id: structure.template.id().to_owned(),
            label: structure.label.clone(),
            description: structure.description.clone(),
            category: structure.category.clone(),
            tags: structure.tags.clone(),
            family: structure.family.as_ref().map(|family| FamilyReceipt {
                id: family.id.clone(),
                member: family.member.clone(),
                label: family.label.clone(),
            }),
            size: structure.template.size(),
            authored_operation_count: structure.blocks.len(),
            placed_block_count,
            material_bill,
            components: structure
                .components
                .iter()
                .map(|component| ComponentReceipt {
                    id: component.id.clone(),
                    label: component.label.clone(),
                    optional: component.optional,
                    block_count: structure
                        .blocks
                        .iter()
                        .filter(|block| {
                            block.components.contains(&component.id)
                                && block.state != TemplateBlockState::Exact(AIR)
                        })
                        .count(),
                })
                .collect(),
            markers: structure
                .template
                .markers()
                .iter()
                .map(|marker| MarkerReceipt {
                    kind: marker.kind.clone(),
                    pos: block_pos_array(marker.local_pos),
                })
                .collect(),
            sockets: structure
                .sockets
                .iter()
                .map(|socket| SocketReceipt {
                    id: socket.id.clone(),
                    kind: socket.kind.clone(),
                    pos: block_pos_array(socket.local_pos),
                    facing: socket_facing(socket.facing),
                })
                .collect(),
            default_theme: structure.default_theme.clone().unwrap_or_default(),
            geometry,
            artifacts: ArtifactReceipt {
                mesh_path,
                mesh_sha256,
                atlas_path,
                atlas_sha256,
                atlas_width,
                atlas_height,
                atlas_sprite_count,
            },
            source: SourceReceipt {
                compiler_id: structure.provenance.compiler_id.clone(),
                path: structure.provenance.source_path.clone(),
                sha256: structure.provenance.source_sha256.clone(),
                semantic_sha256: structure.provenance.semantic_sha256.clone(),
            },
            lighting: LightingReceipt {
                id: "fixed-midday-full-bright-ao-v1",
                description: "Production face shading and ambient occlusion with full midday light baked into vertex colors.",
            },
            asset_packs,
            asset_provenance,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyReceipt {
    pub id: String,
    pub member: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialBillEntry {
    pub palette_key: String,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentReceipt {
    pub id: String,
    pub label: String,
    pub optional: bool,
    pub block_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarkerReceipt {
    pub kind: String,
    pub pos: [i32; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct SocketReceipt {
    pub id: String,
    pub kind: String,
    pub pos: [i32; 3],
    pub facing: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryReceipt {
    pub vertex_count: usize,
    pub index_count: usize,
    pub face_count: usize,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub solid_face_count: usize,
    pub cutout_face_count: usize,
    pub translucent_face_count: usize,
    pub component_groups: Vec<ComponentGroupReceipt>,
    pub vertical_layers: Vec<VerticalLayerReceipt>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentGroupReceipt {
    pub id: String,
    pub face_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerticalLayerReceipt {
    pub y: i32,
    pub face_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactReceipt {
    pub mesh_path: String,
    pub mesh_sha256: String,
    pub atlas_path: String,
    pub atlas_sha256: String,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub atlas_sprite_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReceipt {
    pub compiler_id: String,
    pub path: String,
    pub sha256: String,
    pub semantic_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LightingReceipt {
    pub id: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackReceipt {
    pub id: String,
    pub origin: &'static str,
    pub content_fingerprint: Option<String>,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceSummaryReceipt {
    pub first_party: usize,
    pub generated: usize,
    pub minecraft_reference: usize,
    pub unknown: usize,
    pub missing: usize,
}

#[derive(Clone, Debug, Default)]
struct GroupIndices {
    solid: Vec<u32>,
    cutout: Vec<u32>,
    translucent: Vec<u32>,
}

impl GroupIndices {
    fn layer_mut(&mut self, layer: RenderLayer) -> &mut Vec<u32> {
        match layer {
            RenderLayer::Solid => &mut self.solid,
            RenderLayer::Cutout => &mut self.cutout,
            RenderLayer::Translucent => &mut self.translucent,
        }
    }

    fn face_count(&self) -> usize {
        (self.solid.len() + self.cutout.len() + self.translucent.len()) / 6
    }
}

#[derive(Clone, Copy, Debug)]
enum RenderLayer {
    Solid,
    Cutout,
    Translucent,
}

fn mesh_placed_structure(
    size: [i32; 3],
    blocks: &[mclone_worldgen::structure_template::PlacedTemplateBlock],
    assets: &mclone_mesh::TexturedTerrainAssets,
) -> Result<TexturedVisibleChunkMesh> {
    let chunk_count_x = (size[0] + CHUNK_WIDTH - 1).div_euclid(CHUNK_WIDTH);
    let chunk_count_z = (size[2] + CHUNK_WIDTH - 1).div_euclid(CHUNK_WIDTH);
    let height = (size[1] + 15).div_euclid(16) * 16;
    let mut volumes = Vec::new();
    for chunk_z in 0..chunk_count_z {
        for chunk_x in 0..chunk_count_x {
            volumes.push(ChunkVolume {
                chunk_x,
                chunk_z,
                blocks: vec![AIR_BLOCK_STATE_ID; (height * CHUNK_WIDTH * CHUNK_WIDTH) as usize],
            });
        }
    }
    for block in blocks {
        let chunk_x = block.pos.x.div_euclid(CHUNK_WIDTH);
        let chunk_z = block.pos.z.div_euclid(CHUNK_WIDTH);
        let volume = volumes
            .iter_mut()
            .find(|volume| volume.chunk_x == chunk_x && volume.chunk_z == chunk_z)
            .with_context(|| format!("placed block {:?} escaped preview chunks", block.pos))?;
        let index = chunk_block_index(
            block.pos.x.rem_euclid(CHUNK_WIDTH),
            block.pos.y,
            block.pos.z.rem_euclid(CHUNK_WIDTH),
        );
        volume.blocks[index] = BlockStateId(u32::from(block.block));
    }
    let inputs = volumes
        .iter()
        .map(|volume| {
            TexturedChunkMeshInput::new(volume.chunk_x, volume.chunk_z, 0, height, &volume.blocks)
        })
        .collect::<Vec<_>>();
    build_textured_visible_chunk_area_mesh(&inputs, &assets.catalog)
        .context("production block mesher rejected structure preview")
}

#[derive(Debug)]
struct ChunkVolume {
    chunk_x: i32,
    chunk_z: i32,
    blocks: Vec<BlockStateId>,
}

fn block_component_assignment(structure: &CanonicalStructureRecord) -> BTreeMap<[i32; 3], String> {
    structure
        .blocks
        .iter()
        .filter(|block| block.state != TemplateBlockState::Exact(AIR))
        .map(|block| {
            let id = if block.components.is_empty() {
                "unassigned".to_owned()
            } else {
                block.components.join("+")
            };
            (block_pos_array(block.local_pos), id)
        })
        .collect()
}

fn group_mesh_indices(
    mesh: &TexturedVisibleChunkMesh,
    assignment: &BTreeMap<[i32; 3], String>,
) -> Result<BTreeMap<String, GroupIndices>> {
    if mesh.indices.len() % 6 != 0 {
        bail!("production mesh did not contain whole quad faces");
    }
    let mut groups = BTreeMap::<String, GroupIndices>::new();
    for (face_index, indices) in mesh.indices.chunks_exact(6).enumerate() {
        let layer = if face_index * 6 < mesh.solid_index_count as usize {
            RenderLayer::Solid
        } else if face_index * 6 < mesh.opaque_index_count as usize {
            RenderLayer::Cutout
        } else {
            RenderLayer::Translucent
        };
        let owner = owning_block(mesh, indices)?;
        let group = assignment
            .get(&owner)
            .cloned()
            .unwrap_or_else(|| "unassigned".to_owned());
        groups
            .entry(group)
            .or_default()
            .layer_mut(layer)
            .extend_from_slice(indices);
    }
    Ok(groups)
}

fn owning_block(mesh: &TexturedVisibleChunkMesh, indices: &[u32]) -> Result<[i32; 3]> {
    let vertex = |index: u32| -> Result<[f32; 3]> {
        mesh.vertices
            .get(index as usize)
            .map(|vertex| vertex.position)
            .context("mesh index referenced a missing vertex")
    };
    let p0 = vertex(indices[0])?;
    let p1 = vertex(indices[1])?;
    let p2 = vertex(indices[2])?;
    let p3 = vertex(indices[5])?;
    let centroid = [
        (p0[0] + p1[0] + p2[0] + p3[0]) * 0.25,
        (p0[1] + p1[1] + p2[1] + p3[1]) * 0.25,
        (p0[2] + p1[2] + p2[2] + p3[2]) * 0.25,
    ];
    let first = subtract(p1, p0);
    let second = subtract(p2, p0);
    let normal = normalize(cross(first, second));
    Ok([
        (centroid[0] - normal[0] * 0.001).floor() as i32,
        (centroid[1] - normal[1] * 0.001).floor() as i32,
        (centroid[2] - normal[2] * 0.001).floor() as i32,
    ])
}

fn build_glb(
    structure_id: &str,
    size: [i32; 3],
    mesh: &TexturedVisibleChunkMesh,
    grouped: &BTreeMap<String, GroupIndices>,
    atlas_uri: &str,
) -> Result<(Vec<u8>, GeometryReceipt)> {
    let mut binary = Vec::new();
    let mut buffer_views = Vec::<Value>::new();
    let mut accessors = Vec::<Value>::new();

    let positions = f32_bytes(mesh.vertices.iter().flat_map(|vertex| vertex.position));
    let position_view = append_buffer_view(&mut binary, &mut buffer_views, &positions, 34962);
    let (bounds_min, bounds_max) = vertex_bounds(&mesh.vertices);
    let position_accessor = accessors.len();
    accessors.push(json!({
        "bufferView": position_view,
        "componentType": 5126,
        "count": mesh.vertices.len(),
        "type": "VEC3",
        "min": bounds_min,
        "max": bounds_max,
    }));

    let uvs = f32_bytes(mesh.vertices.iter().flat_map(|vertex| vertex.uv));
    let uv_view = append_buffer_view(&mut binary, &mut buffer_views, &uvs, 34962);
    let uv_accessor = accessors.len();
    accessors.push(json!({
        "bufferView": uv_view,
        "componentType": 5126,
        "count": mesh.vertices.len(),
        "type": "VEC2",
    }));

    let colors = f32_bytes(mesh.vertices.iter().flat_map(|vertex| vertex.color));
    let color_view = append_buffer_view(&mut binary, &mut buffer_views, &colors, 34962);
    let color_accessor = accessors.len();
    accessors.push(json!({
        "bufferView": color_view,
        "componentType": 5126,
        "count": mesh.vertices.len(),
        "type": "VEC4",
    }));

    let mut gltf_meshes = Vec::<Value>::new();
    let mut nodes = Vec::<Value>::new();
    let mut component_groups = Vec::new();
    for (component_id, group) in grouped {
        let mut primitives = Vec::new();
        for (indices, material) in [
            (&group.solid, 0usize),
            (&group.cutout, 1usize),
            (&group.translucent, 2usize),
        ] {
            if indices.is_empty() {
                continue;
            }
            let bytes = u32_bytes(indices.iter().copied());
            let view = append_buffer_view(&mut binary, &mut buffer_views, &bytes, 34963);
            let accessor = accessors.len();
            accessors.push(json!({
                "bufferView": view,
                "componentType": 5125,
                "count": indices.len(),
                "type": "SCALAR",
            }));
            primitives.push(json!({
                "attributes": {
                    "POSITION": position_accessor,
                    "TEXCOORD_0": uv_accessor,
                    "COLOR_0": color_accessor,
                },
                "indices": accessor,
                "material": material,
                "mode": 4,
            }));
        }
        let mesh_index = gltf_meshes.len();
        gltf_meshes.push(json!({
            "name": format!("component:{component_id}"),
            "primitives": primitives,
        }));
        nodes.push(json!({
            "name": format!("component:{component_id}"),
            "mesh": mesh_index,
            "extras": { "componentId": component_id },
        }));
        component_groups.push(ComponentGroupReceipt {
            id: component_id.clone(),
            face_count: group.face_count(),
        });
    }
    let scene_nodes = (0..nodes.len()).collect::<Vec<_>>();
    let vertical_layers = vertical_layer_receipts(mesh)?;
    let gltf = json!({
        "asset": {
            "version": "2.0",
            "generator": STRUCTURE_PREVIEW_COMPILER_ID,
            "copyright": "Mclone first-party visual assets",
        },
        "extensionsUsed": ["KHR_materials_unlit"],
        "scene": 0,
        "scenes": [{ "name": structure_id, "nodes": scene_nodes }],
        "nodes": nodes,
        "meshes": gltf_meshes,
        "materials": [
            gltf_material("solid", "OPAQUE"),
            gltf_material("cutout", "MASK"),
            gltf_material("translucent", "BLEND"),
        ],
        "textures": [{ "sampler": 0, "source": 0 }],
        "samplers": [{
            "magFilter": 9728,
            "minFilter": 9728,
            "wrapS": 10497,
            "wrapT": 10497,
        }],
        "images": [{ "uri": atlas_uri }],
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{ "byteLength": binary.len() }],
        "extras": {
            "structureId": structure_id,
            "size": size,
            "compilerId": STRUCTURE_PREVIEW_COMPILER_ID,
        },
    });
    let json_bytes = serde_json::to_vec(&gltf)?;
    let glb = encode_glb(json_bytes, binary)?;
    let geometry = GeometryReceipt {
        vertex_count: mesh.vertices.len(),
        index_count: mesh.indices.len(),
        face_count: mesh.indices.len() / 6,
        bounds_min,
        bounds_max,
        solid_face_count: mesh.solid_index_count as usize / 6,
        cutout_face_count: (mesh.opaque_index_count - mesh.solid_index_count) as usize / 6,
        translucent_face_count: (mesh.indices.len() - mesh.opaque_index_count as usize) / 6,
        component_groups,
        vertical_layers,
    };
    Ok((glb, geometry))
}

fn gltf_material(name: &str, alpha_mode: &str) -> Value {
    json!({
        "name": name,
        "pbrMetallicRoughness": {
            "baseColorTexture": { "index": 0 },
            "metallicFactor": 0.0,
            "roughnessFactor": 1.0,
        },
        "alphaMode": alpha_mode,
        "alphaCutoff": 0.5,
        "doubleSided": true,
        "extensions": { "KHR_materials_unlit": {} },
    })
}

fn append_buffer_view(
    binary: &mut Vec<u8>,
    views: &mut Vec<Value>,
    bytes: &[u8],
    target: u32,
) -> usize {
    align4(binary, 0);
    let offset = binary.len();
    binary.extend_from_slice(bytes);
    let index = views.len();
    views.push(json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": bytes.len(),
        "target": target,
    }));
    index
}

fn encode_glb(mut json_bytes: Vec<u8>, mut binary: Vec<u8>) -> Result<Vec<u8>> {
    align4(&mut json_bytes, b' ');
    align4(&mut binary, 0);
    let total_len = 12usize
        .checked_add(8 + json_bytes.len())
        .and_then(|length| length.checked_add(8 + binary.len()))
        .context("GLB length overflow")?;
    let total_len = u32::try_from(total_len).context("GLB exceeds 4 GiB")?;
    let mut glb = Vec::with_capacity(total_len as usize);
    glb.extend_from_slice(&0x4654_6C67u32.to_le_bytes());
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&total_len.to_le_bytes());
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    glb.extend_from_slice(&json_bytes);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    glb.extend_from_slice(&binary);
    Ok(glb)
}

fn vertical_layer_receipts(mesh: &TexturedVisibleChunkMesh) -> Result<Vec<VerticalLayerReceipt>> {
    let mut layers = BTreeMap::<i32, usize>::new();
    for indices in mesh.indices.chunks_exact(6) {
        let owner = owning_block(mesh, indices)?;
        *layers.entry(owner[1]).or_default() += 1;
    }
    Ok(layers
        .into_iter()
        .map(|(y, face_count)| VerticalLayerReceipt { y, face_count })
        .collect())
}

fn material_bill(structure: &CanonicalStructureRecord) -> Vec<MaterialBillEntry> {
    let mut counts = BTreeMap::<usize, usize>::new();
    for block in &structure.blocks {
        if block.state != TemplateBlockState::Exact(AIR) {
            *counts.entry(block.palette_index).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|(palette_index, count)| MaterialBillEntry {
            palette_key: structure.palette[palette_index].key.clone(),
            count,
        })
        .collect()
}

fn vertex_bounds(vertices: &[TexturedChunkVertex]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex.position[axis]);
            max[axis] = max[axis].max(vertex.position[axis]);
        }
    }
    (min, max)
}

fn load_pack(path: &Path, expected_origin: AssetPackOrigin) -> Result<PackedAssetSource> {
    let pack = PackedAssetSource::from_file(path)
        .with_context(|| format!("failed to load asset pack {}", path.display()))?;
    if pack.manifest().origin != expected_origin {
        bail!(
            "asset pack {} declares {:?}, expected {:?}",
            path.display(),
            pack.manifest().origin,
            expected_origin
        );
    }
    Ok(pack)
}

fn pack_id(pack: &PackedAssetSource, path: &Path) -> Result<AssetPackId> {
    pack.manifest()
        .pack_id
        .clone()
        .with_context(|| format!("asset pack {} has no pack id", path.display()))
}

fn pack_receipt(path: &Path, pack: &PackedAssetSource, id: &AssetPackId) -> Result<PackReceipt> {
    Ok(PackReceipt {
        id: id.as_str().to_owned(),
        origin: origin_name(pack.manifest().origin),
        content_fingerprint: pack.manifest().content_fingerprint.clone(),
        sha256: sha256_hex(
            &fs::read(path).with_context(|| format!("failed to hash {}", path.display()))?,
        ),
    })
}

const fn origin_name(origin: AssetPackOrigin) -> &'static str {
    match origin {
        AssetPackOrigin::FirstParty => "first_party",
        AssetPackOrigin::FirstPartyProvisional => "first_party_provisional",
        AssetPackOrigin::Diagnostic => "diagnostic",
        AssetPackOrigin::Generated => "generated",
        AssetPackOrigin::MinecraftReference => "minecraft_reference",
        AssetPackOrigin::Unknown => "unknown",
    }
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(rgba, width, height, ColorType::Rgba8.into())
        .context("failed to encode structure preview atlas")?;
    Ok(bytes)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn f32_bytes(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>()
}

fn u32_bytes(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
}

fn align4(bytes: &mut Vec<u8>, value: u8) {
    while bytes.len() % 4 != 0 {
        bytes.push(value);
    }
}

const fn block_pos_array(pos: BlockPos) -> [i32; 3] {
    [pos.x, pos.y, pos.z]
}

const fn socket_facing(facing: CanonicalSocketFacing) -> &'static str {
    match facing {
        CanonicalSocketFacing::North => "north",
        CanonicalSocketFacing::East => "east",
        CanonicalSocketFacing::South => "south",
        CanonicalSocketFacing::West => "west",
        CanonicalSocketFacing::Up => "up",
        CanonicalSocketFacing::Down => "down",
    }
}

fn subtract(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [value[0] / length, value[1] / length, value[2] / length]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad_mesh() -> TexturedVisibleChunkMesh {
        TexturedVisibleChunkMesh {
            vertices: vec![
                vertex([0.0, 1.0, 0.0]),
                vertex([0.0, 1.0, 1.0]),
                vertex([1.0, 1.0, 1.0]),
                vertex([1.0, 1.0, 0.0]),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            solid_index_count: 6,
            opaque_index_count: 6,
        }
    }

    fn vertex(position: [f32; 3]) -> TexturedChunkVertex {
        TexturedChunkVertex {
            position,
            uv: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            packed_light: 15_728_880,
        }
    }

    #[test]
    fn outward_face_maps_back_to_owning_block() {
        assert_eq!(
            owning_block(&quad_mesh(), &[0, 1, 2, 0, 2, 3]).unwrap(),
            [0, 0, 0]
        );
    }

    #[test]
    fn glb_proof_is_deterministic_and_well_formed() {
        let mesh = quad_mesh();
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "shell".to_owned(),
            GroupIndices {
                solid: mesh.indices.clone(),
                ..GroupIndices::default()
            },
        );
        let (first, receipt) = build_glb("proof", [1, 1, 1], &mesh, &grouped, "atlas.png").unwrap();
        let (second, _) = build_glb("proof", [1, 1, 1], &mesh, &grouped, "atlas.png").unwrap();
        assert_eq!(first, second);
        assert_eq!(&first[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(first[4..8].try_into().unwrap()), 2);
        assert_eq!(
            u32::from_le_bytes(first[8..12].try_into().unwrap()) as usize,
            first.len()
        );
        assert_eq!(receipt.vertex_count, 4);
        assert_eq!(receipt.face_count, 1);
        assert_eq!(receipt.component_groups[0].id, "shell");
    }
}
