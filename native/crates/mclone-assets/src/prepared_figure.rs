use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;

use glam::{EulerRot, Mat3, Mat4, Quat, Vec3};

use crate::{AssetError, AssetPath, AssetResult, AssetSource, FigureAsset, FigurePart};

pub const PREPARED_FIGURE_COMPILER_ID: &str = "mclone-prepared-figure-cuboid-proxy-v1";

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
    pub evaluation_order: Vec<u16>,
    pub draw_ranges: Vec<PreparedFigureDrawRange>,
    pub atlas: PreparedFigureAtlas,
    pub clips: BTreeMap<String, PreparedFigureClip>,
    pub normalization_matrix: [[f32; 4]; 4],
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
    pub primitive_kind: PreparedFigurePrimitiveKind,
    /// Semantic source-space group position (`part.at + pivot`).
    pub source_base_position: [f32; 3],
    /// Semantic source-space base Euler rotation in XYZ radians.
    pub source_base_rotation_radians: [f32; 3],
    /// Semantic source-space pivot inherited by the part content and children.
    pub source_pivot: [f32; 3],
    /// Global rest-pose content transform in normalized actor-local space.
    pub rest_matrix: [[f32; 4]; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedFigurePrimitiveKind {
    Box,
    SphereCuboidProxy,
    CapsuleCuboidProxy,
    CylinderCuboidProxy,
}

impl PreparedFigurePrimitiveKind {
    pub const fn source_kind(self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::SphereCuboidProxy => "sphere",
            Self::CapsuleCuboidProxy => "capsule",
            Self::CylinderCuboidProxy => "cylinder",
        }
    }

    pub const fn is_cuboid_proxy(self) -> bool {
        !matches!(self, Self::Box)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFigureClip {
    pub duration_seconds: f32,
    pub source_fps: Option<f32>,
    pub looped: bool,
    pub tracks: BTreeMap<u16, Vec<PreparedFigureClipKey>>,
    pub locomotion: Option<PreparedFigureClipLocomotion>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PreparedFigureClipKey {
    pub time_seconds: f32,
    pub translation: Option<[f32; 3]>,
    pub rotation_radians: Option<[f32; 3]>,
    pub scale: Option<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFigureClipLocomotion {
    pub kind: String,
    pub cycle_distance: f32,
    pub contacts: Vec<PreparedFigureClipContact>,
    pub direction: Option<[f32; 3]>,
    pub speed: Option<f32>,
    pub units: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFigureClipContact {
    pub part_id: u16,
    pub phase_start: f32,
    pub phase_end: f32,
    pub role: Option<String>,
    pub stance_ratio: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedFigurePoseSample {
    pub requested_time_seconds: f64,
    pub local_time_seconds: f64,
    pub duration_seconds: f64,
    pub evaluated_part_count: usize,
    pub sampled_track_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedFigurePartRotationOverride {
    pub part_id: u16,
    /// Additive actor-local Euler rotation in semantic source XYZ radians.
    pub rotation_delta_radians: [f32; 3],
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
    pub box_primitive_count: usize,
    pub sphere_cuboid_proxy_count: usize,
    pub capsule_cuboid_proxy_count: usize,
    pub cylinder_cuboid_proxy_count: usize,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FigurePoseError {
    message: String,
}

impl FigurePoseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for FigurePoseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for FigurePoseError {}

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

/// Recompose the prepared rest pose through the same local hierarchy used by
/// animation. The caller-owned palette retains capacity across frames.
pub fn evaluate_prepared_figure_rest_pose_into(
    figure: &PreparedFigure,
    palette: &mut Vec<[[f32; 4]; 4]>,
) -> Result<(), FigurePoseError> {
    evaluate_prepared_figure_pose_into(figure, None, 0.0, &[], palette).map(|_| ())
}

/// Evaluate one prepared clip at an arbitrary presentation timestamp.
///
/// This function has no fixed update cadence. Looped clips wrap continuous
/// time, non-looped clips clamp it, and the final actor-local part matrices are
/// written into caller-owned storage without rebuilding figure topology.
pub fn evaluate_prepared_figure_clip_into(
    figure: &PreparedFigure,
    clip_name: &str,
    presentation_time_seconds: f64,
    palette: &mut Vec<[[f32; 4]; 4]>,
) -> Result<PreparedFigurePoseSample, FigurePoseError> {
    let clip = figure.clips.get(clip_name).ok_or_else(|| {
        FigurePoseError::new(format!(
            "prepared figure '{}' has no clip '{}'",
            figure.name, clip_name
        ))
    })?;
    evaluate_prepared_figure_pose_into(figure, Some(clip), presentation_time_seconds, &[], palette)
}

/// Evaluate a prepared clip while composing a small set of independent
/// actor-local rotation channels before parent hierarchy composition.
pub fn evaluate_prepared_figure_clip_with_part_rotation_overrides_into(
    figure: &PreparedFigure,
    clip_name: &str,
    presentation_time_seconds: f64,
    rotation_overrides: &[PreparedFigurePartRotationOverride],
    palette: &mut Vec<[[f32; 4]; 4]>,
) -> Result<PreparedFigurePoseSample, FigurePoseError> {
    let clip = figure.clips.get(clip_name).ok_or_else(|| {
        FigurePoseError::new(format!(
            "prepared figure '{}' has no clip '{}'",
            figure.name, clip_name
        ))
    })?;
    for rotation_override in rotation_overrides {
        if usize::from(rotation_override.part_id) >= figure.parts.len()
            || rotation_override
                .rotation_delta_radians
                .iter()
                .any(|value| !value.is_finite())
        {
            return Err(FigurePoseError::new(format!(
                "prepared figure '{}' has an invalid part rotation override for {}",
                figure.name, rotation_override.part_id
            )));
        }
    }
    evaluate_prepared_figure_pose_into(
        figure,
        Some(clip),
        presentation_time_seconds,
        rotation_overrides,
        palette,
    )
}

fn evaluate_prepared_figure_pose_into(
    figure: &PreparedFigure,
    clip: Option<&PreparedFigureClip>,
    presentation_time_seconds: f64,
    rotation_overrides: &[PreparedFigurePartRotationOverride],
    palette: &mut Vec<[[f32; 4]; 4]>,
) -> Result<PreparedFigurePoseSample, FigurePoseError> {
    if !presentation_time_seconds.is_finite() {
        return Err(FigurePoseError::new(format!(
            "prepared figure '{}' pose time must be finite",
            figure.name
        )));
    }
    if figure.evaluation_order.len() != figure.parts.len() {
        return Err(FigurePoseError::new(format!(
            "prepared figure '{}' has an incomplete part evaluation order",
            figure.name
        )));
    }
    let duration_seconds = clip.map_or(0.0, |clip| f64::from(clip.duration_seconds));
    let local_time_seconds = match clip {
        Some(clip) if clip.looped && duration_seconds > 0.0 => {
            presentation_time_seconds.rem_euclid(duration_seconds)
        }
        Some(_) => presentation_time_seconds.clamp(0.0, duration_seconds),
        None => 0.0,
    };
    let local_time = local_time_seconds as f32;
    let normalization = Mat4::from_cols_array_2d(&figure.normalization_matrix);
    palette.clear();
    palette.resize(figure.parts.len(), Mat4::IDENTITY.to_cols_array_2d());

    for &part_id in &figure.evaluation_order {
        let part_index = usize::from(part_id);
        let part = figure.parts.get(part_index).ok_or_else(|| {
            FigurePoseError::new(format!(
                "prepared figure '{}' evaluation order references part {}",
                figure.name, part_index
            ))
        })?;
        let track = clip.and_then(|clip| clip.tracks.get(&part_id));
        let rotation_delta = rotation_overrides
            .iter()
            .find(|rotation_override| rotation_override.part_id == part_id)
            .map_or(Vec3::ZERO, |rotation_override| {
                Vec3::from_array(rotation_override.rotation_delta_radians)
            });
        let local_content =
            prepared_part_local_content_matrix(part, track, local_time, rotation_delta);
        let content = match part.parent {
            Some(parent_id) => {
                let parent_index = usize::from(parent_id);
                if parent_index >= palette.len() {
                    return Err(FigurePoseError::new(format!(
                        "prepared figure '{}' part '{}' has invalid parent {}",
                        figure.name, part.name, parent_index
                    )));
                }
                Mat4::from_cols_array_2d(&palette[parent_index]) * local_content
            }
            None => normalization * local_content,
        };
        if !content.is_finite() {
            return Err(FigurePoseError::new(format!(
                "prepared figure '{}' part '{}' produced a non-finite pose matrix",
                figure.name, part.name
            )));
        }
        palette[part_index] = content.to_cols_array_2d();
    }

    Ok(PreparedFigurePoseSample {
        requested_time_seconds: presentation_time_seconds,
        local_time_seconds,
        duration_seconds,
        evaluated_part_count: figure.parts.len(),
        sampled_track_count: clip.map_or(0, |clip| clip.tracks.len()),
    })
}

fn prepared_part_local_content_matrix(
    part: &PreparedFigurePart,
    track: Option<&Vec<PreparedFigureClipKey>>,
    local_time: f32,
    rotation_delta: Vec3,
) -> Mat4 {
    let sampled = track
        .filter(|keys| !keys.is_empty())
        .map(|keys| sample_prepared_track(part, keys, local_time));
    let (translation, mut rotation, scale) = sampled.unwrap_or_else(|| {
        (
            Vec3::ZERO,
            mirrored_source_rotation(Vec3::from_array(part.source_base_rotation_radians)),
            Vec3::ONE,
        )
    });
    rotation *= mirrored_source_rotation(rotation_delta);
    let base_position = Vec3::from_array(part.source_base_position);
    let source_position = base_position + translation;
    let engine_position = mirror_source_vector(source_position);
    let engine_pivot = mirror_source_vector(Vec3::from_array(part.source_pivot));
    Mat4::from_scale_rotation_translation(scale, rotation, engine_position)
        * Mat4::from_translation(-engine_pivot)
}

fn sample_prepared_track(
    part: &PreparedFigurePart,
    keys: &[PreparedFigureClipKey],
    local_time: f32,
) -> (Vec3, Quat, Vec3) {
    let translation = prepared_channel_span(keys, local_time, |key| key.translation)
        .map_or(Vec3::ZERO, |(left, right, alpha)| left.lerp(right, alpha));
    let base_rotation = Vec3::from_array(part.source_base_rotation_radians);
    let rotation = prepared_channel_span(keys, local_time, |key| key.rotation_radians).map_or_else(
        || mirrored_source_rotation(base_rotation),
        |(left, right, alpha)| {
            shortest_path_slerp(
                mirrored_source_rotation(base_rotation + left),
                mirrored_source_rotation(base_rotation + right),
                alpha,
            )
        },
    );
    let scale = prepared_channel_span(keys, local_time, |key| key.scale)
        .map_or(Vec3::ONE, |(left, right, alpha)| left.lerp(right, alpha));
    (translation, rotation, scale)
}

fn prepared_channel_span(
    keys: &[PreparedFigureClipKey],
    local_time: f32,
    channel: impl Fn(&PreparedFigureClipKey) -> Option<[f32; 3]>,
) -> Option<(Vec3, Vec3, f32)> {
    let mut keyed = keys
        .iter()
        .filter_map(|key| channel(key).map(|value| (key.time_seconds, Vec3::from_array(value))));
    let first = keyed.next()?;
    let mut left = first;
    let mut right = first;
    for current in keyed {
        if local_time < current.0 {
            right = current;
            break;
        }
        left = current;
        right = current;
    }
    let alpha = if (right.0 - left.0).abs() <= f32::EPSILON {
        0.0
    } else {
        ((local_time - left.0) / (right.0 - left.0)).clamp(0.0, 1.0)
    };
    Some((left.1, right.1, alpha))
}

fn shortest_path_slerp(left: Quat, right: Quat, alpha: f32) -> Quat {
    let right = if left.dot(right) < 0.0 { -right } else { right };
    left.slerp(right, alpha).normalize()
}

fn mirrored_source_rotation(source_euler_radians: Vec3) -> Quat {
    let source = Mat3::from_quat(Quat::from_euler(
        EulerRot::XYZ,
        source_euler_radians.x,
        source_euler_radians.y,
        source_euler_radians.z,
    ));
    let mirror = Mat3::from_diagonal(Vec3::new(1.0, 1.0, -1.0));
    Quat::from_mat3(&(mirror * source * mirror)).normalize()
}

fn mirror_source_vector(value: Vec3) -> Vec3 {
    Vec3::new(value.x, value.y, -value.z)
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
    let evaluation_order = part_evaluation_order(&raw_parts)?;
    let raw_bounds = raw_figure_bounds(asset, &raw_parts, &content_matrices)?;
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
            primitive_kind: part.primitive_kind,
            source_base_position: part.base_position.to_array(),
            source_base_rotation_radians: part.base_rotation.to_array(),
            source_pivot: part.pivot.to_array(),
            rest_matrix: (normalization * mirror * *content * mirror).to_cols_array_2d(),
        })
        .collect::<Vec<_>>();
    let clips = prepare_clips(asset, &part_names)?;

    let (atlas, atlas_regions) = build_atlas(asset)?;
    let materials = material_colors(asset)?;
    let mut vertices = Vec::with_capacity(asset.parts.len() * 24);
    let mut indices = Vec::with_capacity(asset.parts.len() * 36);
    let mut draw_ranges = Vec::with_capacity(asset.parts.len() * 6);
    for (part_index, part) in asset.parts.iter().enumerate() {
        append_prepared_cuboid(
            asset,
            part,
            raw_parts[part_index].prepared_size,
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
    let prepared_cpu_bytes = vertices.len() * PREPARED_VERTEX_BYTE_LEN
        + indices.len() * std::mem::size_of::<u16>()
        + atlas.rgba.len()
        + parts.len() * std::mem::size_of::<PreparedFigurePart>()
        + evaluation_order.len() * std::mem::size_of::<u16>()
        + prepared_clip_bytes(&clips);
    let diagnostics = PreparedFigureDiagnostics {
        compiler_id: PREPARED_FIGURE_COMPILER_ID,
        semantic_crc32,
        part_count: parts.len(),
        vertex_count: vertices.len(),
        index_count: indices.len(),
        draw_range_count: draw_ranges.len(),
        box_primitive_count: raw_parts
            .iter()
            .filter(|part| part.primitive_kind == PreparedFigurePrimitiveKind::Box)
            .count(),
        sphere_cuboid_proxy_count: raw_parts
            .iter()
            .filter(|part| part.primitive_kind == PreparedFigurePrimitiveKind::SphereCuboidProxy)
            .count(),
        capsule_cuboid_proxy_count: raw_parts
            .iter()
            .filter(|part| part.primitive_kind == PreparedFigurePrimitiveKind::CapsuleCuboidProxy)
            .count(),
        cylinder_cuboid_proxy_count: raw_parts
            .iter()
            .filter(|part| part.primitive_kind == PreparedFigurePrimitiveKind::CylinderCuboidProxy)
            .count(),
        atlas_bytes: atlas.rgba.len(),
        prepared_cpu_bytes,
    };

    Ok(PreparedFigure {
        name: asset.name.clone(),
        vertices,
        indices,
        parts,
        evaluation_order,
        draw_ranges,
        atlas,
        clips,
        normalization_matrix: normalization.to_cols_array_2d(),
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
        if let Some(locomotion) = &clip.locomotion {
            if !locomotion.cycle_distance.is_finite() || locomotion.cycle_distance <= 0.0 {
                return Err(FigurePrepareError::new(format!(
                    "figure '{}' clip '{}' locomotion cycleDistance must be positive",
                    asset.name, clip_name
                )));
            }
            if let Some(speed) = locomotion.speed
                && (!speed.is_finite() || speed <= 0.0)
            {
                return Err(FigurePrepareError::new(format!(
                    "figure '{}' clip '{}' locomotion speed must be positive",
                    asset.name, clip_name
                )));
            }
            if let Some(direction) = locomotion.direction {
                let direction = Vec3::from_array(direction);
                if !direction.is_finite() || direction.length_squared() <= f32::EPSILON {
                    return Err(FigurePrepareError::new(format!(
                        "figure '{}' clip '{}' locomotion direction must be finite and nonzero",
                        asset.name, clip_name
                    )));
                }
            }
            for contact in &locomotion.contacts {
                if !part_names.contains_key(contact.part.as_str()) {
                    return Err(FigurePrepareError::new(format!(
                        "figure '{}' clip '{}' contact references unknown part '{}'",
                        asset.name, clip_name, contact.part
                    )));
                }
                if !contact.phase_start.is_finite()
                    || !contact.phase_end.is_finite()
                    || !contact.stance_ratio.is_finite()
                    || !(0.0..=1.0).contains(&contact.stance_ratio)
                {
                    return Err(FigurePrepareError::new(format!(
                        "figure '{}' clip '{}' contact '{}' has invalid phase metadata",
                        asset.name, clip_name, contact.part
                    )));
                }
            }
        }
    }
    Ok(())
}

fn prepare_clips(
    asset: &FigureAsset,
    part_names: &HashMap<&str, usize>,
) -> Result<BTreeMap<String, PreparedFigureClip>, FigurePrepareError> {
    let mut prepared = BTreeMap::new();
    for (clip_name, clip) in &asset.clips {
        let mut duration_seconds = 0.0_f32;
        let mut tracks = BTreeMap::<u16, Vec<PreparedFigureClipKey>>::new();
        for key in &clip.keys {
            let part_id = *part_names.get(key.0.as_str()).ok_or_else(|| {
                FigurePrepareError::new(format!(
                    "figure '{}' clip '{}' references unknown part '{}'",
                    asset.name, clip_name, key.0
                ))
            })? as u16;
            duration_seconds = duration_seconds.max(key.1);
            tracks
                .entry(part_id)
                .or_default()
                .push(PreparedFigureClipKey {
                    time_seconds: key.1,
                    translation: key.2.at,
                    rotation_radians: key
                        .2
                        .rot
                        .map(|rotation| rotation.map(|component| component.to_radians())),
                    scale: key.2.scale,
                });
        }
        for keys in tracks.values_mut() {
            keys.sort_by(|left, right| left.time_seconds.total_cmp(&right.time_seconds));
        }
        let locomotion = clip
            .locomotion
            .as_ref()
            .map(|locomotion| {
                let contacts = locomotion
                    .contacts
                    .iter()
                    .map(|contact| {
                        let part_id = *part_names.get(contact.part.as_str()).ok_or_else(|| {
                            FigurePrepareError::new(format!(
                                "figure '{}' clip '{}' contact references unknown part '{}'",
                                asset.name, clip_name, contact.part
                            ))
                        })? as u16;
                        Ok(PreparedFigureClipContact {
                            part_id,
                            phase_start: contact.phase_start,
                            phase_end: contact.phase_end,
                            role: contact.role.clone(),
                            stance_ratio: contact.stance_ratio,
                        })
                    })
                    .collect::<Result<Vec<_>, FigurePrepareError>>()?;
                Ok(PreparedFigureClipLocomotion {
                    kind: locomotion.kind.clone(),
                    cycle_distance: locomotion.cycle_distance,
                    contacts,
                    direction: locomotion.direction,
                    speed: locomotion.speed,
                    units: locomotion.units.clone(),
                })
            })
            .transpose()?;
        prepared.insert(
            clip_name.clone(),
            PreparedFigureClip {
                duration_seconds,
                source_fps: clip.fps,
                looped: clip.r#loop,
                tracks,
                locomotion,
            },
        );
    }
    Ok(prepared)
}

fn prepared_clip_bytes(clips: &BTreeMap<String, PreparedFigureClip>) -> usize {
    clips
        .iter()
        .map(|(name, clip)| {
            name.len()
                + std::mem::size_of::<PreparedFigureClip>()
                + clip
                    .tracks
                    .values()
                    .map(|keys| keys.len() * std::mem::size_of::<PreparedFigureClipKey>())
                    .sum::<usize>()
                + clip.locomotion.as_ref().map_or(0, |locomotion| {
                    locomotion.kind.len()
                        + locomotion.units.as_ref().map_or(0, String::len)
                        + locomotion.contacts.len()
                            * std::mem::size_of::<PreparedFigureClipContact>()
                })
        })
        .sum()
}

#[derive(Clone, Debug)]
struct RawPart {
    name: String,
    parent: Option<usize>,
    base_position: Vec3,
    base_rotation: Vec3,
    pivot: Vec3,
    first_person_visible: bool,
    primitive_kind: PreparedFigurePrimitiveKind,
    prepared_size: Vec3,
}

fn build_raw_parts(
    asset: &FigureAsset,
    part_names: &HashMap<&str, usize>,
) -> Result<Vec<RawPart>, FigurePrepareError> {
    let mut parts = Vec::with_capacity(asset.parts.len());
    for part in &asset.parts {
        let (primitive_kind, prepared_size) = prepared_primitive_size(asset, part)?;
        if primitive_kind.is_cuboid_proxy() && part.texture.is_some() {
            return Err(FigurePrepareError::new(format!(
                "figure '{}' part '{}' applies a texture to {}; cuboid-proxy non-box primitives support solid materials only",
                asset.name,
                part.name,
                primitive_kind.source_kind()
            )));
        }
        if primitive_kind.is_cuboid_proxy() && part.primitive.faces.is_some() {
            return Err(FigurePrepareError::new(format!(
                "figure '{}' part '{}' declares box faces on {}; cuboid-proxy non-box primitives do not support face overrides",
                asset.name,
                part.name,
                primitive_kind.source_kind()
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
            primitive_kind,
            prepared_size,
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

fn prepared_primitive_size(
    asset: &FigureAsset,
    part: &FigurePart,
) -> Result<(PreparedFigurePrimitiveKind, Vec3), FigurePrepareError> {
    let (kind, size) = match part.primitive.kind.as_str() {
        "box" => (
            PreparedFigurePrimitiveKind::Box,
            finite_positive_vec3(part.primitive.size, asset, part, "size")?,
        ),
        "sphere" => {
            let radius = finite_positive_scalar(part.primitive.radius, asset, part, "radius")?;
            (
                PreparedFigurePrimitiveKind::SphereCuboidProxy,
                Vec3::splat(radius * 2.0),
            )
        }
        "capsule" => {
            let radius = finite_positive_scalar(part.primitive.radius, asset, part, "radius")?;
            let length = finite_positive_scalar(part.primitive.length, asset, part, "length")?;
            (
                PreparedFigurePrimitiveKind::CapsuleCuboidProxy,
                Vec3::new(radius * 2.0, length + radius * 2.0, radius * 2.0),
            )
        }
        "cylinder" => {
            let radius_top =
                finite_positive_scalar(part.primitive.radius_top, asset, part, "radiusTop")?;
            let radius_bottom =
                finite_positive_scalar(part.primitive.radius_bottom, asset, part, "radiusBottom")?;
            let length = finite_positive_scalar(part.primitive.length, asset, part, "length")?;
            let diameter = radius_top.max(radius_bottom) * 2.0;
            (
                PreparedFigurePrimitiveKind::CylinderCuboidProxy,
                Vec3::new(diameter, length, diameter),
            )
        }
        other => {
            return Err(FigurePrepareError::new(format!(
                "figure '{}' part '{}' uses unsupported prepared primitive '{}'",
                asset.name, part.name, other
            )));
        }
    };
    if size.min_element() <= 0.0 || !size.is_finite() {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' {} part '{}' has invalid prepared cuboid size {:?}",
            asset.name,
            kind.source_kind(),
            part.name,
            size
        )));
    }
    Ok((kind, size))
}

fn finite_positive_scalar(
    value: Option<f32>,
    asset: &FigureAsset,
    part: &FigurePart,
    field: &str,
) -> Result<f32, FigurePrepareError> {
    let value = value.ok_or_else(|| {
        FigurePrepareError::new(format!(
            "figure '{}' part '{}' is missing {}",
            asset.name, part.name, field
        ))
    })?;
    if !value.is_finite() || value <= 0.0 {
        return Err(FigurePrepareError::new(format!(
            "figure '{}' part '{}' has non-positive or non-finite {}",
            asset.name, part.name, field
        )));
    }
    Ok(value)
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

fn part_evaluation_order(parts: &[RawPart]) -> Result<Vec<u16>, FigurePrepareError> {
    let mut order = Vec::with_capacity(parts.len());
    let mut complete = vec![false; parts.len()];
    let mut visiting = vec![false; parts.len()];
    for index in 0..parts.len() {
        append_part_evaluation_order(index, parts, &mut complete, &mut visiting, &mut order)?;
    }
    Ok(order)
}

fn append_part_evaluation_order(
    index: usize,
    parts: &[RawPart],
    complete: &mut [bool],
    visiting: &mut [bool],
    order: &mut Vec<u16>,
) -> Result<(), FigurePrepareError> {
    if complete[index] {
        return Ok(());
    }
    if visiting[index] {
        return Err(FigurePrepareError::new(format!(
            "figure has a parent cycle at part '{}'",
            parts[index].name
        )));
    }
    visiting[index] = true;
    if let Some(parent) = parts[index].parent {
        append_part_evaluation_order(parent, parts, complete, visiting, order)?;
    }
    visiting[index] = false;
    complete[index] = true;
    order.push(index as u16);
    Ok(())
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
    raw_parts: &[RawPart],
    content_matrices: &[Mat4],
) -> Result<Bounds, FigurePrepareError> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for (part, matrix) in raw_parts.iter().zip(content_matrices) {
        let size = part.prepared_size;
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
fn append_prepared_cuboid(
    asset: &FigureAsset,
    part: &FigurePart,
    size: Vec3,
    part_id: u16,
    materials: &HashMap<String, [f32; 4]>,
    atlas_regions: &BTreeMap<String, AtlasRegion>,
    atlas_width: u32,
    atlas_height: u32,
    vertices: &mut Vec<PreparedFigureVertex>,
    indices: &mut Vec<u16>,
    draw_ranges: &mut Vec<PreparedFigureDrawRange>,
) -> Result<(), FigurePrepareError> {
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
    use crate::{
        AssetPackId, AssetPackOrigin, AssetResolutionOrigin, AssetSourceChain, MemoryAssetSource,
        ProvenanceTrackingAssetSource,
    };

    const PLAYER_FIGURE_JSON: &str =
        include_str!("../../../../assets/mclone/figures/player.figure.json");
    const CHICKEN_FIGURE_JSON: &str =
        include_str!("../../../../assets/mclone/figures/chicken.figure.json");
    const UPRIGHT_BEAR_FIGURE_JSON: &str =
        include_str!("../../../../assets/mclone/figures/upright_bear.figure.json");

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
    fn recomposed_rest_pose_matches_prepared_rest_matrices() {
        let asset: FigureAsset = serde_json::from_str(PLAYER_FIGURE_JSON).unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();
        let mut palette = Vec::new();

        evaluate_prepared_figure_rest_pose_into(&prepared, &mut palette).unwrap();

        assert_eq!(palette.len(), prepared.parts.len());
        for (part, evaluated) in prepared.parts.iter().zip(&palette) {
            assert_matrix_close(part.rest_matrix, *evaluated, 2.0e-5);
        }
        for &part_id in &prepared.evaluation_order {
            if let Some(parent) = prepared.parts[usize::from(part_id)].parent {
                let parent_position = prepared
                    .evaluation_order
                    .iter()
                    .position(|candidate| *candidate == parent)
                    .unwrap();
                let part_position = prepared
                    .evaluation_order
                    .iter()
                    .position(|candidate| *candidate == part_id)
                    .unwrap();
                assert!(parent_position < part_position);
            }
        }
    }

    #[test]
    fn player_walk_evaluates_continuously_at_arbitrary_presentation_times() {
        let asset: FigureAsset = serde_json::from_str(PLAYER_FIGURE_JSON).unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();
        let walk = prepared.clips.get("walk").unwrap();
        assert!((walk.duration_seconds - 0.9).abs() < 1.0e-6);
        assert_eq!(walk.source_fps, Some(12.0));
        assert_eq!(walk.tracks.len(), 6);
        assert_eq!(walk.locomotion.as_ref().unwrap().contacts.len(), 2);

        let mut first = Vec::with_capacity(prepared.parts.len());
        let first_stats =
            evaluate_prepared_figure_clip_into(&prepared, "walk", 0.123, &mut first).unwrap();
        let capacity = first.capacity();
        let mut adjacent = Vec::with_capacity(prepared.parts.len());
        let adjacent_stats = evaluate_prepared_figure_clip_into(
            &prepared,
            "walk",
            0.123 + 1.0 / 500.0,
            &mut adjacent,
        )
        .unwrap();
        assert!((first_stats.duration_seconds - 0.9).abs() < 1.0e-6);
        assert!((first_stats.local_time_seconds - 0.123).abs() < 1.0e-12);
        assert_eq!(first_stats.evaluated_part_count, 12);
        assert_eq!(first_stats.sampled_track_count, 6);
        assert!(adjacent_stats.local_time_seconds > first_stats.local_time_seconds);
        assert_ne!(first, adjacent);

        let first_copy = first.clone();
        evaluate_prepared_figure_clip_into(&prepared, "walk", 0.123, &mut first).unwrap();
        assert_eq!(first, first_copy);
        assert_eq!(first.capacity(), capacity);

        let mut wrapped = Vec::new();
        evaluate_prepared_figure_clip_into(&prepared, "walk", 1.023, &mut wrapped).unwrap();
        for (expected, actual) in first_copy.into_iter().zip(wrapped) {
            assert_matrix_close(expected, actual, 2.0e-5);
        }
    }

    #[test]
    fn actor_local_rotation_overrides_compose_with_clip_pose() {
        let asset: FigureAsset = serde_json::from_str(CHICKEN_FIGURE_JSON).unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();
        let wing = prepared
            .parts
            .iter()
            .position(|part| part.name == "wing_l")
            .unwrap() as u16;
        let mut clip_only = Vec::new();
        let mut overridden = Vec::new();

        evaluate_prepared_figure_clip_into(&prepared, "walk", 0.173, &mut clip_only).unwrap();
        evaluate_prepared_figure_clip_with_part_rotation_overrides_into(
            &prepared,
            "walk",
            0.173,
            &[PreparedFigurePartRotationOverride {
                part_id: wing,
                rotation_delta_radians: [0.0, 0.0, 0.45],
            }],
            &mut overridden,
        )
        .unwrap();

        assert_eq!(overridden.len(), prepared.parts.len());
        assert_ne!(overridden[usize::from(wing)], clip_only[usize::from(wing)]);
        assert!(
            overridden
                .iter()
                .flatten()
                .flatten()
                .all(|value| value.is_finite())
        );
    }

    #[test]
    fn actor_local_rotation_overrides_reject_invalid_part_or_rotation() {
        let asset: FigureAsset = serde_json::from_str(CHICKEN_FIGURE_JSON).unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();
        let mut palette = Vec::new();

        for rotation_override in [
            PreparedFigurePartRotationOverride {
                part_id: prepared.parts.len() as u16,
                rotation_delta_radians: [0.0; 3],
            },
            PreparedFigurePartRotationOverride {
                part_id: 0,
                rotation_delta_radians: [f32::NAN, 0.0, 0.0],
            },
        ] {
            let error = evaluate_prepared_figure_clip_with_part_rotation_overrides_into(
                &prepared,
                "walk",
                0.0,
                &[rotation_override],
                &mut palette,
            )
            .unwrap_err();
            assert!(error.to_string().contains("invalid part rotation override"));
        }
    }

    #[test]
    fn pose_evaluator_clamps_and_uses_shortest_quaternion_path() {
        let asset: FigureAsset = serde_json::from_str(
            r##"{
              "schemaVersion": 1,
              "name": "shortest_rotation",
              "materials": {},
              "textures": {},
              "parts": [
                { "name": "root", "primitive": { "kind": "box", "size": [1, 1, 1] } }
              ],
              "clips": {
                "turn": {
                  "loop": false,
                  "keys": [
                    ["root", 0, { "rot": [0, 170, 0], "at": [0.25, 0, 0] }],
                    ["root", 1, { "rot": [0, -170, 0] }]
                  ]
                }
              }
            }"##,
        )
        .unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();
        let mut before = Vec::new();
        let mut start = Vec::new();
        let mut midpoint = Vec::new();
        let mut end = Vec::new();
        let mut after = Vec::new();
        evaluate_prepared_figure_clip_into(&prepared, "turn", -10.0, &mut before).unwrap();
        evaluate_prepared_figure_clip_into(&prepared, "turn", 0.0, &mut start).unwrap();
        evaluate_prepared_figure_clip_into(&prepared, "turn", 0.5, &mut midpoint).unwrap();
        evaluate_prepared_figure_clip_into(&prepared, "turn", 1.0, &mut end).unwrap();
        evaluate_prepared_figure_clip_into(&prepared, "turn", 10.0, &mut after).unwrap();
        assert_eq!(before, start);
        assert_eq!(end, after);

        let midpoint = Mat4::from_cols_array_2d(&midpoint[0]);
        let rotated_x = midpoint.transform_vector3(Vec3::X).normalize();
        assert!(rotated_x.dot(-Vec3::X) > 0.999);
        let start_translation = Mat4::from_cols_array_2d(&start[0]).w_axis.x;
        let end_translation = Mat4::from_cols_array_2d(&end[0]).w_axis.x;
        assert!((start_translation - end_translation).abs() < 1.0e-6);
    }

    #[test]
    fn pose_evaluator_interpolates_scale_before_composing_children() {
        let asset: FigureAsset = serde_json::from_str(
            r##"{
              "schemaVersion": 1,
              "name": "scaled_hierarchy",
              "materials": {},
              "textures": {},
              "parts": [
                { "name": "child", "parent": "root", "at": [1, 0, 0],
                  "primitive": { "kind": "box", "size": [0.5, 0.5, 0.5] } },
                { "name": "root", "primitive": { "kind": "box", "size": [1, 1, 1] } }
              ],
              "clips": {
                "grow": {
                  "loop": true,
                  "keys": [
                    ["root", 0, { "scale": [1, 1, 1] }],
                    ["root", 1, { "scale": [2, 2, 2] }]
                  ]
                }
              }
            }"##,
        )
        .unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();
        let root = prepared
            .parts
            .iter()
            .position(|part| part.name == "root")
            .unwrap();
        let child = prepared
            .parts
            .iter()
            .position(|part| part.name == "child")
            .unwrap();
        let mut start = Vec::new();
        let mut midpoint = Vec::new();
        let mut wrapped = Vec::new();
        evaluate_prepared_figure_clip_into(&prepared, "grow", 0.0, &mut start).unwrap();
        evaluate_prepared_figure_clip_into(&prepared, "grow", 0.5, &mut midpoint).unwrap();
        evaluate_prepared_figure_clip_into(&prepared, "grow", 1.5, &mut wrapped).unwrap();

        let start_root = Mat4::from_cols_array_2d(&start[root]);
        let midpoint_root = Mat4::from_cols_array_2d(&midpoint[root]);
        let root_scale_ratio =
            midpoint_root.x_axis.truncate().length() / start_root.x_axis.truncate().length();
        assert!((root_scale_ratio - 1.5).abs() < 1.0e-5);
        assert_ne!(start[child], midpoint[child]);
        assert_matrix_close(midpoint[child], wrapped[child], 2.0e-5);
    }

    #[test]
    fn pose_evaluator_rejects_unknown_clip_and_non_finite_time() {
        let asset: FigureAsset = serde_json::from_str(PLAYER_FIGURE_JSON).unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();
        let mut palette = Vec::new();
        assert!(
            evaluate_prepared_figure_clip_into(&prepared, "missing", 0.0, &mut palette)
                .unwrap_err()
                .to_string()
                .contains("no clip")
        );
        assert!(
            evaluate_prepared_figure_clip_into(&prepared, "walk", f64::NAN, &mut palette)
                .unwrap_err()
                .to_string()
                .contains("finite")
        );
    }

    #[test]
    fn rejects_invalid_prepared_locomotion_metadata() {
        let mut asset: FigureAsset = serde_json::from_str(PLAYER_FIGURE_JSON).unwrap();
        asset
            .clips
            .get_mut("walk")
            .unwrap()
            .locomotion
            .as_mut()
            .unwrap()
            .speed = Some(-1.0);
        assert!(
            prepare_figure_asset(&asset)
                .unwrap_err()
                .to_string()
                .contains("speed must be positive")
        );
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
    fn asset_source_replacement_updates_exact_semantic_identity() {
        let path = crate::default_player_figure_path();
        let mut fallback = MemoryAssetSource::new();
        fallback.insert_text(path.clone(), PLAYER_FIGURE_JSON);
        let original = load_prepared_figure(&fallback, &path).unwrap();
        assert_eq!(
            original.diagnostics.semantic_crc32,
            Some(crc32fast::hash(PLAYER_FIGURE_JSON.as_bytes()))
        );

        let replacement_bytes = format!("{PLAYER_FIGURE_JSON}\n");
        let replacement_id = AssetPackId::new("figure-replacement");
        let mut replacement_source = MemoryAssetSource::new();
        replacement_source.insert_text(path.clone(), replacement_bytes.clone());
        let mut chain = AssetSourceChain::new();
        chain.push_named(
            replacement_id.clone(),
            AssetPackOrigin::FirstParty,
            replacement_source,
        );
        chain.push_named(
            AssetPackId::new("figure-fallback"),
            AssetPackOrigin::Generated,
            fallback,
        );
        let tracker = ProvenanceTrackingAssetSource::new(&chain);
        let replacement = load_prepared_figure(&tracker, &path).unwrap();
        assert_eq!(
            replacement.diagnostics.semantic_crc32,
            Some(crc32fast::hash(replacement_bytes.as_bytes()))
        );
        assert_eq!(
            tracker.resolved_origin(&path),
            Some(AssetResolutionOrigin::named(
                replacement_id,
                AssetPackOrigin::FirstParty
            ))
        );
        assert_ne!(
            original.diagnostics.semantic_crc32,
            replacement.diagnostics.semantic_crc32
        );
        assert_eq!(original.vertices, replacement.vertices);
        assert_eq!(original.indices, replacement.indices);
        assert_eq!(original.atlas, replacement.atlas);
    }

    #[test]
    fn asset_source_rejects_missing_and_malformed_semantic_input() {
        let path = crate::default_player_figure_path();
        let mut source = MemoryAssetSource::new();
        assert!(matches!(
            load_prepared_figure(&source, &path),
            Err(AssetError::MissingAsset(missing)) if missing == path
        ));

        source.insert_text(path.clone(), "{");
        assert!(matches!(
            load_prepared_figure(&source, &path),
            Err(AssetError::Json { path: malformed, .. }) if malformed == path
        ));
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
    fn prepares_curved_primitives_as_explicit_cuboid_proxies() {
        let asset: FigureAsset = serde_json::from_str(
            r##"{
              "schemaVersion": 1,
              "name": "proxy-shapes",
              "materials": {},
              "textures": {},
              "parts": [
                { "name": "sphere", "primitive": { "kind": "sphere", "radius": 1 } },
                { "name": "capsule", "at": [3, 0, 0],
                  "primitive": { "kind": "capsule", "radius": 0.5, "length": 2 } },
                { "name": "cylinder", "at": [6, 0, 0],
                  "primitive": {
                    "kind": "cylinder", "radiusTop": 0.25,
                    "radiusBottom": 0.75, "length": 3
                  } }
              ],
              "clips": {}
            }"##,
        )
        .unwrap();
        let prepared = prepare_figure_asset(&asset).unwrap();

        assert_eq!(prepared.vertices.len(), 3 * 24);
        assert_eq!(prepared.indices.len(), 3 * 36);
        assert_eq!(prepared.diagnostics.box_primitive_count, 0);
        assert_eq!(prepared.diagnostics.sphere_cuboid_proxy_count, 1);
        assert_eq!(prepared.diagnostics.capsule_cuboid_proxy_count, 1);
        assert_eq!(prepared.diagnostics.cylinder_cuboid_proxy_count, 1);
        assert_eq!(
            prepared
                .parts
                .iter()
                .map(|part| part.primitive_kind)
                .collect::<Vec<_>>(),
            vec![
                PreparedFigurePrimitiveKind::SphereCuboidProxy,
                PreparedFigurePrimitiveKind::CapsuleCuboidProxy,
                PreparedFigurePrimitiveKind::CylinderCuboidProxy,
            ]
        );
        assert_eq!(prepared.vertices[0].position[0].abs(), 1.0);
        assert_eq!(prepared.vertices[24].position[0].abs(), 0.5);
        assert_eq!(prepared.vertices[24].position[1].abs(), 1.5);
        assert_eq!(prepared.vertices[48].position[0].abs(), 0.75);
        assert_eq!(prepared.vertices[48].position[1].abs(), 1.5);
    }

    #[test]
    fn rejects_textures_and_face_overrides_on_non_box_primitives() {
        for part in [
            serde_json::json!({
                "name": "textured",
                "texture": "face",
                "primitive": { "kind": "sphere", "radius": 1 }
            }),
            serde_json::json!({
                "name": "faces",
                "primitive": {
                    "kind": "capsule",
                    "radius": 1,
                    "length": 1,
                    "faces": { "north": { "material": "white" } }
                }
            }),
        ] {
            let asset: FigureAsset = serde_json::from_value(serde_json::json!({
                "schemaVersion": 1,
                "name": "invalid-curved-texture",
                "materials": { "white": { "color": "#ffffff" } },
                "textures": {
                    "face": { "palette": { "x": "#ffffff" }, "pixels": ["x"] }
                },
                "parts": [part],
                "clips": {}
            }))
            .unwrap();
            let error = prepare_figure_asset(&asset).unwrap_err().to_string();
            assert!(error.contains("non-box primitives"));
        }
    }

    #[test]
    fn promoted_animals_prepare_deterministically_with_proxy_accounting() {
        let chicken: FigureAsset = serde_json::from_str(CHICKEN_FIGURE_JSON).unwrap();
        let first = prepare_figure_asset(&chicken).unwrap();
        let second = prepare_figure_asset(&chicken).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.parts.len(), 21);
        assert_eq!(first.vertices.len(), 21 * 24);
        assert_eq!(first.indices.len(), 21 * 36);
        assert_eq!(first.diagnostics.box_primitive_count, 6);
        assert_eq!(first.diagnostics.sphere_cuboid_proxy_count, 11);
        assert_eq!(first.diagnostics.capsule_cuboid_proxy_count, 3);
        assert_eq!(first.diagnostics.cylinder_cuboid_proxy_count, 1);
        assert!(first.clips.contains_key("walk"));

        let bear: FigureAsset = serde_json::from_str(UPRIGHT_BEAR_FIGURE_JSON).unwrap();
        let prepared_bear = prepare_figure_asset(&bear).unwrap();
        assert_eq!(prepared_bear.parts.len(), 15);
        assert_eq!(prepared_bear.diagnostics.box_primitive_count, 15);
        assert_eq!(prepared_bear.diagnostics.sphere_cuboid_proxy_count, 0);
        assert_eq!(
            prepared_bear,
            prepare_figure_asset(&bear).expect("repeat bear preparation")
        );
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

    fn assert_matrix_close(left: [[f32; 4]; 4], right: [[f32; 4]; 4], tolerance: f32) {
        for (left, right) in left.into_iter().flatten().zip(right.into_iter().flatten()) {
            assert!(
                (left - right).abs() <= tolerance,
                "matrix component mismatch: {left} vs {right}"
            );
        }
    }
}
