use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use mclone_core::{ChunkPos, ChunkRevision, Vec3d, block_to_chunk_coord, block_to_section_coord};
use mclone_protocol::{
    ClientIdentity, DimensionKey, EntityRotation, ItemKind, ItemStackSnapshot,
    MallardFieldGuideProgress, MallardObservationKind,
};
use mclone_worldgen::block::{LILY_PAD, generated_block_state_id};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::persistence::{
    EntityChunkRecord, EntityPersistentId, EntitySavePayload, EntitySaveRecord, PlayerRecord,
    PlayerRecordKey,
};
use crate::{
    AUTHORED_WORLD_HEIGHT, AUTHORED_WORLD_MIN_Y, AuthoredWorldFixtureKind, ChunkStoreError,
    DimensionDefinition, DimensionRecord, MemoryWorldStore, WorldBehaviorProfile,
    WorldGenerationProfile, WorldMetadata, WorldStore, write_authored_world_fixture_to_store,
};

pub const PLAYABLE_SHOWCASE_SCHEMA_VERSION: u32 = 1;
const MAX_BLOCK_PATCHES: usize = 256;
const MAX_ENTITIES: usize = 64;
const MAX_ENTITY_ID_BYTES: usize = 64;
const PLAYER_EYE_HEIGHT: f64 = 1.62;

const MALLARD_ECOLOGY_RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/showcases/mallard-ecology.showcase.json"
));

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlayableShowcaseId {
    MallardEcology,
}

impl PlayableShowcaseId {
    pub const ALL: [Self; 1] = [Self::MallardEcology];

    pub const fn label(self) -> &'static str {
        match self {
            Self::MallardEcology => "mallard-ecology",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PlayableShowcaseError> {
        match value {
            "mallard-ecology" => Ok(Self::MallardEcology),
            _ => Err(PlayableShowcaseError::invalid(format!(
                "unknown playable showcase `{value}`; expected mallard-ecology"
            ))),
        }
    }

    const fn recipe_json(self) -> &'static str {
        match self {
            Self::MallardEcology => MALLARD_ECOLOGY_RECIPE,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayableShowcaseManifest {
    pub schema_version: u32,
    pub id: PlayableShowcaseId,
    pub title: String,
    pub revision: u32,
    pub seed: i64,
    pub world_generation_profile: WorldGenerationProfile,
    pub day_time: u64,
    pub freeze_time: bool,
    pub entry_feet: [f64; 3],
    pub entry_eye: [f64; 3],
    pub entry_look_at: [f64; 3],
    pub entity_count: usize,
    pub mallard_count: usize,
    pub mallard_nest_count: usize,
    pub field_guide_bits: u32,
}

#[derive(Debug)]
pub struct PlayableShowcaseError {
    message: String,
}

impl PlayableShowcaseError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PlayableShowcaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for PlayableShowcaseError {}

impl From<ChunkStoreError> for PlayableShowcaseError {
    fn from(error: ChunkStoreError) -> Self {
        Self::invalid(format!("playable showcase persistence failed: {error}"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveInstantiationSubject {
    LilyPad,
    AdultMallard,
    MallardDuckling,
    MallardNest,
    MallardEggItem,
    MallardFeatherItem,
    MallardObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveInstantiationEvidence {
    pub id: &'static str,
    pub ordinary_producer: &'static str,
    pub contract: &'static str,
    subject: LiveInstantiationSubject,
}

pub const LIVE_INSTANTIATION_EVIDENCE: &[LiveInstantiationEvidence] = &[
    LiveInstantiationEvidence {
        id: "minecraft-waterlily-patch",
        ordinary_producer: "overworld waterlily random-patch feature",
        contract: "mclone-worldgen::feature::tests::placement::random_patch_places_lily_pad_on_projected_water_surface",
        subject: LiveInstantiationSubject::LilyPad,
    },
    LiveInstantiationEvidence {
        id: "mclone-mallard-natural-spawn",
        ordinary_producer: "wetland-qualified passive natural spawning",
        contract: "mclone-server::integrated::tests::entities::generated_wetland_mallard_flock_and_due_egg_survive_reload",
        subject: LiveInstantiationSubject::AdultMallard,
    },
    LiveInstantiationEvidence {
        id: "mclone-mallard-attended-hatch",
        ordinary_producer: "attended mallard nest hatching",
        contract: "mclone-server::entity::store::tests::covered_wetland_nest_pauses_resumes_hatches_once_and_roundtrips",
        subject: LiveInstantiationSubject::MallardDuckling,
    },
    LiveInstantiationEvidence {
        id: "mclone-mallard-nest-placement",
        ordinary_producer: "using a mallard egg on a valid covered wetland shore",
        contract: "mclone-server::entity::store::tests::covered_wetland_nest_pauses_resumes_hatches_once_and_roundtrips",
        subject: LiveInstantiationSubject::MallardNest,
    },
    LiveInstantiationEvidence {
        id: "mclone-mallard-egg-laying",
        ordinary_producer: "adult mallard egg production in suitable wetland habitat",
        contract: "mclone-server::entity::store::tests::mallard_egg_waits_for_wetland_habitat_then_spawns_distinct_item",
        subject: LiveInstantiationSubject::MallardEggItem,
    },
    LiveInstantiationEvidence {
        id: "mclone-mallard-feather-shedding",
        ordinary_producer: "periodic mallard feather shedding",
        contract: "mclone-server::entity::store::tests::mallard_calls_are_flock_suppressed_and_feathers_are_collectible_entities",
        subject: LiveInstantiationSubject::MallardFeatherItem,
    },
    LiveInstantiationEvidence {
        id: "mclone-mallard-field-guide-observation",
        ordinary_producer: "ordinary proximity, sound, trace, pickup, nest, and hatch observation paths",
        contract: "mclone-server::integrated::observe_mallard",
        subject: LiveInstantiationSubject::MallardObservation,
    },
];

pub fn playable_showcase_manifest(
    id: PlayableShowcaseId,
) -> Result<PlayableShowcaseManifest, PlayableShowcaseError> {
    let recipe = parse_and_validate_recipe(id, id.recipe_json())?;
    Ok(manifest_from_recipe(id, &recipe))
}

pub fn playable_showcase_memory_store(
    id: PlayableShowcaseId,
    identity: &ClientIdentity,
) -> Result<(PlayableShowcaseManifest, MemoryWorldStore), PlayableShowcaseError> {
    let mut store = MemoryWorldStore::new();
    let manifest = write_playable_showcase_to_store(&mut store, id, identity)?;
    Ok((manifest, store))
}

pub fn write_playable_showcase_to_store(
    store: &mut dyn WorldStore,
    id: PlayableShowcaseId,
    identity: &ClientIdentity,
) -> Result<PlayableShowcaseManifest, PlayableShowcaseError> {
    let recipe = parse_and_validate_recipe(id, id.recipe_json())?;
    write_authored_world_fixture_to_store(store, recipe.base_terrain.fixture_kind())?;
    write_world_records(store, &recipe)?;
    apply_block_patches(store, &recipe.block_patches)?;
    write_entities(store, id, &recipe.entities)?;
    write_player(store, identity, &recipe)?;
    store.flush()?;
    Ok(manifest_from_recipe(id, &recipe))
}

fn write_world_records(
    store: &mut dyn WorldStore,
    recipe: &PlayableShowcaseRecipe,
) -> Result<(), PlayableShowcaseError> {
    let mut metadata = WorldMetadata::new(
        recipe.seed,
        WorldGenerationProfile::authored_only(),
        WorldBehaviorProfile::Mutable,
        1,
    );
    metadata.day_time = recipe.day_time;
    store.save_world_metadata(&metadata)?;
    store.save_dimension(&DimensionRecord {
        key: DimensionKey::overworld(),
        codec_version: crate::DIMENSION_RECORD_VERSION,
        revision: 1,
        definition: DimensionDefinition::overworld(
            recipe.seed,
            WorldGenerationProfile::authored_only(),
        ),
    })?;
    Ok(())
}

fn parse_and_validate_recipe(
    expected_id: PlayableShowcaseId,
    source: &str,
) -> Result<PlayableShowcaseRecipe, PlayableShowcaseError> {
    let recipe: PlayableShowcaseRecipe = serde_json::from_str(source).map_err(|error| {
        PlayableShowcaseError::invalid(format!("invalid showcase JSON: {error}"))
    })?;
    validate_recipe(expected_id, &recipe)?;
    Ok(recipe)
}

fn validate_recipe(
    expected_id: PlayableShowcaseId,
    recipe: &PlayableShowcaseRecipe,
) -> Result<(), PlayableShowcaseError> {
    if recipe.schema_version != PLAYABLE_SHOWCASE_SCHEMA_VERSION {
        return Err(PlayableShowcaseError::invalid(format!(
            "showcase `{}` uses schema {}, expected {}",
            recipe.id, recipe.schema_version, PLAYABLE_SHOWCASE_SCHEMA_VERSION
        )));
    }
    if recipe.id != expected_id.label() {
        return Err(PlayableShowcaseError::invalid(format!(
            "showcase catalogue key `{}` does not match recipe id `{}`",
            expected_id.label(),
            recipe.id
        )));
    }
    if recipe.title.trim().is_empty() || recipe.revision == 0 {
        return Err(PlayableShowcaseError::invalid(
            "showcase title must be non-empty and revision must be positive",
        ));
    }
    if recipe.seed != recipe.base_terrain.fixture_kind().seed() {
        return Err(PlayableShowcaseError::invalid(format!(
            "showcase `{}` base terrain requires seed {}, got {}",
            recipe.id,
            recipe.base_terrain.fixture_kind().seed(),
            recipe.seed
        )));
    }
    validate_position("entry feet", recipe.entry.feet)?;
    validate_position("entry lookAt", recipe.entry.look_at)?;
    if recipe.entry.feet == recipe.entry.look_at {
        return Err(PlayableShowcaseError::invalid(
            "showcase entry feet and lookAt must differ",
        ));
    }
    if recipe.block_patches.len() > MAX_BLOCK_PATCHES {
        return Err(PlayableShowcaseError::invalid(format!(
            "showcase has {} block patches; maximum is {MAX_BLOCK_PATCHES}",
            recipe.block_patches.len()
        )));
    }
    if recipe.entities.len() > MAX_ENTITIES {
        return Err(PlayableShowcaseError::invalid(format!(
            "showcase has {} entities; maximum is {MAX_ENTITIES}",
            recipe.entities.len()
        )));
    }

    for patch in &recipe.block_patches {
        validate_block_position(patch.position)?;
        let expected = match patch.block.as_str() {
            "minecraft:lily_pad" => LiveInstantiationSubject::LilyPad,
            value => {
                return Err(PlayableShowcaseError::invalid(format!(
                    "showcase block `{value}` is not in the bounded block-patch allowlist"
                )));
            }
        };
        validate_evidence(&patch.live_instantiation, expected)?;
    }

    let mut entity_ids = BTreeSet::new();
    for entity in &recipe.entities {
        if entity.id.is_empty() || entity.id.len() > MAX_ENTITY_ID_BYTES {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase entity id `{}` must contain 1..={MAX_ENTITY_ID_BYTES} bytes",
                entity.id
            )));
        }
        if !entity_ids.insert(entity.id.as_str()) {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase entity id `{}` is duplicated",
                entity.id
            )));
        }
        validate_position(&format!("entity `{}`", entity.id), entity.position)?;
        if !entity.y_rot_degrees.is_finite() {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase entity `{}` has a non-finite rotation",
                entity.id
            )));
        }
        let expected = match &entity.state {
            ShowcaseEntityState::Mallard { age_ticks, .. } if *age_ticks < 2_400 => {
                LiveInstantiationSubject::MallardDuckling
            }
            ShowcaseEntityState::Mallard { .. } => LiveInstantiationSubject::AdultMallard,
            ShowcaseEntityState::MallardNest { .. } => LiveInstantiationSubject::MallardNest,
        };
        validate_evidence(&entity.live_instantiation, expected)?;
    }
    for entity in &recipe.entities {
        for parent in entity.state.parents().into_iter().flatten() {
            if parent == entity.id {
                return Err(PlayableShowcaseError::invalid(format!(
                    "showcase entity `{}` cannot be its own parent",
                    entity.id
                )));
            }
            if !entity_ids.contains(parent) {
                return Err(PlayableShowcaseError::invalid(format!(
                    "showcase entity `{}` references missing parent `{parent}`",
                    entity.id
                )));
            }
        }
        if let ShowcaseEntityState::MallardNest {
            incubation_progress,
            incubation_required,
            ..
        } = &entity.state
            && (*incubation_required == 0 || incubation_progress > incubation_required)
        {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase nest `{}` has invalid incubation progress {incubation_progress}/{incubation_required}",
                entity.id
            )));
        }
    }

    if recipe.player.selected_hotbar_slot >= 9 {
        return Err(PlayableShowcaseError::invalid(
            "showcase selectedHotbarSlot must be in 0..=8",
        ));
    }
    let mut slots = BTreeSet::new();
    for item in &recipe.player.inventory {
        if item.slot >= 36 || !slots.insert(item.slot) || item.count == 0 {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase inventory slot {} is duplicated, out of range, or empty",
                item.slot
            )));
        }
        let expected = match item.item.as_str() {
            "mclone:mallard_egg" => LiveInstantiationSubject::MallardEggItem,
            "mclone:mallard_feather" => LiveInstantiationSubject::MallardFeatherItem,
            value => {
                return Err(PlayableShowcaseError::invalid(format!(
                    "showcase inventory item `{value}` is not in the bounded item allowlist"
                )));
            }
        };
        validate_evidence(&item.live_instantiation, expected)?;
    }
    let mut observations = BTreeSet::new();
    for observation in &recipe.player.observations {
        if !observations.insert(observation.kind) {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase player observation `{:?}` is duplicated",
                observation.kind
            )));
        }
        validate_evidence(
            &observation.live_instantiation,
            LiveInstantiationSubject::MallardObservation,
        )?;
    }
    Ok(())
}

fn validate_evidence(
    id: &str,
    expected: LiveInstantiationSubject,
) -> Result<(), PlayableShowcaseError> {
    let Some(evidence) = LIVE_INSTANTIATION_EVIDENCE
        .iter()
        .find(|entry| entry.id == id)
    else {
        return Err(PlayableShowcaseError::invalid(format!(
            "showcase liveInstantiation `{id}` is not registered"
        )));
    };
    if evidence.subject != expected {
        return Err(PlayableShowcaseError::invalid(format!(
            "showcase liveInstantiation `{id}` is incompatible with this fact"
        )));
    }
    Ok(())
}

fn validate_position(label: &str, position: [f64; 3]) -> Result<(), PlayableShowcaseError> {
    if !position.into_iter().all(f64::is_finite)
        || position[0].abs() > 64.0
        || position[2].abs() > 64.0
        || position[1] < f64::from(AUTHORED_WORLD_MIN_Y)
        || position[1] >= f64::from(AUTHORED_WORLD_MIN_Y + AUTHORED_WORLD_HEIGHT)
    {
        return Err(PlayableShowcaseError::invalid(format!(
            "showcase {label} position {position:?} is outside the bounded authored world"
        )));
    }
    Ok(())
}

fn validate_block_position(position: [i32; 3]) -> Result<(), PlayableShowcaseError> {
    validate_position(
        "block patch",
        position.map(|coordinate| f64::from(coordinate)),
    )
}

fn apply_block_patches(
    store: &mut dyn WorldStore,
    patches: &[ShowcaseBlockPatch],
) -> Result<(), PlayableShowcaseError> {
    let dimension = DimensionKey::overworld();
    let mut chunks = BTreeMap::new();
    for patch in patches {
        let pos = ChunkPos::new(
            block_to_chunk_coord(patch.position[0]),
            block_to_chunk_coord(patch.position[2]),
        );
        let record = if let Some(record) = chunks.get_mut(&pos) {
            record
        } else {
            let record = store.load_chunk(&dimension, pos)?.ok_or_else(|| {
                PlayableShowcaseError::invalid(format!(
                    "showcase block patch at {:?} has no authored base chunk",
                    patch.position
                ))
            })?;
            chunks.entry(pos).or_insert(record)
        };
        let block_state = match patch.block.as_str() {
            "minecraft:lily_pad" => generated_block_state_id(LILY_PAD),
            _ => unreachable!("validated block patch"),
        };
        record.snapshot.patch_section_block(
            block_to_section_coord(patch.position[1]),
            patch.position[0].rem_euclid(16),
            patch.position[1].rem_euclid(16),
            patch.position[2].rem_euclid(16),
            block_state,
        );
    }
    for record in chunks.values_mut() {
        record.snapshot.revision = ChunkRevision(record.snapshot.revision.0.saturating_add(1));
        store.save_chunk(&dimension, record)?;
    }
    Ok(())
}

fn write_entities(
    store: &mut dyn WorldStore,
    showcase_id: PlayableShowcaseId,
    recipes: &[ShowcaseEntityRecipe],
) -> Result<(), PlayableShowcaseError> {
    let persistent_ids = recipes
        .iter()
        .map(|recipe| {
            (
                recipe.id.as_str(),
                persistent_entity_id(showcase_id, &recipe.id),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut chunks: BTreeMap<ChunkPos, Vec<EntitySaveRecord>> = BTreeMap::new();
    for recipe in recipes {
        let parents = recipe
            .state
            .parents()
            .map(|parent| parent.map(|id| *persistent_ids.get(id).expect("validated parent id")));
        let payload = match &recipe.state {
            ShowcaseEntityState::Mallard {
                age_ticks,
                egg_time,
                feather_time,
                call_time,
                ..
            } => EntitySavePayload::Mallard {
                egg_time: *egg_time,
                age_ticks: *age_ticks,
                parents,
                feather_time: *feather_time,
                call_time: *call_time,
            },
            ShowcaseEntityState::MallardNest {
                incubation_progress,
                incubation_required,
                ..
            } => EntitySavePayload::MallardNest {
                incubation_progress: *incubation_progress,
                incubation_required: *incubation_required,
                parents,
            },
        };
        let kind = match &recipe.state {
            ShowcaseEntityState::Mallard { .. } => "mclone:mallard",
            ShowcaseEntityState::MallardNest { .. } => "mclone:mallard_nest",
        };
        let position = Vec3d::new(recipe.position[0], recipe.position[1], recipe.position[2]);
        chunks
            .entry(ChunkPos::new(
                block_to_chunk_coord(position.x.floor() as i32),
                block_to_chunk_coord(position.z.floor() as i32),
            ))
            .or_default()
            .push(EntitySaveRecord {
                persistent_id: persistent_ids[recipe.id.as_str()],
                kind: kind.to_owned(),
                position,
                delta_movement: Vec3d::ZERO,
                y_rot_degrees: recipe.y_rot_degrees,
                x_rot_degrees: 0.0,
                rotation: Some(EntityRotation::IDENTITY),
                on_ground: recipe.on_ground,
                payload,
            });
    }
    let dimension = DimensionKey::overworld();
    for (pos, entities) in chunks {
        store.save_entity_chunk(&dimension, &EntityChunkRecord::new(pos, 1, entities))?;
    }
    Ok(())
}

fn write_player(
    store: &mut dyn WorldStore,
    identity: &ClientIdentity,
    recipe: &PlayableShowcaseRecipe,
) -> Result<(), PlayableShowcaseError> {
    let (y_rot_degrees, x_rot_degrees) = entry_rotation(&recipe.entry);
    let mut player = PlayerRecord::new(
        PlayerRecordKey::from_profile_id(identity.profile_id),
        1,
        identity.display_name.clone(),
        Vec3d::new(
            recipe.entry.feet[0],
            recipe.entry.feet[1],
            recipe.entry.feet[2],
        ),
    );
    player.y_rot_degrees = y_rot_degrees;
    player.x_rot_degrees = x_rot_degrees;
    player.on_ground = true;
    player.selected_hotbar_slot = recipe.player.selected_hotbar_slot;
    for item in &recipe.player.inventory {
        let kind = match item.item.as_str() {
            "mclone:mallard_egg" => ItemKind::MallardEgg,
            "mclone:mallard_feather" => ItemKind::MallardFeather,
            _ => unreachable!("validated inventory item"),
        };
        player.inventory[usize::from(item.slot)] = Some(ItemStackSnapshot {
            kind,
            count: item.count,
        });
    }
    for observation in &recipe.player.observations {
        player
            .mallard_field_guide
            .observe(observation.kind.protocol_kind());
    }
    store.save_player(&player)?;
    Ok(())
}

fn persistent_entity_id(showcase_id: PlayableShowcaseId, symbolic_id: &str) -> EntityPersistentId {
    let mut hasher = Sha256::new();
    hasher.update(b"mclone-playable-showcase-entity-v1\0");
    hasher.update(showcase_id.label().as_bytes());
    hasher.update(b"\0");
    hasher.update(symbolic_id.as_bytes());
    let digest = hasher.finalize();
    let most = u64::from_be_bytes(digest[0..8].try_into().expect("SHA-256 prefix"));
    let mut least = u64::from_be_bytes(digest[8..16].try_into().expect("SHA-256 prefix"));
    if most == 0 && least == 0 {
        least = 1;
    }
    EntityPersistentId::new(most, least)
}

fn entry_rotation(entry: &ShowcaseEntryRecipe) -> (f32, f32) {
    let eye = [
        entry.feet[0],
        entry.feet[1] + PLAYER_EYE_HEIGHT,
        entry.feet[2],
    ];
    let direction = [
        entry.look_at[0] - eye[0],
        entry.look_at[1] - eye[1],
        entry.look_at[2] - eye[2],
    ];
    let horizontal = direction[0].hypot(direction[2]);
    let y_rot = (-direction[0].atan2(direction[2])).to_degrees() as f32;
    let x_rot = (-direction[1].atan2(horizontal)).to_degrees() as f32;
    (y_rot, x_rot)
}

fn manifest_from_recipe(
    id: PlayableShowcaseId,
    recipe: &PlayableShowcaseRecipe,
) -> PlayableShowcaseManifest {
    let mut guide = MallardFieldGuideProgress::default();
    for observation in &recipe.player.observations {
        guide.observe(observation.kind.protocol_kind());
    }
    PlayableShowcaseManifest {
        schema_version: recipe.schema_version,
        id,
        title: recipe.title.clone(),
        revision: recipe.revision,
        seed: recipe.seed,
        world_generation_profile: WorldGenerationProfile::authored_only(),
        day_time: recipe.day_time,
        freeze_time: recipe.freeze_time,
        entry_feet: recipe.entry.feet,
        entry_eye: [
            recipe.entry.feet[0],
            recipe.entry.feet[1] + PLAYER_EYE_HEIGHT,
            recipe.entry.feet[2],
        ],
        entry_look_at: recipe.entry.look_at,
        entity_count: recipe.entities.len(),
        mallard_count: recipe
            .entities
            .iter()
            .filter(|entity| matches!(&entity.state, ShowcaseEntityState::Mallard { .. }))
            .count(),
        mallard_nest_count: recipe
            .entities
            .iter()
            .filter(|entity| matches!(&entity.state, ShowcaseEntityState::MallardNest { .. }))
            .count(),
        field_guide_bits: guide.bits(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlayableShowcaseRecipe {
    schema_version: u32,
    id: String,
    title: String,
    revision: u32,
    base_terrain: ShowcaseBaseTerrain,
    seed: i64,
    day_time: u64,
    freeze_time: bool,
    entry: ShowcaseEntryRecipe,
    block_patches: Vec<ShowcaseBlockPatch>,
    entities: Vec<ShowcaseEntityRecipe>,
    player: ShowcasePlayerRecipe,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseBaseTerrain {
    AuthoredIslandV1,
    MallardWetlandV1,
}

impl ShowcaseBaseTerrain {
    const fn fixture_kind(self) -> AuthoredWorldFixtureKind {
        match self {
            Self::AuthoredIslandV1 => AuthoredWorldFixtureKind::Island,
            Self::MallardWetlandV1 => AuthoredWorldFixtureKind::MallardWetland,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShowcaseEntryRecipe {
    feet: [f64; 3],
    look_at: [f64; 3],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShowcaseBlockPatch {
    position: [i32; 3],
    block: String,
    live_instantiation: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShowcaseEntityRecipe {
    id: String,
    position: [f64; 3],
    y_rot_degrees: f32,
    on_ground: bool,
    live_instantiation: String,
    state: ShowcaseEntityState,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum ShowcaseEntityState {
    Mallard {
        #[serde(rename = "ageTicks")]
        age_ticks: u32,
        parents: [Option<String>; 2],
        #[serde(rename = "eggTime")]
        egg_time: i32,
        #[serde(rename = "featherTime")]
        feather_time: i32,
        #[serde(rename = "callTime")]
        call_time: i32,
    },
    MallardNest {
        #[serde(rename = "incubationProgress")]
        incubation_progress: u32,
        #[serde(rename = "incubationRequired")]
        incubation_required: u32,
        parents: [Option<String>; 2],
    },
}

impl ShowcaseEntityState {
    fn parents(&self) -> [Option<&str>; 2] {
        let parents = match self {
            Self::Mallard { parents, .. } | Self::MallardNest { parents, .. } => parents,
        };
        [parents[0].as_deref(), parents[1].as_deref()]
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShowcasePlayerRecipe {
    selected_hotbar_slot: u8,
    inventory: Vec<ShowcaseInventoryRecipe>,
    observations: Vec<ShowcaseObservationRecipe>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShowcaseInventoryRecipe {
    slot: u8,
    item: String,
    count: u8,
    live_instantiation: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseObservationKind {
    Seen,
    HeardCall,
    FoundFeather,
    FoundTrack,
    FoundNest,
    WitnessedHatch,
}

impl ShowcaseObservationKind {
    const fn protocol_kind(self) -> MallardObservationKind {
        match self {
            Self::Seen => MallardObservationKind::Seen,
            Self::HeardCall => MallardObservationKind::HeardCall,
            Self::FoundFeather => MallardObservationKind::FoundFeather,
            Self::FoundTrack => MallardObservationKind::FoundTrack,
            Self::FoundNest => MallardObservationKind::FoundNest,
            Self::WitnessedHatch => MallardObservationKind::WitnessedHatch,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShowcaseObservationRecipe {
    kind: ShowcaseObservationKind,
    live_instantiation: String,
}

#[cfg(test)]
mod tests {
    use mclone_core::BlockPos;

    use super::*;

    #[test]
    fn catalogue_recipe_validates_and_compiles_deterministically() {
        let identity = ClientIdentity::test_default();
        let (first_manifest, first) =
            playable_showcase_memory_store(PlayableShowcaseId::MallardEcology, &identity).unwrap();
        let (second_manifest, second) =
            playable_showcase_memory_store(PlayableShowcaseId::MallardEcology, &identity).unwrap();
        assert_eq!(first_manifest, second_manifest);
        assert_eq!(first_manifest.entity_count, 4);
        assert_eq!(first_manifest.mallard_count, 3);
        assert_eq!(first_manifest.mallard_nest_count, 1);
        assert_eq!(
            first_manifest.field_guide_bits,
            MallardObservationKind::Seen.bit()
        );
        assert_eq!(first.world_metadata().unwrap().day_time, 6_000);
        assert!(first.world_metadata().unwrap().do_daylight_cycle);
        assert_eq!(
            first
                .dimension(&DimensionKey::overworld())
                .unwrap()
                .definition
                .generation_profile,
            WorldGenerationProfile::authored_only()
        );

        let first_entities = first.entity_chunk(ChunkPos::new(0, 0)).unwrap();
        let second_entities = second.entity_chunk(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(first_entities, second_entities);
        assert_eq!(first_entities.entities.len(), 4);
        assert!(
            first
                .player(&PlayerRecordKey::from_profile_id(identity.profile_id))
                .unwrap()
                .inventory
                .iter()
                .all(Option::is_none)
        );

        let center = first.chunk(ChunkPos::new(0, 0)).unwrap();
        let pos = BlockPos::new(8, 65, 11);
        let section = center
            .snapshot
            .sections
            .iter()
            .find(|section| section.section_y == block_to_section_coord(pos.y))
            .unwrap();
        assert_eq!(
            section.block_state_id_at(mclone_core::chunk_section_index(
                pos.x.rem_euclid(16),
                pos.y.rem_euclid(16),
                pos.z.rem_euclid(16),
            )),
            generated_block_state_id(LILY_PAD)
        );
    }

    #[test]
    fn schema_rejects_unknown_fields_and_bad_live_evidence() {
        let unknown = MALLARD_ECOLOGY_RECIPE.replacen(
            "\"schemaVersion\": 1,",
            "\"schemaVersion\": 1, \"script\": \"become-a-game-mode\",",
            1,
        );
        assert!(parse_and_validate_recipe(PlayableShowcaseId::MallardEcology, &unknown).is_err());

        let missing = MALLARD_ECOLOGY_RECIPE.replacen(
            "mclone-mallard-natural-spawn",
            "showcase-only-mallard",
            1,
        );
        let error = parse_and_validate_recipe(PlayableShowcaseId::MallardEcology, &missing)
            .unwrap_err()
            .to_string();
        assert!(error.contains("not registered"), "{error}");

        let wrong = MALLARD_ECOLOGY_RECIPE.replacen(
            "mclone-mallard-natural-spawn",
            "mclone-mallard-feather-shedding",
            1,
        );
        let error = parse_and_validate_recipe(PlayableShowcaseId::MallardEcology, &wrong)
            .unwrap_err()
            .to_string();
        assert!(error.contains("incompatible"), "{error}");
    }

    #[test]
    fn schema_rejects_duplicate_ids_and_missing_parent_references() {
        let duplicate = MALLARD_ECOLOGY_RECIPE.replacen("\"parent-b\"", "\"parent-a\"", 1);
        let error = parse_and_validate_recipe(PlayableShowcaseId::MallardEcology, &duplicate)
            .unwrap_err()
            .to_string();
        assert!(error.contains("duplicated"), "{error}");

        let missing = MALLARD_ECOLOGY_RECIPE.replacen(
            "\"parent-a\", \"parent-b\"",
            "\"missing\", \"parent-b\"",
            1,
        );
        let error = parse_and_validate_recipe(PlayableShowcaseId::MallardEcology, &missing)
            .unwrap_err()
            .to_string();
        assert!(error.contains("missing parent"), "{error}");
    }

    #[test]
    fn evidence_registry_has_unique_stable_ids_and_contracts() {
        let mut ids = BTreeSet::new();
        for evidence in LIVE_INSTANTIATION_EVIDENCE {
            assert!(ids.insert(evidence.id));
            assert!(!evidence.ordinary_producer.is_empty());
            assert!(!evidence.contract.is_empty());
        }
    }
}
