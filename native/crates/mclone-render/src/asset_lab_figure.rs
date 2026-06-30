use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use mclone_assets::{
    FigureAsciiTexture, FigureAsset, FigurePart, FigurePrimitive, default_player_figure_path,
    load_figure_asset,
};

const TEXTURE_OVERLAY_DEPTH: f32 = 0.004;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledFigure {
    pub(crate) cuboids: Vec<CompiledCuboid>,
    pub(crate) overlay_cuboids: Vec<CompiledOverlayCuboid>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CompiledCuboid {
    pub min: Vec3,
    pub max: Vec3,
    pub face_colors: [[f32; 4]; 6],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CompiledOverlayCuboid {
    pub min: Vec3,
    pub max: Vec3,
    pub color: [f32; 4],
}

pub(crate) fn load_compiled_player_figure(
    source: &impl mclone_assets::AssetSource,
) -> Result<CompiledFigure> {
    let path = default_player_figure_path();
    let asset = load_figure_asset(source, &path)
        .with_context(|| format!("failed to load player figure asset {}", path.as_str()))?;
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
    let mut raw_cuboids = Vec::new();
    let mut raw_overlay_cuboids = Vec::new();

    for (part_index, part) in asset.parts.iter().enumerate() {
        if has_nonzero_rotation(part.rot) {
            bail!(
                "asset-lab figure '{}' part '{}' has unsupported static rotation",
                asset.name,
                part.name
            );
        }
        let FigurePrimitive { kind, size, faces } = &part.primitive;
        if kind != "box" {
            bail!(
                "asset-lab figure '{}' part '{}' uses unsupported primitive kind '{}'",
                asset.name,
                part.name,
                kind
            );
        }
        let size = Vec3::from_array(size.with_context(|| {
            format!(
                "asset-lab figure '{}' box part '{}' is missing size",
                asset.name, part.name
            )
        })?);
        if size.x <= 0.0 || size.y <= 0.0 || size.z <= 0.0 {
            bail!(
                "asset-lab figure '{}' box part '{}' has non-positive size {:?}",
                asset.name,
                part.name,
                size
            );
        }

        let origin = part_origin(
            part_index,
            &asset.parts,
            &part_names,
            &mut origin_cache,
            &mut visiting,
        )?;
        let base_color = part
            .material
            .as_deref()
            .map(|name| material_color(&materials, name))
            .transpose()?
            .unwrap_or([0.84, 0.87, 0.89, 1.0]);
        let mut face_colors = shaded_faces(base_color);
        let face_overrides = faces.as_ref();
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
        let (min, max) = lab_box_bounds_to_actor_local(origin - half_size, origin + half_size);
        raw_cuboids.push(CompiledCuboid {
            min,
            max,
            face_colors,
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
                    )?;
                }
            }
        }
    }

    if raw_cuboids.is_empty() {
        bail!("asset-lab figure '{}' compiled to no cuboids", asset.name);
    }
    normalize_figure(raw_cuboids, raw_overlay_cuboids)
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

fn append_texture_overlay_cuboids(
    overlay_cuboids: &mut Vec<CompiledOverlayCuboid>,
    box_origin: Vec3,
    box_size: Vec3,
    face: BoxFace,
    asset: &FigureAsset,
    texture_name: &str,
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
            overlay_cuboids.push(CompiledOverlayCuboid { min, max, color });
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
        cuboids: cuboids
            .into_iter()
            .map(|cuboid| CompiledCuboid {
                min: (cuboid.min - origin) * inv_height,
                max: (cuboid.max - origin) * inv_height,
                face_colors: cuboid.face_colors,
            })
            .collect(),
        overlay_cuboids: overlay_cuboids
            .into_iter()
            .map(|cuboid| CompiledOverlayCuboid {
                min: (cuboid.min - origin) * inv_height,
                max: (cuboid.max - origin) * inv_height,
                color: cuboid.color,
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

fn has_nonzero_rotation(rotation: Option<[f32; 3]>) -> bool {
    rotation
        .map(|rotation| rotation.iter().any(|value| value.abs() > f32::EPSILON))
        .unwrap_or(false)
}

fn lab_box_bounds_to_actor_local(min: Vec3, max: Vec3) -> (Vec3, Vec3) {
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

    #[test]
    fn compiles_player_figure_asset() {
        let figure = compile_test_player_figure();

        assert_eq!(figure.cuboids.len(), 12);
        assert_eq!(figure.overlay_cuboids.len(), 64);

        let bounds = compiled_bounds(&figure);
        assert!((bounds.min.y - 0.0).abs() < 1.0e-6);
        assert!((bounds.max.y - 1.0).abs() < 1.0e-6);
        assert!(bounds.min.x < -0.22);
        assert!(bounds.max.x > 0.22);
        assert!(bounds.max.z > 0.12);
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
    fn rejects_unsupported_primitives() {
        let json = r##"
        {
          "schemaVersion": 1,
          "name": "bad",
          "materials": { "skin": { "color": "#ffffff" } },
          "textures": {},
          "parts": [
            { "name": "body", "material": "skin", "primitive": { "kind": "sphere", "radius": 1 } }
          ],
          "clips": {}
        }
        "##;

        let asset: FigureAsset = serde_json::from_str(json).unwrap();
        let error = compile_figure_asset(&asset).unwrap_err().to_string();
        assert!(error.contains("unsupported primitive kind 'sphere'"));
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
