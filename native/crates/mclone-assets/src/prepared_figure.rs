use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;

use glam::{EulerRot, Mat4, Quat, Vec3};

use crate::{AssetError, AssetPath, AssetResult, AssetSource, FigureAsset, FigureClip, FigurePart};

pub const PREPARED_FIGURE_COMPILER_ID: &str = "mclone-prepared-figure-box-v0";

const MAX_PARTS: usize = 256;
const MAX_VERTICES: usize = u16::MAX as usize;
const MAX_TEXTURE_DIMENSION: usize = 4096;
const MAX_ATLAS_BYTES: usize = 64 * 1024 * 1024;
const PREPARED_VERTEX_BYTE_LEN: usize = 52;

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFigure {
    pub name: String,
    pub vertices: Vec<PreparedFigureVertex>,
    pub indices: Vec<u16>,
    pub parts: Vec<PreparedFigurePart>,
    pub draw_ranges: Vec<PreparedFigureDrawRange>,
    pub atlas: PreparedFigureAtlas,
    pub clips: BTreeMap<String, FigureClip>,
    pub bounds: PreparedFigureBounds,
    pub diagnostics: PreparedFigureDiagnostics,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedFigureVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub part_id: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFigurePart {
    pub name: String,
    pub parent: Option<u16>,
    pub first_person_visible: bool,
    /// Global rest-pose content transform in normalized actor-local space.
    pub rest_matrix: [[f32; 4]; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFigureDrawRange {
    pub part_id: u16,
    pub face: String,
    pub material: Option<String>,
    pub texture: Option<String>,
    pub first_index: u32,
    pub index_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFigureAtlas {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedFigureBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedFigureDiagnostics {
    pub compiler_id: &'static str,
    pub semantic_crc32: Option<u32>,
    pub part_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub draw_range_count: usize,
    pub atlas_bytes: usize,
    pub prepared_cpu_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FigurePrepareError {
    message: String,
}

impl FigurePrepareError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for FigurePrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for FigurePrepareError {}

pub fn load_prepared_figure(
    source: &impl AssetSource,
    path: &AssetPath,
) -> AssetResult<PreparedFigure> {
    let bytes = source
        .read(path)?
        .ok_or_else(|| AssetError::MissingAsset(path.clone()))?;
    let asset: FigureAsset = serde_json::from_slice(&bytes).map_err(|source| AssetError::Json {
        path: path.clone(),
        source,
    })?;
    prepare_figure_asset_with_crc(&asset, Some(crc32fast::hash(&bytes)))
        .map_err(|error| AssetError::InvalidFigure(format!("{} at {}", error, path.as_str())))
}

pub fn prepare_figure_asset(asset: &FigureAsset) -> Result<PreparedFigure, FigurePrepareError> {
    prepare_figure_asset_with_crc(asset, None)
}

fn prepare_figure_asset_with_crc(
    asset: &FigureAsset,
    semantic_crc32: Option<u32>,
) -> Result<PreparedFigure, FigurePrepareError> {
    validate_asset_header(asset)?;
    let part_names = part_name_map(&asset.parts)?;
    validate_clips(asset, &part_names)?;
    let raw_parts = build_raw_parts(asset, &part_names)?;
    let content_matrices = content_matrices(&raw_parts)?;
    let raw_bounds = raw_figure_bounds(asset, &content_matrices)?;
    let height = raw_bounds.max.y - raw_bounds.min.y;
    if !height.is_finite() || height <= 0.0 {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' has non-positive prepared height",
            asset.name
        )));
    }
    let raw_origin = Vec3::new(
        (raw_bounds.min.x + raw_bounds.max.x) * 0.5,
        raw_bounds.min.y,
        (raw_bounds.min.z + raw_bounds.max.z) * 0.5,
    );
    let inv_height = 1.0 / height;
    let mirror = Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0));
    let actor_origin = mirror.transform_point3(raw_origin);
    let normalization =
        Mat4::from_scale(Vec3::splat(inv_height)) * Mat4::from_translation(-actor_origin);

    let parts = raw_parts
        .iter()
        .zip(&content_matrices)
        .map(|(part, content)| PreparedFigurePart {
            name: part.name.clone(),
            parent: part.parent.map(|index| index as u16),
            first_person_visible: part.first_person_visible,
            rest_matrix: (normalization * mirror * *content * mirror).to_cols_array_2d(),
        })
        .collect::<Vec<_>>();

    let (atlas, atlas_regions) = build_atlas(asset)?;
    let materials = material_colors(asset)?;
    let mut vertices = Vec::with_capacity(asset.parts.len() * 24);
    let mut indices = Vec::with_capacity(asset.parts.len() * 36);
    let mut draw_ranges = Vec::with_capacity(asset.parts.len() * 6);
    for (part_index, part) in asset.parts.iter().enumerate() {
        append_box(
            asset,
            part,
            part_index as u16,
            &materials,
            &atlas_regions,
            atlas.width,
            atlas.height,
            &mut vertices,
            &mut indices,
            &mut draw_ranges,
        )?;
    }
    if vertices.len() > MAX_VERTICES {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' prepared {} vertices; limit is {}",
            asset.name,
            vertices.len(),
            MAX_VERTICES
        )));
    }

    let actor_raw_min = Vec3::new(raw_bounds.min.x, raw_bounds.min.y, -raw_bounds.max.z);
    let actor_raw_max = Vec3::new(raw_bounds.max.x, raw_bounds.max.y, -raw_bounds.min.z);
    let bounds = PreparedFigureBounds {
        min: ((actor_raw_min - actor_origin) * inv_height).to_array(),
        max: ((actor_raw_max - actor_origin) * inv_height).to_array(),
    };
    let clips = asset
        .clips
        .iter()
        .map(|(name, clip)| (name.clone(), clip.clone()))
        .collect::<BTreeMap<_, _>>();
    let prepared_cpu_bytes = vertices.len() * PREPARED_VERTEX_BYTE_LEN
        + indices.len() * std::mem::size_of::<u16>()
        + atlas.rgba.len()
        + parts.len() * std::mem::size_of::<PreparedFigurePart>();
    let diagnostics = PreparedFigureDiagnostics {
        compiler_id: PREPARED_FIGURE_COMPILER_ID,
        semantic_crc32,
        part_count: parts.len(),
        vertex_count: vertices.len(),
        index_count: indices.len(),
        draw_range_count: draw_ranges.len(),
        atlas_bytes: atlas.rgba.len(),
        prepared_cpu_bytes,
    };

    Ok(PreparedFigure {
        name: asset.name.clone(),
        vertices,
        indices,
        parts,
        draw_ranges,
        atlas,
        clips,
        bounds,
        diagnostics,
    })
}

fn validate_asset_header(asset: &FigureAsset) -> Result<(), FigurePrepareError> {
    if asset.schema_version != 1 {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' uses unsupported schema version {}",
            asset.name, asset.schema_version
        )));
    }
    if asset.parts.is_empty() {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' has no parts",
            asset.name
        )));
    }
    if asset.parts.len() > MAX_PARTS {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' has {} parts; limit is {}",
            asset.name,
            asset.parts.len(),
            MAX_PARTS
        )));
    }
    Ok(())
}

fn part_name_map(parts: &[FigurePart]) -> Result<HashMap<&str, usize>, FigurePrepareError> {
    let mut names = HashMap::with_capacity(parts.len());
    for (index, part) in parts.iter().enumerate() {
        if part.name.is_empty() {
            return Err(FigurePrepareError::new("figure has an empty part name"));
        }
        if names.insert(part.name.as_str(), index).is_some() {
            return Err(FigurePrepareError::new(format!(
                "figure has duplicate part '{}'",
                part.name
            )));
        }
    }
    Ok(names)
}

fn validate_clips(
    asset: &FigureAsset,
    part_names: &HashMap<&str, usize>,
) -> Result<(), FigurePrepareError> {
    for (clip_name, clip) in &asset.clips {
        if let Some(fps) = clip.fps
            && (!fps.is_finite() || fps <= 0.0)
        {
            return Err(FigurePrepareError::new(format!(
                "figure '{}' clip '{}' has invalid fps {}",
                asset.name, clip_name, fps
            )));
        }
        if clip.keys.is_empty() {
            return Err(FigurePrepareError::new(format!(
                "figure '{}' clip '{}' has no keys",
                asset.name, clip_name
            )));
        }
        for key in &clip.keys {
            if !part_names.contains_key(key.0.as_str()) {
                return Err(FigurePrepareError::new(format!(
                    "figure '{}' clip '{}' references unknown part '{}'",
                    asset.name, clip_name, key.0
                )));
            }
            if !key.1.is_finite() || key.1 < 0.0 {
                return Err(FigurePrepareError::new(format!(
                    "figure '{}' clip '{}' has invalid key time {}",
                    asset.name, clip_name, key.1
                )));
            }
            for (name, value) in [("at", key.2.at), ("rot", key.2.rot), ("scale", key.2.scale)] {
                if value.is_some_and(|value| value.iter().any(|component| !component.is_finite())) {
                    return Err(FigurePrepareError::new(format!(
                        "figure '{}' clip '{}' has non-finite {} key",
                        asset.name, clip_name, name
                    )));
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct RawPart {
    name: String,
    parent: Option<usize>,
    base_position: Vec3,
    base_rotation: Vec3,
    pivot: Vec3,
    first_person_visible: bool,
}

fn build_raw_parts(
    asset: &FigureAsset,
    part_names: &HashMap<&str, usize>,
) -> Result<Vec<RawPart>, FigurePrepareError> {
    let mut parts = Vec::with_capacity(asset.parts.len());
    for part in &asset.parts {
        if part.primitive.kind != "box" {
            return Err(FigurePrepareError::new(format!(
                "figure '{}' part '{}' uses unsupported prepared primitive '{}'; this proof accepts only boxes",
                asset.name, part.name, part.primitive.kind
            )));
        }
        let size = finite_positive_vec3(part.primitive.size, asset, part, "size")?;
        if size.min_element() <= 0.0 {
            return Err(FigurePrepareError::new(format!(
                "figure '{}' box part '{}' has non-positive size {:?}",
                asset.name, part.name, size
            )));
        }
        if let Some(faces) = &part.primitive.faces {
            for face in faces.keys() {
                BoxFace::from_name(face).ok_or_else(|| {
                    FigurePrepareError::new(format!(
                        "figure '{}' part '{}' has unknown box face '{}'",
                        asset.name, part.name, face
                    ))
                })?;
            }
        }
        let at = finite_vec3(part.at.unwrap_or([0.0, 0.0, 0.0]), asset, part, "at")?;
        let rotation_degrees =
            finite_vec3(part.rot.unwrap_or([0.0, 0.0, 0.0]), asset, part, "rot")?;
        let pivot = finite_vec3(
            part.joint
                .as_ref()
                .and_then(|joint| joint.pivot)
                .or(part.pivot)
                .unwrap_or([0.0, 0.0, 0.0]),
            asset,
            part,
            "pivot",
        )?;
        let parent = part
            .parent
            .as_deref()
            .map(|name| {
                part_names.get(name).copied().ok_or_else(|| {
                    FigurePrepareError::new(format!(
                        "figure '{}' part '{}' has unknown parent '{}'",
                        asset.name, part.name, name
                    ))
                })
            })
            .transpose()?;
        parts.push(RawPart {
            name: part.name.clone(),
            parent,
            base_position: at + pivot,
            base_rotation: Vec3::new(
                rotation_degrees.x.to_radians(),
                rotation_degrees.y.to_radians(),
                rotation_degrees.z.to_radians(),
            ),
            pivot,
            first_person_visible: false,
        });
    }
    let mut visibility_cache = vec![None; parts.len()];
    let mut visiting = vec![false; parts.len()];
    for index in 0..parts.len() {
        parts[index].first_person_visible =
            first_person_visible(index, &parts, &mut visibility_cache, &mut visiting)?;
    }
    Ok(parts)
}

fn finite_positive_vec3(
    value: Option<[f32; 3]>,
    asset: &FigureAsset,
    part: &FigurePart,
    field: &str,
) -> Result<Vec3, FigurePrepareError> {
    let value = value.ok_or_else(|| {
        FigurePrepareError::new(format!(
            "figure '{}' part '{}' is missing {}",
            asset.name, part.name, field
        ))
    })?;
    finite_vec3(value, asset, part, field)
}

fn finite_vec3(
    value: [f32; 3],
    asset: &FigureAsset,
    part: &FigurePart,
    field: &str,
) -> Result<Vec3, FigurePrepareError> {
    let value = Vec3::from_array(value);
    if !value.is_finite() {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' part '{}' has non-finite {}",
            asset.name, part.name, field
        )));
    }
    Ok(value)
}

fn first_person_visible(
    index: usize,
    parts: &[RawPart],
    cache: &mut [Option<bool>],
    visiting: &mut [bool],
) -> Result<bool, FigurePrepareError> {
    if let Some(value) = cache[index] {
        return Ok(value);
    }
    if visiting[index] {
        return Err(FigurePrepareError::new(format!(
            "figure has a parent cycle at part '{}'",
            parts[index].name
        )));
    }
    visiting[index] = true;
    let mut visible = !is_first_person_hidden_part_name(&parts[index].name);
    if visible && let Some(parent) = parts[index].parent {
        visible = first_person_visible(parent, parts, cache, visiting)?;
    }
    visiting[index] = false;
    cache[index] = Some(visible);
    Ok(visible)
}

fn is_first_person_hidden_part_name(name: &str) -> bool {
    name == "head" || name.ends_with("_head")
}

fn content_matrices(parts: &[RawPart]) -> Result<Vec<Mat4>, FigurePrepareError> {
    let mut cache = vec![None; parts.len()];
    let mut visiting = vec![false; parts.len()];
    let mut matrices = Vec::with_capacity(parts.len());
    for index in 0..parts.len() {
        matrices.push(content_matrix(index, parts, &mut cache, &mut visiting)?);
    }
    Ok(matrices)
}

fn content_matrix(
    index: usize,
    parts: &[RawPart],
    cache: &mut [Option<Mat4>],
    visiting: &mut [bool],
) -> Result<Mat4, FigurePrepareError> {
    if let Some(value) = cache[index] {
        return Ok(value);
    }
    if visiting[index] {
        return Err(FigurePrepareError::new(format!(
            "figure has a parent cycle at part '{}'",
            parts[index].name
        )));
    }
    visiting[index] = true;
    let part = &parts[index];
    let parent = part
        .parent
        .map(|parent| content_matrix(parent, parts, cache, visiting))
        .transpose()?
        .unwrap_or(Mat4::IDENTITY);
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        part.base_rotation.x,
        part.base_rotation.y,
        part.base_rotation.z,
    );
    let matrix = parent
        * Mat4::from_translation(part.base_position)
        * Mat4::from_quat(rotation)
        * Mat4::from_translation(-part.pivot);
    if !matrix.is_finite() {
        return Err(FigurePrepareError::new(format!(
            "figure part '{}' produced a non-finite rest matrix",
            part.name
        )));
    }
    visiting[index] = false;
    cache[index] = Some(matrix);
    Ok(matrix)
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: Vec3,
    max: Vec3,
}

fn raw_figure_bounds(
    asset: &FigureAsset,
    content_matrices: &[Mat4],
) -> Result<Bounds, FigurePrepareError> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for (part, matrix) in asset.parts.iter().zip(content_matrices) {
        let size = Vec3::from_array(part.primitive.size.expect("box size validated"));
        let half = size * 0.5;
        for x in [-half.x, half.x] {
            for y in [-half.y, half.y] {
                for z in [-half.z, half.z] {
                    let point = matrix.transform_point3(Vec3::new(x, y, z));
                    min = min.min(point);
                    max = max.max(point);
                }
            }
        }
    }
    if !min.is_finite() || !max.is_finite() {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' produced non-finite bounds",
            asset.name
        )));
    }
    Ok(Bounds { min, max })
}

#[derive(Clone, Copy, Debug)]
struct AtlasRegion {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

fn build_atlas(
    asset: &FigureAsset,
) -> Result<(PreparedFigureAtlas, BTreeMap<String, AtlasRegion>), FigurePrepareError> {
    let mut texture_names = asset.textures.keys().cloned().collect::<Vec<_>>();
    texture_names.sort();
    let mut widths = BTreeMap::new();
    let mut atlas_width = 3_usize;
    let mut atlas_height = 3_usize;
    for name in &texture_names {
        let texture = &asset.textures[name];
        let (width, height) = validate_texture(asset, name, texture)?;
        widths.insert(name.clone(), (width, height));
        atlas_width = atlas_width.checked_add(width + 2).ok_or_else(|| {
            FigurePrepareError::new(format!(
                "figure '{}' texture atlas width overflowed",
                asset.name
            ))
        })?;
        atlas_height = atlas_height.max(height + 2);
    }
    if atlas_width > MAX_TEXTURE_DIMENSION || atlas_height > MAX_TEXTURE_DIMENSION {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' texture atlas {}x{} exceeds {}",
            asset.name, atlas_width, atlas_height, MAX_TEXTURE_DIMENSION
        )));
    }
    let byte_len = atlas_width
        .checked_mul(atlas_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| FigurePrepareError::new("figure texture atlas byte size overflowed"))?;
    if byte_len > MAX_ATLAS_BYTES {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' texture atlas uses {} bytes; limit is {}",
            asset.name, byte_len, MAX_ATLAS_BYTES
        )));
    }
    let mut rgba = vec![255_u8; byte_len];
    let mut regions = BTreeMap::new();
    let mut cursor_x = 3_usize;
    for name in texture_names {
        let texture = &asset.textures[&name];
        let (width, height) = widths[&name];
        let region = AtlasRegion {
            x: cursor_x + 1,
            y: 1,
            width,
            height,
        };
        for (row_index, row) in texture.pixels.iter().enumerate() {
            for (column_index, symbol) in row.chars().enumerate() {
                let color = texture.palette.get(&symbol.to_string()).ok_or_else(|| {
                    FigurePrepareError::new(format!(
                        "figure '{}' texture '{}' references unknown palette symbol '{}'",
                        asset.name, name, symbol
                    ))
                })?;
                let color = parse_hex_color(color).map_err(|message| {
                    FigurePrepareError::new(format!(
                        "figure '{}' texture '{}' palette symbol '{}' {}",
                        asset.name, name, symbol, message
                    ))
                })?;
                set_atlas_pixel(
                    &mut rgba,
                    atlas_width,
                    region.x + column_index,
                    region.y + row_index,
                    color,
                );
            }
        }
        for row in 0..height {
            let left = atlas_pixel(&rgba, atlas_width, region.x, region.y + row);
            let right = atlas_pixel(&rgba, atlas_width, region.x + width - 1, region.y + row);
            set_atlas_pixel(&mut rgba, atlas_width, region.x - 1, region.y + row, left);
            set_atlas_pixel(
                &mut rgba,
                atlas_width,
                region.x + width,
                region.y + row,
                right,
            );
        }
        for column in 0..(width + 2) {
            let x = region.x - 1 + column;
            let top = atlas_pixel(&rgba, atlas_width, x, region.y);
            let bottom = atlas_pixel(&rgba, atlas_width, x, region.y + height - 1);
            set_atlas_pixel(&mut rgba, atlas_width, x, region.y - 1, top);
            set_atlas_pixel(&mut rgba, atlas_width, x, region.y + height, bottom);
        }
        regions.insert(name, region);
        cursor_x += width + 2;
    }
    Ok((
        PreparedFigureAtlas {
            width: atlas_width as u32,
            height: atlas_height as u32,
            rgba,
        },
        regions,
    ))
}

fn validate_texture(
    asset: &FigureAsset,
    name: &str,
    texture: &crate::FigureAsciiTexture,
) -> Result<(usize, usize), FigurePrepareError> {
    let first = texture.pixels.first().ok_or_else(|| {
        FigurePrepareError::new(format!(
            "figure '{}' texture '{}' has no rows",
            asset.name, name
        ))
    })?;
    let width = first.chars().count();
    if width == 0 {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' texture '{}' has an empty row",
            asset.name, name
        )));
    }
    if texture
        .pixels
        .iter()
        .any(|row| row.chars().count() != width)
    {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' texture '{}' has ragged rows",
            asset.name, name
        )));
    }
    Ok((width, texture.pixels.len()))
}

fn set_atlas_pixel(rgba: &mut [u8], width: usize, x: usize, y: usize, color: [u8; 4]) {
    let start = (y * width + x) * 4;
    rgba[start..start + 4].copy_from_slice(&color);
}

fn atlas_pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    let start = (y * width + x) * 4;
    rgba[start..start + 4].try_into().expect("RGBA pixel")
}

fn material_colors(asset: &FigureAsset) -> Result<HashMap<String, [f32; 4]>, FigurePrepareError> {
    asset
        .materials
        .iter()
        .map(|(name, material)| {
            let color = parse_hex_color(&material.color).map_err(|message| {
                FigurePrepareError::new(format!(
                    "figure '{}' material '{}' {}",
                    asset.name, name, message
                ))
            })?;
            Ok((name.clone(), rgba8_to_float(color)))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn append_box(
    asset: &FigureAsset,
    part: &FigurePart,
    part_id: u16,
    materials: &HashMap<String, [f32; 4]>,
    atlas_regions: &BTreeMap<String, AtlasRegion>,
    atlas_width: u32,
    atlas_height: u32,
    vertices: &mut Vec<PreparedFigureVertex>,
    indices: &mut Vec<u16>,
    draw_ranges: &mut Vec<PreparedFigureDrawRange>,
) -> Result<(), FigurePrepareError> {
    let size = Vec3::from_array(part.primitive.size.expect("box size validated"));
    for face in BoxFace::ALL {
        let override_face = part
            .primitive
            .faces
            .as_ref()
            .and_then(|faces| faces.get(face.name()));
        let material_name = override_face
            .and_then(|face| face.material.as_deref())
            .or(part.material.as_deref());
        let texture_name = override_face
            .and_then(|face| face.texture.as_deref())
            .or(part.texture.as_deref());
        let color = match material_name {
            Some(name) => *materials.get(name).ok_or_else(|| {
                FigurePrepareError::new(format!(
                    "figure '{}' part '{}' references unknown material '{}'",
                    asset.name, part.name, name
                ))
            })?,
            None if texture_name.is_some() => [1.0, 1.0, 1.0, 1.0],
            None => rgba8_to_float(parse_hex_color("#d7dde2").expect("default color")),
        };
        let region = texture_name
            .map(|name| {
                atlas_regions.get(name).copied().ok_or_else(|| {
                    FigurePrepareError::new(format!(
                        "figure '{}' part '{}' references unknown texture '{}'",
                        asset.name, part.name, name
                    ))
                })
            })
            .transpose()?;
        let first_index = indices.len() as u32;
        append_box_face(
            size,
            face,
            part_id,
            color,
            region,
            atlas_width,
            atlas_height,
            vertices,
            indices,
        )?;
        draw_ranges.push(PreparedFigureDrawRange {
            part_id,
            face: face.name().to_owned(),
            material: material_name.map(str::to_owned),
            texture: texture_name.map(str::to_owned),
            first_index,
            index_count: 6,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_box_face(
    size: Vec3,
    face: BoxFace,
    part_id: u16,
    color: [f32; 4],
    region: Option<AtlasRegion>,
    atlas_width: u32,
    atlas_height: u32,
    vertices: &mut Vec<PreparedFigureVertex>,
    indices: &mut Vec<u16>,
) -> Result<(), FigurePrepareError> {
    let plane = face.plane(size);
    let base_vertex = u16::try_from(vertices.len())
        .map_err(|_| FigurePrepareError::new("prepared figure exceeds u16 vertex index range"))?;
    for iy in 0..=1 {
        let y = iy as f32 * plane.height - plane.height * 0.5;
        for ix in 0..=1 {
            let x = ix as f32 * plane.width - plane.width * 0.5;
            let mut position = [0.0_f32; 3];
            position[plane.u] = x * plane.udir;
            position[plane.v] = y * plane.vdir;
            position[plane.w] = plane.depth * 0.5;
            position[2] = -position[2];
            let mut normal = [0.0_f32; 3];
            normal[plane.w] = if plane.depth > 0.0 { 1.0 } else { -1.0 };
            normal[2] = -normal[2];
            let semantic_uv = [ix as f32, 1.0 - iy as f32];
            vertices.push(PreparedFigureVertex {
                position,
                normal,
                uv: atlas_uv(region, semantic_uv, atlas_width, atlas_height),
                color,
                part_id: u32::from(part_id),
            });
        }
    }
    // Mirroring local Z changes handedness, so reverse Three.js's triangles.
    indices.extend_from_slice(&[
        base_vertex,
        base_vertex + 1,
        base_vertex + 2,
        base_vertex + 2,
        base_vertex + 1,
        base_vertex + 3,
    ]);
    Ok(())
}

fn atlas_uv(
    region: Option<AtlasRegion>,
    semantic_uv: [f32; 2],
    atlas_width: u32,
    atlas_height: u32,
) -> [f32; 2] {
    let Some(region) = region else {
        return [1.5 / atlas_width as f32, 1.5 / atlas_height as f32];
    };
    [
        (region.x as f32 + semantic_uv[0] * region.width as f32) / atlas_width as f32,
        (region.y as f32 + (1.0 - semantic_uv[1]) * region.height as f32) / atlas_height as f32,
    ]
}

fn parse_hex_color(value: &str) -> Result<[u8; 4], String> {
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| format!("expected #rrggbb color, got '{}'", value))?;
    if hex.len() != 6 {
        return Err(format!("expected #rrggbb color, got '{}'", value));
    }
    let red = u8::from_str_radix(&hex[0..2], 16).map_err(|error| error.to_string())?;
    let green = u8::from_str_radix(&hex[2..4], 16).map_err(|error| error.to_string())?;
    let blue = u8::from_str_radix(&hex[4..6], 16).map_err(|error| error.to_string())?;
    Ok([red, green, blue, 255])
}

fn rgba8_to_float(value: [u8; 4]) -> [f32; 4] {
    [
        value[0] as f32 / 255.0,
        value[1] as f32 / 255.0,
        value[2] as f32 / 255.0,
        value[3] as f32 / 255.0,
    ]
}

#[derive(Clone, Copy, Debug)]
struct Plane {
    u: usize,
    v: usize,
    w: usize,
    udir: f32,
    vdir: f32,
    width: f32,
    height: f32,
    depth: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoxFace {
    East,
    West,
    Up,
    Down,
    South,
    North,
}

impl BoxFace {
    /// Three.js `BoxGeometry` group/material order.
    const ALL: [Self; 6] = [
        Self::East,
        Self::West,
        Self::Up,
        Self::Down,
        Self::South,
        Self::North,
    ];

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "east" => Some(Self::East),
            "west" => Some(Self::West),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "south" => Some(Self::South),
            "north" => Some(Self::North),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::East => "east",
            Self::West => "west",
            Self::Up => "up",
            Self::Down => "down",
            Self::South => "south",
            Self::North => "north",
        }
    }

    fn plane(self, size: Vec3) -> Plane {
        match self {
            Self::East => Plane {
                u: 2,
                v: 1,
                w: 0,
                udir: -1.0,
                vdir: -1.0,
                width: size.z,
                height: size.y,
                depth: size.x,
            },
            Self::West => Plane {
                u: 2,
                v: 1,
                w: 0,
                udir: 1.0,
                vdir: -1.0,
                width: size.z,
                height: size.y,
                depth: -size.x,
            },
            Self::Up => Plane {
                u: 0,
                v: 2,
                w: 1,
                udir: 1.0,
                vdir: 1.0,
                width: size.x,
                height: size.z,
                depth: size.y,
            },
            Self::Down => Plane {
                u: 0,
                v: 2,
                w: 1,
                udir: 1.0,
                vdir: -1.0,
                width: size.x,
                height: size.z,
                depth: -size.y,
            },
            Self::South => Plane {
                u: 0,
                v: 1,
                w: 2,
                udir: 1.0,
                vdir: -1.0,
                width: size.x,
                height: size.y,
                depth: size.z,
            },
            Self::North => Plane {
                u: 0,
                v: 1,
                w: 2,
                udir: -1.0,
                vdir: -1.0,
                width: size.x,
                height: size.y,
                depth: -size.z,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER_FIGURE_JSON: &str =
        include_str!("../../../../assets/mclone/figures/player.figure.json");

    #[test]
    fn prepares_player_as_static_box_geometry() {
        let asset: FigureAsset = serde_json::from_str(PLAYER_FIGURE_JSON).unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();

        assert_eq!(prepared.parts.len(), 12);
        assert_eq!(prepared.vertices.len(), 288);
        assert_eq!(prepared.indices.len(), 432);
        assert_eq!(prepared.draw_ranges.len(), 72);
        assert_eq!(prepared.atlas.width, 13);
        assert_eq!(prepared.atlas.height, 10);
        assert!((prepared.bounds.min[1] - 0.0).abs() < 1.0e-6);
        assert!((prepared.bounds.max[1] - 1.0).abs() < 1.0e-6);
        assert_eq!(
            prepared
                .draw_ranges
                .iter()
                .filter(|range| range.texture.as_deref() == Some("face"))
                .count(),
            1
        );
        assert!(prepared.clips.contains_key("walk"));
    }

    #[test]
    fn repeated_player_preparation_is_equal() {
        let asset: FigureAsset = serde_json::from_str(PLAYER_FIGURE_JSON).unwrap();
        assert_eq!(
            prepare_figure_asset(&asset).unwrap(),
            prepare_figure_asset(&asset).unwrap()
        );
    }

    #[test]
    fn box_geometry_matches_pinned_three_face_order_and_attributes() {
        let asset: FigureAsset = serde_json::from_str(
            r##"{
              "schemaVersion": 1,
              "name": "three_box_oracle",
              "materials": { "white": { "color": "#ffffff" } },
              "textures": {
                "oracle": {
                  "palette": { "a": "#ffffff", "b": "#000000" },
                  "pixels": ["ab", "ba"]
                }
              },
              "parts": [
                {
                  "name": "box",
                  "material": "white",
                  "primitive": {
                    "kind": "box",
                    "size": [2, 4, 6],
                    "faces": { "east": { "texture": "oracle" } }
                  }
                }
              ],
              "clips": {}
            }"##,
        )
        .unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();

        let expected_positions = [
            [1.0, 2.0, -3.0],
            [1.0, 2.0, 3.0],
            [1.0, -2.0, -3.0],
            [1.0, -2.0, 3.0],
        ];
        let expected_uvs = [[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]];
        for (index, (position, uv)) in expected_positions.into_iter().zip(expected_uvs).enumerate()
        {
            assert_eq!(prepared.vertices[index].position, position);
            assert_eq!(prepared.vertices[index].normal, [1.0, 0.0, -0.0]);
            assert_eq!(
                prepared.vertices[index].uv,
                [(4.0 + uv[0] * 2.0) / 7.0, (1.0 + (1.0 - uv[1]) * 2.0) / 4.0]
            );
        }
        assert_eq!(&prepared.indices[..6], &[0, 1, 2, 2, 1, 3]);
        let face_normals = [
            [1.0, 0.0, -0.0],
            [-1.0, 0.0, -0.0],
            [0.0, 1.0, -0.0],
            [0.0, -1.0, -0.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, 1.0],
        ];
        for (face, normal) in face_normals.into_iter().enumerate() {
            assert_eq!(prepared.vertices[face * 4].normal, normal);
        }
    }

    #[test]
    fn north_texture_maps_to_actor_front_without_overlay_geometry() {
        let asset: FigureAsset = serde_json::from_str(PLAYER_FIGURE_JSON).unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();
        let range = prepared
            .draw_ranges
            .iter()
            .find(|range| range.texture.as_deref() == Some("face"))
            .unwrap();
        assert_eq!(range.face, "north");
        let first_vertex = range.part_id as usize * 24 + 5 * 4;
        assert_eq!(prepared.vertices[first_vertex].normal, [0.0, 0.0, 1.0]);
        assert_eq!(prepared.vertices.len(), 12 * 24);
    }

    #[test]
    fn rejects_curved_primitives_in_static_box_proof() {
        let asset: FigureAsset = serde_json::from_str(
            r##"{
              "schemaVersion": 1,
              "name": "sphere",
              "materials": {},
              "textures": {},
              "parts": [{ "name": "body", "primitive": { "kind": "sphere", "radius": 1 } }],
              "clips": {}
            }"##,
        )
        .unwrap();
        let error = prepare_figure_asset(&asset).unwrap_err().to_string();
        assert!(error.contains("accepts only boxes"));
    }

    #[test]
    fn rejects_parent_cycles() {
        let asset: FigureAsset = serde_json::from_str(
            r##"{
              "schemaVersion": 1,
              "name": "cycle",
              "materials": {},
              "textures": {},
              "parts": [
                { "name": "a", "parent": "b", "primitive": { "kind": "box", "size": [1, 1, 1] } },
                { "name": "b", "parent": "a", "primitive": { "kind": "box", "size": [1, 1, 1] } }
              ],
              "clips": {}
            }"##,
        )
        .unwrap();
        let error = prepare_figure_asset(&asset).unwrap_err().to_string();
        assert!(error.contains("parent cycle"));
    }

    #[test]
    fn rejects_figures_over_the_part_budget() {
        let mut parts = Vec::new();
        for index in 0..=MAX_PARTS {
            parts.push(serde_json::json!({
                "name": format!("part-{index}"),
                "primitive": { "kind": "box", "size": [1, 1, 1] }
            }));
        }
        let asset: FigureAsset = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "name": "over-budget",
            "materials": {},
            "textures": {},
            "parts": parts,
            "clips": {}
        }))
        .unwrap();
        let error = prepare_figure_asset(&asset).unwrap_err().to_string();
        assert!(error.contains("limit is 256"));
    }
}
