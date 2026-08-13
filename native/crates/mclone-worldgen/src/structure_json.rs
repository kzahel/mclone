use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use mclone_core::BlockPos;
use serde::Deserialize;

use crate::block::{
    AIR, BRICKS, CARROTS_AGE_0, COBBLESTONE, CORNFLOWER, GLASS, HAY_BLOCK, MOSSY_COBBLESTONE,
    OAK_LOG, OAK_LOG_X, OAK_LOG_Z, OAK_PLANKS, OakFenceGateState, OakFenceState, POPPY,
    RED_TERRACOTTA, RawBlockId, SPRUCE_LOG, SPRUCE_LOG_X, SPRUCE_LOG_Z, SPRUCE_PLANKS,
    SPRUCE_SLAB_BOTTOM, SPRUCE_SLAB_TOP, SPRUCE_STAIRS_EAST, SPRUCE_STAIRS_NORTH,
    SPRUCE_STAIRS_SOUTH, SPRUCE_STAIRS_WEST, STONE_BRICKS, WALL_TORCH_SOUTH, WHITE_TERRACOTTA,
    oak_fence_for_state, oak_fence_gate_for_state,
};
use crate::structure_template::{
    StructureMaterialTheme, StructureTemplate, StructureTemplateBuilder, TemplateBlockState,
    TemplateError, TemplateMaterialRole,
};

pub const CANONICAL_STRUCTURE_SCHEMA_VERSION: u32 = 1;
pub const MAX_CANONICAL_STRUCTURE_AXIS: i32 = 512;
pub const MAX_CANONICAL_STRUCTURE_BLOCKS: usize = 16_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStructureRecord {
    pub template: StructureTemplate,
    pub label: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub family: Option<CanonicalStructureFamily>,
    pub components: Vec<CanonicalStructureComponent>,
    pub palette: Vec<CanonicalStructurePaletteEntry>,
    pub blocks: Vec<CanonicalStructureBlock>,
    pub sockets: Vec<CanonicalStructureSocket>,
    pub themes: Vec<CanonicalStructureTheme>,
    pub default_theme: Option<String>,
    pub provenance: CanonicalStructureProvenance,
}

impl CanonicalStructureRecord {
    pub fn theme(&self, id: &str) -> Option<&StructureMaterialTheme> {
        self.themes
            .iter()
            .find(|theme| theme.id == id)
            .map(|theme| &theme.materials)
    }

    pub fn default_theme(&self) -> Option<&StructureMaterialTheme> {
        self.default_theme.as_deref().and_then(|id| self.theme(id))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStructureFamily {
    pub id: String,
    pub member: String,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStructureComponent {
    pub id: String,
    pub label: String,
    pub optional: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStructurePaletteEntry {
    pub key: String,
    pub state: TemplateBlockState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStructureBlock {
    pub local_pos: BlockPos,
    pub palette_index: usize,
    pub state: TemplateBlockState,
    pub components: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStructureSocket {
    pub id: String,
    pub kind: String,
    pub local_pos: BlockPos,
    pub facing: CanonicalSocketFacing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalSocketFacing {
    North,
    East,
    South,
    West,
    Up,
    Down,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStructureTheme {
    pub id: String,
    pub label: String,
    pub materials: StructureMaterialTheme,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStructureProvenance {
    pub compiler_id: String,
    pub source_path: String,
    pub source_sha256: String,
    pub semantic_sha256: String,
}

#[derive(Debug)]
pub enum CanonicalStructureError {
    Json(serde_json::Error),
    Invalid(String),
    Template(TemplateError),
}

impl fmt::Display for CanonicalStructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid structure JSON: {error}"),
            Self::Invalid(message) => formatter.write_str(message),
            Self::Template(error) => write!(formatter, "invalid structure template: {error}"),
        }
    }
}

impl Error for CanonicalStructureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Template(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

impl From<serde_json::Error> for CanonicalStructureError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<TemplateError> for CanonicalStructureError {
    fn from(value: TemplateError) -> Self {
        Self::Template(value)
    }
}

pub fn load_canonical_structure_json(
    json: &str,
) -> Result<CanonicalStructureRecord, CanonicalStructureError> {
    let raw: RawStructure = serde_json::from_str(json)?;
    if raw.schema_version != CANONICAL_STRUCTURE_SCHEMA_VERSION {
        return invalid(format!(
            "structure `{}` uses schema {}, expected {}",
            raw.id, raw.schema_version, CANONICAL_STRUCTURE_SCHEMA_VERSION
        ));
    }
    validate_safe_id(&raw.id, "structure id")?;
    validate_nonempty(&raw.label, "structure label")?;
    validate_nonempty(&raw.description, "structure description")?;
    if !matches!(
        raw.category.as_str(),
        "building" | "outbuilding" | "infrastructure" | "decoration"
    ) {
        return invalid(format!(
            "structure `{}` has invalid category `{}`",
            raw.id, raw.category
        ));
    }
    validate_size(raw.size, &raw.id)?;
    validate_sorted_unique_strings(&raw.tags, "tags", &raw.id)?;

    let component_ids = validate_components(&raw.components, &raw.id)?;
    let palette = validate_palette(&raw.palette, &raw.id)?;
    let mut builder = StructureTemplateBuilder::new(&raw.id, raw.size)?;
    let mut blocks = Vec::with_capacity(raw.blocks.len());
    if raw.blocks.len() > MAX_CANONICAL_STRUCTURE_BLOCKS {
        return invalid(format!(
            "structure `{}` contains {} blocks, exceeding limit {}",
            raw.id,
            raw.blocks.len(),
            MAX_CANONICAL_STRUCTURE_BLOCKS
        ));
    }
    let mut previous_pos = None;
    for (index, block) in raw.blocks.into_iter().enumerate() {
        let pos = block_pos(block.pos);
        validate_pos(pos, raw.size, &format!("block {index}"), &raw.id)?;
        if previous_pos.is_some_and(|previous| previous >= pos) {
            return invalid(format!(
                "structure `{}` blocks are not in strict canonical position order at {:?}",
                raw.id, block.pos
            ));
        }
        previous_pos = Some(pos);
        let state = palette
            .get(block.palette)
            .map(|entry| entry.state)
            .ok_or_else(|| {
                CanonicalStructureError::Invalid(format!(
                    "structure `{}` block {index} references palette index {} outside 0..{}",
                    raw.id,
                    block.palette,
                    palette.len()
                ))
            })?;
        validate_sorted_unique_strings(
            &block.components,
            &format!("block {index} components"),
            &raw.id,
        )?;
        for component in &block.components {
            if !component_ids.contains(component) {
                return invalid(format!(
                    "structure `{}` block {index} references unknown component `{component}`",
                    raw.id
                ));
            }
        }
        builder.set(pos, state)?;
        blocks.push(CanonicalStructureBlock {
            local_pos: pos,
            palette_index: block.palette,
            state,
            components: block.components,
        });
    }

    validate_markers(&raw.markers, raw.size, &raw.id)?;
    for marker in raw.markers {
        builder.marker(block_pos(marker.pos), marker.kind)?;
    }
    let sockets = validate_sockets(raw.sockets, raw.size, &raw.id)?;
    let themes = validate_themes(raw.themes, &raw.id)?;
    if let Some(default_theme) = &raw.default_theme {
        if !themes.iter().any(|theme| &theme.id == default_theme) {
            return invalid(format!(
                "structure `{}` default theme `{default_theme}` is not declared",
                raw.id
            ));
        }
        let used_roles = palette
            .iter()
            .filter_map(|entry| match entry.state {
                TemplateBlockState::Role(role) => Some(role),
                TemplateBlockState::Exact(_) => None,
            })
            .collect::<BTreeSet<_>>();
        let theme = themes
            .iter()
            .find(|theme| &theme.id == default_theme)
            .expect("default theme existence checked");
        for role in used_roles {
            if theme.materials.resolve(role).is_none() {
                return invalid(format!(
                    "structure `{}` default theme `{default_theme}` does not resolve role {role:?}",
                    raw.id
                ));
            }
        }
    }
    let family = raw
        .family
        .map(|family| validate_family(family, &raw.id))
        .transpose()?;
    let provenance = validate_provenance(raw.provenance, &raw.id)?;

    Ok(CanonicalStructureRecord {
        template: builder.build(),
        label: raw.label,
        description: raw.description,
        category: raw.category,
        tags: raw.tags,
        family,
        components: raw
            .components
            .into_iter()
            .map(|component| CanonicalStructureComponent {
                id: component.id,
                label: component.label,
                optional: component.optional,
            })
            .collect(),
        palette,
        blocks,
        sockets,
        themes,
        default_theme: raw.default_theme,
        provenance,
    })
}

pub fn raw_block_state_for_canonical_key(key: &str) -> Option<RawBlockId> {
    for bits in 0_u16..32 {
        let state = OakFenceState {
            north: bits & 1 != 0,
            east: bits & 2 != 0,
            south: bits & 4 != 0,
            west: bits & 8 != 0,
            waterlogged: bits & 16 != 0,
        };
        let canonical = format!(
            "minecraft:oak_fence[east={},north={},south={},waterlogged={},west={}]",
            state.east, state.north, state.south, state.waterlogged, state.west
        );
        if key == canonical {
            return Some(oak_fence_for_state(state));
        }
    }
    for bits in 0_u16..32 {
        let state = OakFenceGateState {
            facing: (bits & 3) as u8,
            open: bits & 4 != 0,
            powered: bits & 8 != 0,
            in_wall: bits & 16 != 0,
        };
        let canonical = format!(
            "minecraft:oak_fence_gate[facing={},in_wall={},open={},powered={}]",
            ["north", "east", "south", "west"][state.facing as usize],
            state.in_wall,
            state.open,
            state.powered
        );
        if key == canonical {
            return oak_fence_gate_for_state(state);
        }
    }
    for age in 0_u16..8 {
        if key == format!("minecraft:carrots[age={age}]") {
            return Some(CARROTS_AGE_0 + age);
        }
    }
    Some(match key {
        "minecraft:air" => AIR,
        "minecraft:bricks" => BRICKS,
        "minecraft:cobblestone" => COBBLESTONE,
        "minecraft:cornflower" => CORNFLOWER,
        "minecraft:glass" => GLASS,
        "minecraft:hay_block" => HAY_BLOCK,
        "minecraft:mossy_cobblestone" => MOSSY_COBBLESTONE,
        "minecraft:oak_log[axis=x]" => OAK_LOG_X,
        "minecraft:oak_log[axis=y]" => OAK_LOG,
        "minecraft:oak_log[axis=z]" => OAK_LOG_Z,
        "minecraft:oak_planks" => OAK_PLANKS,
        "minecraft:poppy" => POPPY,
        "minecraft:red_terracotta" => RED_TERRACOTTA,
        "minecraft:spruce_log[axis=x]" => SPRUCE_LOG_X,
        "minecraft:spruce_log[axis=y]" => SPRUCE_LOG,
        "minecraft:spruce_log[axis=z]" => SPRUCE_LOG_Z,
        "minecraft:spruce_planks" => SPRUCE_PLANKS,
        "minecraft:spruce_slab[type=bottom,waterlogged=false]" => SPRUCE_SLAB_BOTTOM,
        "minecraft:spruce_slab[type=top,waterlogged=false]" => SPRUCE_SLAB_TOP,
        "minecraft:spruce_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]" => {
            SPRUCE_STAIRS_EAST
        }
        "minecraft:spruce_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]" => {
            SPRUCE_STAIRS_NORTH
        }
        "minecraft:spruce_stairs[facing=south,half=bottom,shape=straight,waterlogged=false]" => {
            SPRUCE_STAIRS_SOUTH
        }
        "minecraft:spruce_stairs[facing=west,half=bottom,shape=straight,waterlogged=false]" => {
            SPRUCE_STAIRS_WEST
        }
        "minecraft:stone_bricks" => STONE_BRICKS,
        "minecraft:wall_torch[facing=south]" => WALL_TORCH_SOUTH,
        "minecraft:white_terracotta" => WHITE_TERRACOTTA,
        _ => return None,
    })
}

fn validate_size(size: [i32; 3], structure_id: &str) -> Result<(), CanonicalStructureError> {
    if size
        .into_iter()
        .any(|axis| !(1..=MAX_CANONICAL_STRUCTURE_AXIS).contains(&axis))
    {
        return invalid(format!(
            "structure `{structure_id}` size {size:?} must be within 1..={MAX_CANONICAL_STRUCTURE_AXIS}"
        ));
    }
    Ok(())
}

fn validate_palette(
    entries: &[RawPaletteEntry],
    structure_id: &str,
) -> Result<Vec<CanonicalStructurePaletteEntry>, CanonicalStructureError> {
    if entries.is_empty() {
        return invalid(format!("structure `{structure_id}` palette is empty"));
    }
    let mut previous_key: Option<&str> = None;
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        validate_palette_key(&entry.key)?;
        if previous_key.is_some_and(|previous| previous >= entry.key.as_str()) {
            return invalid(format!(
                "structure `{structure_id}` palette keys are not in strict canonical order at `{}`",
                entry.key
            ));
        }
        previous_key = Some(&entry.key);
        let state = match &entry.state {
            RawPaletteState::Role { role } => {
                TemplateBlockState::Role(material_role(role).ok_or_else(|| {
                    CanonicalStructureError::Invalid(format!(
                        "structure `{structure_id}` palette `{}` has unknown role `{role}`",
                        entry.key
                    ))
                })?)
            }
            RawPaletteState::Block { state } => TemplateBlockState::Exact(
                raw_block_state_for_canonical_key(state).ok_or_else(|| {
                    CanonicalStructureError::Invalid(format!(
                        "structure `{structure_id}` palette `{}` has unknown block state `{state}`",
                        entry.key
                    ))
                })?,
            ),
        };
        result.push(CanonicalStructurePaletteEntry {
            key: entry.key.clone(),
            state,
        });
    }
    Ok(result)
}

fn validate_components(
    components: &[RawComponent],
    structure_id: &str,
) -> Result<BTreeSet<String>, CanonicalStructureError> {
    let mut ids = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for component in components {
        validate_safe_id(&component.id, "component id")?;
        validate_nonempty(&component.label, "component label")?;
        if previous.is_some_and(|value| value >= component.id.as_str()) {
            return invalid(format!(
                "structure `{structure_id}` components are not in strict canonical order at `{}`",
                component.id
            ));
        }
        previous = Some(&component.id);
        ids.insert(component.id.clone());
    }
    Ok(ids)
}

fn validate_markers(
    markers: &[RawMarker],
    size: [i32; 3],
    structure_id: &str,
) -> Result<(), CanonicalStructureError> {
    for marker in markers {
        validate_marker_kind(&marker.kind)?;
        validate_pos(block_pos(marker.pos), size, "marker", structure_id)?;
    }
    Ok(())
}

fn validate_sockets(
    sockets: Vec<RawSocket>,
    size: [i32; 3],
    structure_id: &str,
) -> Result<Vec<CanonicalStructureSocket>, CanonicalStructureError> {
    let mut result = Vec::with_capacity(sockets.len());
    let mut previous: Option<String> = None;
    for socket in sockets {
        validate_safe_id(&socket.id, "socket id")?;
        validate_marker_kind(&socket.kind)?;
        let pos = block_pos(socket.pos);
        validate_pos(pos, size, "socket", structure_id)?;
        if previous
            .as_deref()
            .is_some_and(|value| value >= socket.id.as_str())
        {
            return invalid(format!(
                "structure `{structure_id}` sockets are not in strict canonical order at `{}`",
                socket.id
            ));
        }
        previous = Some(socket.id.clone());
        result.push(CanonicalStructureSocket {
            id: socket.id,
            kind: socket.kind,
            local_pos: pos,
            facing: match socket.facing.as_str() {
                "north" => CanonicalSocketFacing::North,
                "east" => CanonicalSocketFacing::East,
                "south" => CanonicalSocketFacing::South,
                "west" => CanonicalSocketFacing::West,
                "up" => CanonicalSocketFacing::Up,
                "down" => CanonicalSocketFacing::Down,
                other => {
                    return invalid(format!(
                        "structure `{structure_id}` socket has invalid facing `{other}`"
                    ));
                }
            },
        });
    }
    Ok(result)
}

fn validate_themes(
    themes: Vec<RawTheme>,
    structure_id: &str,
) -> Result<Vec<CanonicalStructureTheme>, CanonicalStructureError> {
    let mut result = Vec::with_capacity(themes.len());
    let mut previous: Option<String> = None;
    for raw in themes {
        validate_safe_id(&raw.id, "theme id")?;
        validate_nonempty(&raw.label, "theme label")?;
        if previous
            .as_deref()
            .is_some_and(|value| value >= raw.id.as_str())
        {
            return invalid(format!(
                "structure `{structure_id}` themes are not in strict canonical order at `{}`",
                raw.id
            ));
        }
        previous = Some(raw.id.clone());
        let mut theme = StructureMaterialTheme::new(&raw.id);
        for (role_name, state_key) in raw.materials {
            let role = material_role(&role_name).ok_or_else(|| {
                CanonicalStructureError::Invalid(format!(
                    "structure `{structure_id}` theme `{}` has unknown role `{role_name}`",
                    raw.id
                ))
            })?;
            let state = raw_block_state_for_canonical_key(&state_key).ok_or_else(|| {
                CanonicalStructureError::Invalid(format!(
                    "structure `{structure_id}` theme `{}` has unknown block state `{state_key}`",
                    raw.id
                ))
            })?;
            theme = theme.with(role, state);
        }
        result.push(CanonicalStructureTheme {
            id: raw.id,
            label: raw.label,
            materials: theme,
        });
    }
    Ok(result)
}

fn validate_family(
    family: RawFamily,
    structure_id: &str,
) -> Result<CanonicalStructureFamily, CanonicalStructureError> {
    validate_safe_id(&family.id, "family id")?;
    validate_safe_id(&family.member, "family member")?;
    validate_nonempty(&family.label, "family label")?;
    if family.id == structure_id {
        return invalid(format!(
            "structure `{structure_id}` family id must differ from the structure id"
        ));
    }
    Ok(CanonicalStructureFamily {
        id: family.id,
        member: family.member,
        label: family.label,
    })
}

fn validate_provenance(
    provenance: RawProvenance,
    structure_id: &str,
) -> Result<CanonicalStructureProvenance, CanonicalStructureError> {
    validate_nonempty(&provenance.compiler_id, "compiler id")?;
    if !safe_relative_path(&provenance.source_path) {
        return invalid(format!(
            "structure `{structure_id}` has unsafe source path `{}`",
            provenance.source_path
        ));
    }
    if !sha256_digest(&provenance.source_sha256) || !sha256_digest(&provenance.semantic_sha256) {
        return invalid(format!(
            "structure `{structure_id}` provenance hashes must be lowercase SHA-256 digests"
        ));
    }
    Ok(CanonicalStructureProvenance {
        compiler_id: provenance.compiler_id,
        source_path: provenance.source_path,
        source_sha256: provenance.source_sha256,
        semantic_sha256: provenance.semantic_sha256,
    })
}

fn validate_pos(
    pos: BlockPos,
    size: [i32; 3],
    label: &str,
    structure_id: &str,
) -> Result<(), CanonicalStructureError> {
    if pos.x < 0
        || pos.y < 0
        || pos.z < 0
        || pos.x >= size[0]
        || pos.y >= size[1]
        || pos.z >= size[2]
    {
        return invalid(format!(
            "structure `{structure_id}` {label} position {pos:?} is outside size {size:?}"
        ));
    }
    Ok(())
}

fn validate_sorted_unique_strings(
    values: &[String],
    label: &str,
    structure_id: &str,
) -> Result<(), CanonicalStructureError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return invalid(format!(
            "structure `{structure_id}` {label} are not in strict canonical order"
        ));
    }
    for value in values {
        validate_safe_id(value, label)?;
    }
    Ok(())
}

fn validate_safe_id(value: &str, label: &str) -> Result<(), CanonicalStructureError> {
    let valid = value
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        });
    if !valid {
        return invalid(format!("{label} `{value}` is not a safe lowercase id"));
    }
    Ok(())
}

fn validate_palette_key(value: &str) -> Result<(), CanonicalStructureError> {
    let valid = value
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if !valid {
        return invalid(format!("palette key `{value}` is invalid"));
    }
    Ok(())
}

fn validate_marker_kind(value: &str) -> Result<(), CanonicalStructureError> {
    let valid = value
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b':')
        });
    if !valid {
        return invalid(format!("marker kind `{value}` is invalid"));
    }
    Ok(())
}

fn validate_nonempty(value: &str, label: &str) -> Result<(), CanonicalStructureError> {
    if value.trim().is_empty() {
        return invalid(format!("{label} must not be empty"));
    }
    Ok(())
}

fn material_role(value: &str) -> Option<TemplateMaterialRole> {
    Some(match value {
        "foundation" => TemplateMaterialRole::Foundation,
        "wall" => TemplateMaterialRole::Wall,
        "timberY" => TemplateMaterialRole::TimberY,
        "timberX" => TemplateMaterialRole::TimberX,
        "timberZ" => TemplateMaterialRole::TimberZ,
        "roof" => TemplateMaterialRole::Roof,
        "roofNorth" => TemplateMaterialRole::RoofNorth,
        "roofEast" => TemplateMaterialRole::RoofEast,
        "roofSouth" => TemplateMaterialRole::RoofSouth,
        "roofWest" => TemplateMaterialRole::RoofWest,
        "roofSlabBottom" => TemplateMaterialRole::RoofSlabBottom,
        "roofSlabTop" => TemplateMaterialRole::RoofSlabTop,
        "glazing" => TemplateMaterialRole::Glazing,
        "trim" => TemplateMaterialRole::Trim,
        "floor" => TemplateMaterialRole::Floor,
        "accent" => TemplateMaterialRole::Accent,
        _ => return None,
    })
}

fn block_pos(pos: [i32; 3]) -> BlockPos {
    BlockPos::new(pos[0], pos[1], pos[2])
}

fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn sha256_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid<T>(message: String) -> Result<T, CanonicalStructureError> {
    Err(CanonicalStructureError::Invalid(message))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawStructure {
    schema_version: u32,
    id: String,
    label: String,
    description: String,
    category: String,
    size: [i32; 3],
    palette: Vec<RawPaletteEntry>,
    blocks: Vec<RawBlock>,
    markers: Vec<RawMarker>,
    sockets: Vec<RawSocket>,
    components: Vec<RawComponent>,
    tags: Vec<String>,
    family: Option<RawFamily>,
    themes: Vec<RawTheme>,
    default_theme: Option<String>,
    provenance: RawProvenance,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPaletteEntry {
    key: String,
    state: RawPaletteState,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum RawPaletteState {
    Role { role: String },
    Block { state: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBlock {
    components: Vec<String>,
    palette: usize,
    pos: [i32; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMarker {
    kind: String,
    pos: [i32; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSocket {
    facing: String,
    id: String,
    kind: String,
    pos: [i32; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawComponent {
    id: String,
    label: String,
    optional: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFamily {
    id: String,
    member: String,
    label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTheme {
    id: String,
    label: String,
    materials: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawProvenance {
    compiler_id: String,
    source_path: String,
    source_sha256: String,
    semantic_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const STANDARD_COTTAGE_JSON: &str =
        include_str!("../../../../assets/mclone/structures/farmstead-cottage-a-v2.structure.json");

    #[test]
    fn loads_checked_standard_cottage_record() {
        let record = load_canonical_structure_json(STANDARD_COTTAGE_JSON).unwrap();
        assert_eq!(record.template.id(), "farmstead-cottage-a-v2");
        assert_eq!(record.template.size(), [15, 15, 17]);
        assert_eq!(record.template.blocks().len(), 1_320);
        assert_eq!(record.template.markers().len(), 2);
        assert_eq!(record.components.len(), 7);
        assert_eq!(record.sockets.len(), 1);
        assert_eq!(
            record.default_theme.as_deref(),
            Some("warm-oak-and-plaster-v2")
        );
        assert!(record.default_theme().is_some());
    }

    #[test]
    fn rejects_unknown_exact_block_state_before_building() {
        let malformed = STANDARD_COTTAGE_JSON.replacen(
            "minecraft:mossy_cobblestone",
            "minecraft:not_a_real_block",
            1,
        );
        let error = load_canonical_structure_json(&malformed).unwrap_err();
        assert!(error.to_string().contains("unknown block state"));
    }

    #[test]
    fn rejects_out_of_bounds_and_noncanonical_blocks() {
        let malformed = STANDARD_COTTAGE_JSON.replacen("\"pos\":[0,6,1]", "\"pos\":[600,6,1]", 1);
        let error = load_canonical_structure_json(&malformed).unwrap_err();
        assert!(error.to_string().contains("outside size"));

        let malformed = STANDARD_COTTAGE_JSON.replacen(
            "{\"components\":[\"roof\"],\"palette\":8,\"pos\":[0,6,2]}",
            "{\"components\":[\"roof\"],\"palette\":8,\"pos\":[0,6,1]}",
            1,
        );
        let error = load_canonical_structure_json(&malformed).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("strict canonical position order")
        );
    }
}
