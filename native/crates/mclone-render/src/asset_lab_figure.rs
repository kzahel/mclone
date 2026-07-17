use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_assets::{
    ActorFigureId, FIRST_PARTY_ACTOR_FIGURE_IDS, FigureAsciiTexture, FigureAsset, FigureClip,
    FigureClipLocomotion, FigureClipTransform, FigurePart, PreparedFigure, actor_figure_path,
    load_figure_asset, prepare_figure_asset,
};

const TEXTURE_OVERLAY_DEPTH: f32 = 0.004;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledFigure {
    pub(crate) cuboids: Vec<CompiledCuboid>,
    pub(crate) overlay_cuboids: Vec<CompiledOverlayCuboid>,
    pub(crate) parts: Vec<CompiledFigurePart>,
    pub(crate) clips: BTreeMap<String, CompiledFigureClip>,
    pub(crate) normalization_origin: Vec3,
    pub(crate) inv_height: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActorFigureSet {
    figures: BTreeMap<ActorFigureId, CompiledFigure>,
    prepared_figures: BTreeMap<ActorFigureId, PreparedFigure>,
}

impl ActorFigureSet {
    pub fn new(figures: impl IntoIterator<Item = (ActorFigureId, CompiledFigure)>) -> Self {
        Self {
            figures: figures.into_iter().collect(),
            prepared_figures: BTreeMap::new(),
        }
    }

    fn with_prepared(
        figures: impl IntoIterator<Item = (ActorFigureId, CompiledFigure)>,
        prepared_figures: impl IntoIterator<Item = (ActorFigureId, PreparedFigure)>,
    ) -> Self {
        Self {
            figures: figures.into_iter().collect(),
            prepared_figures: prepared_figures.into_iter().collect(),
        }
    }

    pub fn get(&self, id: ActorFigureId) -> Option<&CompiledFigure> {
        self.figures.get(&id)
    }

    pub fn prepared(&self, id: ActorFigureId) -> Option<&PreparedFigure> {
        self.prepared_figures.get(&id)
    }

    pub(crate) fn prepared_figures(
        &self,
    ) -> impl Iterator<Item = (ActorFigureId, &PreparedFigure)> {
        self.prepared_figures
            .iter()
            .map(|(id, figure)| (*id, figure))
    }

    pub fn len(&self) -> usize {
        self.figures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.figures.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CompiledCuboid {
    pub min: Vec3,
    pub max: Vec3,
    pub face_colors: [[f32; 4]; 6],
    pub first_person_visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CompiledOverlayCuboid {
    pub min: Vec3,
    pub max: Vec3,
    pub color: [f32; 4],
    pub first_person_visible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompiledFigurePart {
    pub name: String,
    pub parent: Option<usize>,
    pub base_position: Vec3,
    pub base_rotation_radians: Vec3,
    pub pivot: Vec3,
    pub cuboids: Vec<CompiledLocalCuboid>,
    pub overlay_cuboids: Vec<CompiledLocalOverlayCuboid>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CompiledLocalCuboid {
    pub min: Vec3,
    pub max: Vec3,
    pub face_colors: [[f32; 4]; 6],
    pub first_person_visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CompiledLocalOverlayCuboid {
    pub min: Vec3,
    pub max: Vec3,
    pub color: [f32; 4],
    pub first_person_visible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompiledFigureClip {
    pub duration_seconds: f32,
    pub looped: bool,
    pub tracks: BTreeMap<usize, Vec<CompiledFigureKey>>,
    pub locomotion: Option<CompiledFigureLocomotion>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CompiledFigureKey {
    pub time_seconds: f32,
    pub transform: CompiledFigureTransform,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CompiledFigureTransform {
    pub at: Option<Vec3>,
    pub rot_radians: Option<Vec3>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompiledFigureLocomotion {
    pub kind: String,
    pub cycle_distance: f32,
    pub speed: Option<f32>,
    pub direction: Option<Vec3>,
    pub contacts: Vec<CompiledFigureContact>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompiledFigureContact {
    pub part_index: usize,
    pub phase_start: f32,
    pub phase_end: f32,
    pub role: Option<String>,
    pub stance_ratio: f32,
}

pub(crate) fn load_first_party_actor_figures(
    source: &impl mclone_assets::AssetSource,
) -> Result<ActorFigureSet> {
    let mut figures = Vec::new();
    let mut prepared_figures = Vec::new();
    for id in FIRST_PARTY_ACTOR_FIGURE_IDS {
        let path = actor_figure_path(id)
            .with_context(|| format!("unknown actor figure id {}", id.as_str()))?;
        let asset = load_figure_asset(source, &path)
            .with_context(|| format!("failed to load actor figure {} at {}", id.as_str(), path))?;
        figures.push((id, compile_figure_asset(&asset)?));
        match prepare_figure_asset(&asset) {
            Ok(prepared) => prepared_figures.push((id, prepared)),
            Err(error) => log::warn!(
                "prepared actor figure {} unavailable; legacy fallback remains active: {}",
                id.as_str(),
                error
            ),
        }
    }
    Ok(ActorFigureSet::with_prepared(figures, prepared_figures))
}

#[cfg(test)]
pub(crate) fn load_compiled_actor_figure(
    source: &impl mclone_assets::AssetSource,
    id: ActorFigureId,
) -> Result<CompiledFigure> {
    let path = actor_figure_path(id)
        .with_context(|| format!("unknown actor figure id {}", id.as_str()))?;
    let asset = load_figure_asset(source, &path)
        .with_context(|| format!("failed to load actor figure {} at {}", id.as_str(), path))?;
    compile_figure_asset(&asset)
}

pub(crate) fn compile_figure_asset(asset: &FigureAsset) -> Result<CompiledFigure> {
    if asset.schema_version != 1 {
        bail!(
            "unsupported asset-lab figure schema version {}",
            asset.schema_version
        );
    }
    if asset.parts.is_empty() {
        bail!("asset-lab figure '{}' has no parts", asset.name);
    }

    let materials = compile_materials(&asset)?;
    let part_names = part_name_map(&asset.parts)?;
    let mut origin_cache = vec![None; asset.parts.len()];
    let mut visiting = vec![false; asset.parts.len()];
    let mut first_person_visible_cache = vec![None; asset.parts.len()];
    let mut first_person_visible_visiting = vec![false; asset.parts.len()];
    let mut raw_cuboids = Vec::new();
    let mut raw_overlay_cuboids = Vec::new();
    let mut compiled_parts = Vec::with_capacity(asset.parts.len());

    for (part_index, part) in asset.parts.iter().enumerate() {
        let (size, face_overrides) = primitive_cuboid_size(asset, part)?;

        let origin = part_origin(
            part_index,
            &asset.parts,
            &part_names,
            &mut origin_cache,
            &mut visiting,
        )?;
        let first_person_visible = part_first_person_visible(
            part_index,
            &asset.parts,
            &part_names,
            &mut first_person_visible_cache,
            &mut first_person_visible_visiting,
        )?;
        let parent = part
            .parent
            .as_deref()
            .map(|parent_name| {
                part_names.get(parent_name).copied().with_context(|| {
                    format!("part '{}' has unknown parent '{}'", part.name, parent_name)
                })
            })
            .transpose()?;
        let local_at = Vec3::from_array(part.at.unwrap_or([0.0, 0.0, 0.0]));
        let pivot = Vec3::from_array(
            part.joint
                .as_ref()
                .and_then(|joint| joint.pivot)
                .unwrap_or([0.0, 0.0, 0.0]),
        );
        let base_rotation_radians = euler_degrees_to_radians(part.rot.unwrap_or([0.0, 0.0, 0.0]));
        let base_color = part
            .material
            .as_deref()
            .map(|name| material_color(&materials, name))
            .transpose()?
            .unwrap_or([0.84, 0.87, 0.89, 1.0]);
        let mut face_colors = shaded_faces(base_color);
        if let Some(face_overrides) = face_overrides {
            for (face_name, face) in face_overrides {
                let box_face = parse_box_face(face_name)?;
                if let Some(material_name) = face.material.as_deref() {
                    let color = material_color(&materials, material_name)?;
                    face_colors[box_face.actor_face_index()] =
                        shade_face(color, box_face.actor_face_index());
                }
            }
        }

        let half_size = size * 0.5;
        let mut local_cuboids = vec![CompiledLocalCuboid {
            min: -half_size,
            max: half_size,
            face_colors,
            first_person_visible,
        }];
        let mut local_overlay_cuboids = Vec::new();
        let (min, max) = lab_box_bounds_to_actor_local(origin - half_size, origin + half_size);
        raw_cuboids.push(CompiledCuboid {
            min,
            max,
            face_colors,
            first_person_visible,
        });

        if let Some(texture_name) = part.texture.as_deref() {
            for face in BoxFace::ALL {
                append_texture_overlay_cuboids(
                    &mut raw_overlay_cuboids,
                    origin,
                    size,
                    face,
                    &asset,
                    texture_name,
                    first_person_visible,
                )?;
                append_texture_overlay_cuboids(
                    &mut local_overlay_cuboids,
                    Vec3::ZERO,
                    size,
                    face,
                    &asset,
                    texture_name,
                    first_person_visible,
                )?;
            }
        }
        if let Some(face_overrides) = face_overrides {
            for (face_name, face) in face_overrides {
                if let Some(texture_name) = face.texture.as_deref() {
                    append_texture_overlay_cuboids(
                        &mut raw_overlay_cuboids,
                        origin,
                        size,
                        parse_box_face(face_name)?,
                        &asset,
                        texture_name,
                        first_person_visible,
                    )?;
                    append_texture_overlay_cuboids(
                        &mut local_overlay_cuboids,
                        Vec3::ZERO,
                        size,
                        parse_box_face(face_name)?,
                        &asset,
                        texture_name,
                        first_person_visible,
                    )?;
                }
            }
        }
        compiled_parts.push(CompiledFigurePart {
            name: part.name.clone(),
            parent,
            base_position: local_at + pivot,
            base_rotation_radians,
            pivot,
            cuboids: std::mem::take(&mut local_cuboids),
            overlay_cuboids: local_overlay_cuboids
                .into_iter()
                .map(|cuboid| {
                    let (min, max) = actor_local_bounds_to_lab(cuboid.min, cuboid.max);
                    CompiledLocalOverlayCuboid {
                        min,
                        max,
                        color: cuboid.color,
                        first_person_visible: cuboid.first_person_visible,
                    }
                })
                .collect(),
        });
    }

    if raw_cuboids.is_empty() {
        bail!("asset-lab figure '{}' compiled to no cuboids", asset.name);
    }
    let clips = compile_clips(asset, &part_names)?;
    normalize_figure(raw_cuboids, raw_overlay_cuboids, compiled_parts, clips)
}

fn primitive_cuboid_size<'a>(
    asset: &FigureAsset,
    part: &'a FigurePart,
) -> Result<(Vec3, Option<&'a HashMap<String, mclone_assets::FigureFace>>)> {
    let primitive = &part.primitive;
    let size = match primitive.kind.as_str() {
        "box" => {
            let size = Vec3::from_array(primitive.size.with_context(|| {
                format!(
                    "asset-lab figure '{}' box part '{}' is missing size",
                    asset.name, part.name
                )
            })?);
            require_positive_size(asset, part, size)?;
            return Ok((size, primitive.faces.as_ref()));
        }
        "sphere" => {
            let radius =
                required_positive_primitive_field(asset, part, "radius", primitive.radius)?;
            Vec3::splat(radius * 2.0)
        }
        "capsule" => {
            let radius =
                required_positive_primitive_field(asset, part, "radius", primitive.radius)?;
            let length =
                required_positive_primitive_field(asset, part, "length", primitive.length)?;
            Vec3::new(radius * 2.0, length + radius * 2.0, radius * 2.0)
        }
        "cylinder" => {
            let radius_top =
                required_positive_primitive_field(asset, part, "radiusTop", primitive.radius_top)?;
            let radius_bottom = required_positive_primitive_field(
                asset,
                part,
                "radiusBottom",
                primitive.radius_bottom,
            )?;
            let length =
                required_positive_primitive_field(asset, part, "length", primitive.length)?;
            let radius = radius_top.max(radius_bottom);
            Vec3::new(radius * 2.0, length, radius * 2.0)
        }
        _ => {
            bail!(
                "asset-lab figure '{}' part '{}' uses unsupported primitive kind '{}'",
                asset.name,
                part.name,
                primitive.kind
            );
        }
    };

    if primitive.faces.is_some() {
        bail!(
            "asset-lab figure '{}' part '{}' uses face overrides on non-box primitive '{}'",
            asset.name,
            part.name,
            primitive.kind
        );
    }
    Ok((size, None))
}

fn require_positive_size(asset: &FigureAsset, part: &FigurePart, size: Vec3) -> Result<()> {
    if size.x <= 0.0 || size.y <= 0.0 || size.z <= 0.0 {
        bail!(
            "asset-lab figure '{}' box part '{}' has non-positive size {:?}",
            asset.name,
            part.name,
            size
        );
    }
    Ok(())
}

fn required_positive_primitive_field(
    asset: &FigureAsset,
    part: &FigurePart,
    field: &str,
    value: Option<f32>,
) -> Result<f32> {
    let value = value.with_context(|| {
        format!(
            "asset-lab figure '{}' {} part '{}' is missing {}",
            asset.name, part.primitive.kind, part.name, field
        )
    })?;
    if value <= 0.0 {
        bail!(
            "asset-lab figure '{}' {} part '{}' has non-positive {} {}",
            asset.name,
            part.primitive.kind,
            part.name,
            field,
            value
        );
    }
    Ok(value)
}

fn compile_materials(asset: &FigureAsset) -> Result<HashMap<String, [f32; 4]>> {
    let mut materials = HashMap::with_capacity(asset.materials.len());
    for (name, material) in &asset.materials {
        materials.insert(
            name.clone(),
            parse_hex_color(&material.color).with_context(|| {
                format!(
                    "asset-lab figure '{}' material '{}' has invalid color '{}'",
                    asset.name, name, material.color
                )
            })?,
        );
    }
    Ok(materials)
}

fn part_name_map(parts: &[FigurePart]) -> Result<HashMap<String, usize>> {
    let mut names = HashMap::with_capacity(parts.len());
    for (index, part) in parts.iter().enumerate() {
        if names.insert(part.name.clone(), index).is_some() {
            bail!("asset-lab figure has duplicate part '{}'", part.name);
        }
    }
    Ok(names)
}

fn part_origin(
    index: usize,
    parts: &[FigurePart],
    part_names: &HashMap<String, usize>,
    origin_cache: &mut [Option<Vec3>],
    visiting: &mut [bool],
) -> Result<Vec3> {
    if let Some(origin) = origin_cache[index] {
        return Ok(origin);
    }
    if visiting[index] {
        bail!(
            "asset-lab figure has a parent cycle at part '{}'",
            parts[index].name
        );
    }
    visiting[index] = true;

    let local = Vec3::from_array(parts[index].at.unwrap_or([0.0, 0.0, 0.0]));
    let origin = if let Some(parent_name) = parts[index].parent.as_deref() {
        let parent_index = *part_names.get(parent_name).with_context(|| {
            format!(
                "part '{}' has unknown parent '{}'",
                parts[index].name, parent_name
            )
        })?;
        part_origin(parent_index, parts, part_names, origin_cache, visiting)? + local
    } else {
        local
    };

    visiting[index] = false;
    origin_cache[index] = Some(origin);
    Ok(origin)
}

fn part_first_person_visible(
    index: usize,
    parts: &[FigurePart],
    part_names: &HashMap<String, usize>,
    visible_cache: &mut [Option<bool>],
    visiting: &mut [bool],
) -> Result<bool> {
    if let Some(visible) = visible_cache[index] {
        return Ok(visible);
    }
    if visiting[index] {
        bail!(
            "asset-lab figure has a parent cycle at part '{}'",
            parts[index].name
        );
    }
    visiting[index] = true;

    let mut visible = !is_first_person_hidden_part_name(&parts[index].name);
    if visible && let Some(parent_name) = parts[index].parent.as_deref() {
        let parent_index = *part_names.get(parent_name).with_context(|| {
            format!(
                "part '{}' has unknown parent '{}'",
                parts[index].name, parent_name
            )
        })?;
        visible =
            part_first_person_visible(parent_index, parts, part_names, visible_cache, visiting)?;
    }

    visiting[index] = false;
    visible_cache[index] = Some(visible);
    Ok(visible)
}

fn is_first_person_hidden_part_name(name: &str) -> bool {
    name == "head" || name.ends_with("_head")
}

fn compile_clips(
    asset: &FigureAsset,
    part_names: &HashMap<String, usize>,
) -> Result<BTreeMap<String, CompiledFigureClip>> {
    let mut clips = BTreeMap::new();
    for (name, clip) in &asset.clips {
        clips.insert(name.clone(), compile_clip(asset, name, clip, part_names)?);
    }
    Ok(clips)
}

fn compile_clip(
    asset: &FigureAsset,
    clip_name: &str,
    clip: &FigureClip,
    part_names: &HashMap<String, usize>,
) -> Result<CompiledFigureClip> {
    let mut tracks: BTreeMap<usize, Vec<CompiledFigureKey>> = BTreeMap::new();
    let mut duration_seconds = 0.0_f32;
    for key in &clip.keys {
        let part_index = *part_names.get(&key.0).with_context(|| {
            format!(
                "asset-lab figure '{}' clip '{}' references unknown part '{}'",
                asset.name, clip_name, key.0
            )
        })?;
        if !key.1.is_finite() || key.1 < 0.0 {
            bail!(
                "asset-lab figure '{}' clip '{}' has invalid key time {}",
                asset.name,
                clip_name,
                key.1
            );
        }
        duration_seconds = duration_seconds.max(key.1);
        tracks
            .entry(part_index)
            .or_default()
            .push(CompiledFigureKey {
                time_seconds: key.1,
                transform: compile_clip_transform(asset, clip_name, &key.2)?,
            });
    }
    if clip.keys.is_empty() {
        bail!(
            "asset-lab figure '{}' clip '{}' has no keys",
            asset.name,
            clip_name
        );
    }
    for keys in tracks.values_mut() {
        keys.sort_by(|left, right| {
            left.time_seconds
                .partial_cmp(&right.time_seconds)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    Ok(CompiledFigureClip {
        duration_seconds,
        looped: clip.r#loop,
        tracks,
        locomotion: clip
            .locomotion
            .as_ref()
            .map(|locomotion| compile_locomotion(asset, clip_name, locomotion, part_names))
            .transpose()?,
    })
}

fn compile_clip_transform(
    asset: &FigureAsset,
    clip_name: &str,
    transform: &FigureClipTransform,
) -> Result<CompiledFigureTransform> {
    if let Some(scale) = transform.scale {
        if scale.iter().any(|value| !value.is_finite()) {
            bail!(
                "asset-lab figure '{}' clip '{}' has non-finite scale key",
                asset.name,
                clip_name
            );
        }
        if scale.iter().any(|value| (value - 1.0).abs() > 1.0e-6) {
            bail!(
                "asset-lab figure '{}' clip '{}' uses animated scale, which is not supported yet",
                asset.name,
                clip_name
            );
        }
    }
    let at = transform.at.map(Vec3::from_array);
    if let Some(at) = at
        && !at.is_finite()
    {
        bail!(
            "asset-lab figure '{}' clip '{}' has non-finite at key",
            asset.name,
            clip_name
        );
    }
    let rot_radians = transform.rot.map(euler_degrees_to_radians);
    if let Some(rot_radians) = rot_radians
        && !rot_radians.is_finite()
    {
        bail!(
            "asset-lab figure '{}' clip '{}' has non-finite rot key",
            asset.name,
            clip_name
        );
    }
    Ok(CompiledFigureTransform { at, rot_radians })
}

fn compile_locomotion(
    asset: &FigureAsset,
    clip_name: &str,
    locomotion: &FigureClipLocomotion,
    part_names: &HashMap<String, usize>,
) -> Result<CompiledFigureLocomotion> {
    if !locomotion.cycle_distance.is_finite() || locomotion.cycle_distance <= 0.0 {
        bail!(
            "asset-lab figure '{}' clip '{}' locomotion cycleDistance must be positive",
            asset.name,
            clip_name
        );
    }
    if let Some(speed) = locomotion.speed
        && (!speed.is_finite() || speed <= 0.0)
    {
        bail!(
            "asset-lab figure '{}' clip '{}' locomotion speed must be positive",
            asset.name,
            clip_name
        );
    }
    let direction = locomotion.direction.map(Vec3::from_array);
    if let Some(direction) = direction
        && (!direction.is_finite() || direction.length_squared() <= f32::EPSILON)
    {
        bail!(
            "asset-lab figure '{}' clip '{}' locomotion direction must be finite and nonzero",
            asset.name,
            clip_name
        );
    }
    let mut contacts = Vec::with_capacity(locomotion.contacts.len());
    for contact in &locomotion.contacts {
        let part_index = *part_names.get(&contact.part).with_context(|| {
            format!(
                "asset-lab figure '{}' clip '{}' locomotion contact references unknown part '{}'",
                asset.name, clip_name, contact.part
            )
        })?;
        if !contact.phase_start.is_finite()
            || !contact.phase_end.is_finite()
            || !contact.stance_ratio.is_finite()
            || !(0.0..=1.0).contains(&contact.stance_ratio)
        {
            bail!(
                "asset-lab figure '{}' clip '{}' locomotion contact '{}' has invalid phase metadata",
                asset.name,
                clip_name,
                contact.part
            );
        }
        contacts.push(CompiledFigureContact {
            part_index,
            phase_start: contact.phase_start,
            phase_end: contact.phase_end,
            role: contact.role.clone(),
            stance_ratio: contact.stance_ratio,
        });
    }
    Ok(CompiledFigureLocomotion {
        kind: locomotion.kind.clone(),
        cycle_distance: locomotion.cycle_distance,
        speed: locomotion.speed,
        direction,
        contacts,
    })
}

fn append_texture_overlay_cuboids(
    overlay_cuboids: &mut Vec<CompiledOverlayCuboid>,
    box_origin: Vec3,
    box_size: Vec3,
    face: BoxFace,
    asset: &FigureAsset,
    texture_name: &str,
    first_person_visible: bool,
) -> Result<()> {
    let texture = asset.textures.get(texture_name).with_context(|| {
        format!(
            "asset-lab figure '{}' references unknown texture '{}'",
            asset.name, texture_name
        )
    })?;
    let texture_width = validate_texture(texture_name, texture)?;
    let texture_height = texture.pixels.len();
    for (row_index, row) in texture.pixels.iter().enumerate() {
        for (column_index, symbol) in row.chars().enumerate() {
            let color_string = texture.palette.get(&symbol.to_string()).with_context(|| {
                format!(
                    "asset-lab texture '{}' references unknown palette symbol '{}'",
                    texture_name, symbol
                )
            })?;
            let color = parse_hex_color(color_string).with_context(|| {
                format!(
                    "asset-lab texture '{}' palette symbol '{}' has invalid color '{}'",
                    texture_name, symbol, color_string
                )
            })?;
            let (min, max) = texture_cell_lab_bounds(
                box_origin,
                box_size,
                face,
                column_index,
                row_index,
                texture_width,
                texture_height,
            );
            let (min, max) = lab_box_bounds_to_actor_local(min, max);
            overlay_cuboids.push(CompiledOverlayCuboid {
                min,
                max,
                color,
                first_person_visible,
            });
        }
    }
    Ok(())
}

fn validate_texture(name: &str, texture: &FigureAsciiTexture) -> Result<usize> {
    let Some(first_row) = texture.pixels.first() else {
        bail!("asset-lab texture '{}' has no rows", name);
    };
    let width = first_row.chars().count();
    if width == 0 {
        bail!("asset-lab texture '{}' has an empty first row", name);
    }
    for row in &texture.pixels {
        if row.chars().count() != width {
            bail!("asset-lab texture '{}' has ragged rows", name);
        }
    }
    Ok(width)
}

#[allow(clippy::too_many_arguments)]
fn texture_cell_lab_bounds(
    origin: Vec3,
    size: Vec3,
    face: BoxFace,
    column_index: usize,
    row_index: usize,
    texture_width: usize,
    texture_height: usize,
) -> (Vec3, Vec3) {
    let min = -size * 0.5;
    let max = size * 0.5;
    let u0 = column_index as f32 / texture_width as f32;
    let u1 = (column_index + 1) as f32 / texture_width as f32;
    let v_top = 1.0 - row_index as f32 / texture_height as f32;
    let v_bottom = 1.0 - (row_index + 1) as f32 / texture_height as f32;

    let (cell_min, cell_max) = match face {
        BoxFace::North => (
            Vec3::new(
                lerp(min.x, max.x, u0),
                lerp(min.y, max.y, v_bottom),
                min.z - TEXTURE_OVERLAY_DEPTH,
            ),
            Vec3::new(lerp(min.x, max.x, u1), lerp(min.y, max.y, v_top), min.z),
        ),
        BoxFace::South => (
            Vec3::new(lerp(max.x, min.x, u1), lerp(min.y, max.y, v_bottom), max.z),
            Vec3::new(
                lerp(max.x, min.x, u0),
                lerp(min.y, max.y, v_top),
                max.z + TEXTURE_OVERLAY_DEPTH,
            ),
        ),
        BoxFace::West => (
            Vec3::new(
                min.x - TEXTURE_OVERLAY_DEPTH,
                lerp(min.y, max.y, v_bottom),
                lerp(max.z, min.z, u1),
            ),
            Vec3::new(min.x, lerp(min.y, max.y, v_top), lerp(max.z, min.z, u0)),
        ),
        BoxFace::East => (
            Vec3::new(max.x, lerp(min.y, max.y, v_bottom), lerp(min.z, max.z, u0)),
            Vec3::new(
                max.x + TEXTURE_OVERLAY_DEPTH,
                lerp(min.y, max.y, v_top),
                lerp(min.z, max.z, u1),
            ),
        ),
        BoxFace::Up => (
            Vec3::new(lerp(min.x, max.x, u0), max.y, lerp(max.z, min.z, u1)),
            Vec3::new(
                lerp(min.x, max.x, u1),
                max.y + TEXTURE_OVERLAY_DEPTH,
                lerp(max.z, min.z, u0),
            ),
        ),
        BoxFace::Down => (
            Vec3::new(
                lerp(min.x, max.x, u0),
                min.y - TEXTURE_OVERLAY_DEPTH,
                lerp(min.z, max.z, u0),
            ),
            Vec3::new(lerp(min.x, max.x, u1), min.y, lerp(min.z, max.z, u1)),
        ),
    };
    (
        origin + cell_min.min(cell_max),
        origin + cell_min.max(cell_max),
    )
}

fn normalize_figure(
    cuboids: Vec<CompiledCuboid>,
    overlay_cuboids: Vec<CompiledOverlayCuboid>,
    parts: Vec<CompiledFigurePart>,
    clips: BTreeMap<String, CompiledFigureClip>,
) -> Result<CompiledFigure> {
    let bounds = cuboid_bounds(&cuboids);
    let height = bounds.max.y - bounds.min.y;
    if height <= 0.0 {
        bail!("asset-lab figure has non-positive compiled height");
    }
    let origin = Vec3::new(
        (bounds.min.x + bounds.max.x) * 0.5,
        bounds.min.y,
        (bounds.min.z + bounds.max.z) * 0.5,
    );
    let inv_height = 1.0 / height;
    Ok(CompiledFigure {
        normalization_origin: origin,
        inv_height,
        parts,
        clips,
        cuboids: cuboids
            .into_iter()
            .map(|cuboid| CompiledCuboid {
                min: (cuboid.min - origin) * inv_height,
                max: (cuboid.max - origin) * inv_height,
                face_colors: cuboid.face_colors,
                first_person_visible: cuboid.first_person_visible,
            })
            .collect(),
        overlay_cuboids: overlay_cuboids
            .into_iter()
            .map(|cuboid| CompiledOverlayCuboid {
                min: (cuboid.min - origin) * inv_height,
                max: (cuboid.max - origin) * inv_height,
                color: cuboid.color,
                first_person_visible: cuboid.first_person_visible,
            })
            .collect(),
    })
}

fn cuboid_bounds(cuboids: &[CompiledCuboid]) -> Bounds {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for cuboid in cuboids {
        min = min.min(cuboid.min).min(cuboid.max);
        max = max.max(cuboid.min).max(cuboid.max);
    }
    Bounds { min, max }
}

fn material_color(materials: &HashMap<String, [f32; 4]>, material_name: &str) -> Result<[f32; 4]> {
    materials
        .get(material_name)
        .copied()
        .with_context(|| format!("unknown asset-lab material '{}'", material_name))
}

fn parse_box_face(face: &str) -> Result<BoxFace> {
    match face {
        "north" => Ok(BoxFace::North),
        "south" => Ok(BoxFace::South),
        "west" => Ok(BoxFace::West),
        "east" => Ok(BoxFace::East),
        "up" => Ok(BoxFace::Up),
        "down" => Ok(BoxFace::Down),
        _ => bail!("unknown asset-lab box face '{}'", face),
    }
}

fn euler_degrees_to_radians(rotation: [f32; 3]) -> Vec3 {
    Vec3::new(
        rotation[0].to_radians(),
        rotation[1].to_radians(),
        rotation[2].to_radians(),
    )
}

fn lab_box_bounds_to_actor_local(min: Vec3, max: Vec3) -> (Vec3, Vec3) {
    mirror_z_bounds(min, max)
}

fn actor_local_bounds_to_lab(min: Vec3, max: Vec3) -> (Vec3, Vec3) {
    mirror_z_bounds(min, max)
}

fn mirror_z_bounds(min: Vec3, max: Vec3) -> (Vec3, Vec3) {
    let a = Vec3::new(min.x, min.y, -min.z);
    let b = Vec3::new(max.x, max.y, -max.z);
    (a.min(b), a.max(b))
}

fn parse_hex_color(value: &str) -> Result<[f32; 4]> {
    let hex = value
        .strip_prefix('#')
        .with_context(|| format!("expected #rrggbb color, got '{}'", value))?;
    if hex.len() != 6 {
        bail!("expected #rrggbb color, got '{}'", value);
    }
    let red = u8::from_str_radix(&hex[0..2], 16)?;
    let green = u8::from_str_radix(&hex[2..4], 16)?;
    let blue = u8::from_str_radix(&hex[4..6], 16)?;
    Ok([
        red as f32 / 255.0,
        green as f32 / 255.0,
        blue as f32 / 255.0,
        1.0,
    ])
}

fn shaded_faces(color: [f32; 4]) -> [[f32; 4]; 6] {
    [
        shade_face(color, 0),
        shade_face(color, 1),
        shade_face(color, 2),
        shade_face(color, 3),
        shade_face(color, 4),
        shade_face(color, 5),
    ]
}

fn shade_face(color: [f32; 4], face_index: usize) -> [f32; 4] {
    let factor = match face_index {
        0 => 0.58,
        1 => 0.90,
        2 => 0.72,
        3 => 0.82,
        4 => 0.66,
        _ => 1.08,
    };
    [
        (color[0] * factor).clamp(0.0, 1.0),
        (color[1] * factor).clamp(0.0, 1.0),
        (color[2] * factor).clamp(0.0, 1.0),
        color[3],
    ]
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoxFace {
    North,
    South,
    West,
    East,
    Up,
    Down,
}

impl BoxFace {
    const ALL: [Self; 6] = [
        Self::North,
        Self::South,
        Self::West,
        Self::East,
        Self::Up,
        Self::Down,
    ];

    fn actor_face_index(self) -> usize {
        match self {
            Self::South => 0,
            Self::North => 1,
            Self::West => 2,
            Self::East => 3,
            Self::Down => 4,
            Self::Up => 5,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: Vec3,
    max: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER_FIGURE_JSON: &str =
        include_str!("../../../../assets/mclone/figures/player.figure.json");
    const CHICKEN_FIGURE_JSON: &str =
        include_str!("../../../../assets/mclone/figures/chicken.figure.json");

    #[test]
    fn compiles_player_figure_asset() {
        let figure = compile_test_player_figure();

        assert_eq!(figure.cuboids.len(), 12);
        assert_eq!(figure.overlay_cuboids.len(), 64);
        assert_eq!(figure.parts.len(), 12);
        assert!(figure.clips.contains_key("walk"));
        let walk = figure.clips.get("walk").unwrap();
        assert!((walk.duration_seconds - 0.9).abs() < 1.0e-6);
        assert_eq!(walk.locomotion.as_ref().unwrap().contacts.len(), 2);

        let bounds = compiled_bounds(&figure);
        assert!((bounds.min.y - 0.0).abs() < 1.0e-6);
        assert!((bounds.max.y - 1.0).abs() < 1.0e-6);
        assert!(bounds.min.x < -0.22);
        assert!(bounds.max.x > 0.22);
        assert!(bounds.max.z > 0.12);
    }

    #[test]
    fn compiles_chicken_figure_asset_with_non_box_primitives() {
        let asset: FigureAsset = serde_json::from_str(CHICKEN_FIGURE_JSON).unwrap();
        let figure = compile_figure_asset(&asset).unwrap();

        assert!(figure.cuboids.len() > 20);
        assert_eq!(figure.overlay_cuboids.len(), 0);
        assert_eq!(figure.parts.len(), asset.parts.len());
        assert!(figure.clips.contains_key("walk"));
        assert!(figure.inv_height > 0.0);

        let bounds = compiled_bounds(&figure);
        assert!((bounds.min.y - 0.0).abs() < 1.0e-6);
        assert!((bounds.max.y - 1.0).abs() < 1.0e-6);
        assert!(bounds.max.x > 0.2);
        assert!(bounds.max.z > 0.2);
    }

    #[test]
    fn first_party_actor_figure_set_loads_default_player() {
        let source = mclone_assets::FilesystemAssetSource::new("../../..");
        let figures = load_first_party_actor_figures(&source).unwrap();

        assert_eq!(figures.len(), 3);
        assert!(
            figures
                .get(mclone_assets::default_player_figure_id())
                .is_some()
        );
        assert!(
            figures
                .get(mclone_assets::upright_bear_figure_id())
                .is_some()
        );
        assert!(figures.get(mclone_assets::chicken_figure_id()).is_some());
    }

    #[test]
    fn actor_figure_load_reports_unknown_ids() {
        let source = mclone_assets::MemoryAssetSource::new();
        let error = load_compiled_actor_figure(
            &source,
            mclone_assets::ActorFigureId::from_static("mclone:missing"),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("unknown actor figure id mclone:missing"));
    }

    #[test]
    fn compiles_face_texture_as_front_overlay() {
        let figure = compile_test_player_figure();
        let eye_color = parse_hex_color("#19120e").unwrap();
        let eye_cuboids: Vec<_> = figure
            .overlay_cuboids
            .iter()
            .filter(|cuboid| cuboid.color == eye_color)
            .collect();

        assert_eq!(eye_cuboids.len(), 8);
        assert!(eye_cuboids.iter().all(|cuboid| cuboid.min.z > 0.10));
        assert!(eye_cuboids.iter().all(|cuboid| cuboid.max.y > 0.68));
    }

    #[test]
    fn player_head_details_are_hidden_for_first_person_body() {
        let figure = compile_test_player_figure();

        assert_eq!(
            figure
                .cuboids
                .iter()
                .filter(|cuboid| !cuboid.first_person_visible)
                .count(),
            2
        );
        assert_eq!(
            figure
                .overlay_cuboids
                .iter()
                .filter(|cuboid| !cuboid.first_person_visible)
                .count(),
            64
        );
    }

    #[test]
    fn rejects_animated_scale_keys_until_supported() {
        let json = r##"
        {
          "schemaVersion": 1,
          "name": "bad_scale",
          "materials": { "skin": { "color": "#ffffff" } },
          "textures": {},
          "parts": [
            { "name": "body", "material": "skin", "primitive": { "kind": "box", "size": [1, 1, 1] } }
          ],
          "clips": {
            "walk": {
              "loop": true,
              "keys": [
                ["body", 0, { "scale": [2, 1, 1] }]
              ]
            }
          }
        }
        "##;

        let asset: FigureAsset = serde_json::from_str(json).unwrap();
        let error = compile_figure_asset(&asset).unwrap_err().to_string();
        assert!(error.contains("uses animated scale"));
    }

    #[test]
    fn rejects_unsupported_primitives() {
        let json = r##"
        {
          "schemaVersion": 1,
          "name": "bad",
          "materials": { "skin": { "color": "#ffffff" } },
          "textures": {},
          "parts": [
            { "name": "body", "material": "skin", "primitive": { "kind": "torus", "radius": 1 } }
          ],
          "clips": {}
        }
        "##;

        let asset: FigureAsset = serde_json::from_str(json).unwrap();
        let error = compile_figure_asset(&asset).unwrap_err().to_string();
        assert!(error.contains("unsupported primitive kind 'torus'"));
    }

    fn compile_test_player_figure() -> CompiledFigure {
        let asset: FigureAsset = serde_json::from_str(PLAYER_FIGURE_JSON).unwrap();
        compile_figure_asset(&asset).unwrap()
    }

    fn compiled_bounds(figure: &CompiledFigure) -> Bounds {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for cuboid in &figure.cuboids {
            min = min.min(cuboid.min).min(cuboid.max);
            max = max.max(cuboid.min).max(cuboid.max);
        }
        Bounds { min, max }
    }
}
