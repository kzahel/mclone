use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use mclone_core::{
    BlockPos, ChunkPos, ChunkRevision, Vec3d, block_to_chunk_coord, block_to_section_coord,
};
use mclone_protocol::{
    BeeBehavior, BeeFieldGuideProgress, BeeObservationKind, ClientIdentity, DeerBehavior,
    DeerFieldGuideProgress, DeerLifeStage, DeerObservationKind, DeerSex, DimensionKey,
    EntityRotation, ItemKind, ItemStackSnapshot, MallardFieldGuideProgress, MallardObservationKind,
    RabbitBehavior, RabbitFieldGuideProgress, RabbitLifeStage, RabbitObservationKind,
};
use mclone_worldgen::block::{
    FARMLAND_MOISTURE_0, FARMLAND_MOISTURE_7, LILY_PAD, generated_block_state_id, wheat_for_age,
};
use mclone_worldgen::structure_json::raw_block_state_for_canonical_key;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::persistence::{
    EntityChunkRecord, EntityPersistentId, EntitySavePayload, EntitySaveRecord, PlayerRecord,
    PlayerRecordKey, RabbitRefugeSaveRecord, WildlifeRemainsCause, WildlifeRemainsSpecies,
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
const DEER_FOREST_EDGE_RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/showcases/deer-forest-edge.showcase.json"
));
const BEE_POLLINATION_RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/showcases/bee-pollination.showcase.json"
));
const WHEAT_FARMING_RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/showcases/wheat-farming.showcase.json"
));
const KITCHEN_GARDEN_RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/showcases/kitchen-garden.showcase.json"
));
const RABBIT_BURROW_RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/mclone/showcases/rabbit-burrow.showcase.json"
));

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlayableShowcaseId {
    MallardEcology,
    DeerForestEdge,
    BeePollination,
    WheatFarming,
    KitchenGarden,
    RabbitBurrow,
}

impl PlayableShowcaseId {
    pub const ALL: [Self; 6] = [
        Self::MallardEcology,
        Self::DeerForestEdge,
        Self::BeePollination,
        Self::WheatFarming,
        Self::KitchenGarden,
        Self::RabbitBurrow,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::MallardEcology => "mallard-ecology",
            Self::DeerForestEdge => "deer-forest-edge",
            Self::BeePollination => "bee-pollination",
            Self::WheatFarming => "wheat-farming",
            Self::KitchenGarden => "kitchen-garden",
            Self::RabbitBurrow => "rabbit-burrow",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PlayableShowcaseError> {
        match value {
            "mallard-ecology" => Ok(Self::MallardEcology),
            "deer-forest-edge" => Ok(Self::DeerForestEdge),
            "bee-pollination" => Ok(Self::BeePollination),
            "wheat-farming" => Ok(Self::WheatFarming),
            "kitchen-garden" => Ok(Self::KitchenGarden),
            "rabbit-burrow" => Ok(Self::RabbitBurrow),
            _ => Err(PlayableShowcaseError::invalid(format!(
                "unknown playable showcase `{value}`; expected mallard-ecology, deer-forest-edge, bee-pollination, wheat-farming, kitchen-garden, or rabbit-burrow"
            ))),
        }
    }

    const fn recipe_json(self) -> &'static str {
        match self {
            Self::MallardEcology => MALLARD_ECOLOGY_RECIPE,
            Self::DeerForestEdge => DEER_FOREST_EDGE_RECIPE,
            Self::BeePollination => BEE_POLLINATION_RECIPE,
            Self::WheatFarming => WHEAT_FARMING_RECIPE,
            Self::KitchenGarden => KITCHEN_GARDEN_RECIPE,
            Self::RabbitBurrow => RABBIT_BURROW_RECIPE,
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
    pub deer_count: usize,
    pub deer_bed_count: usize,
    pub deer_field_guide_bits: u32,
    pub bee_count: usize,
    pub bee_nest_count: usize,
    pub bee_hotel_count: usize,
    pub bee_field_guide_bits: u32,
    pub rabbit_count: usize,
    pub rabbit_burrow_count: usize,
    pub rabbit_field_guide_bits: u32,
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
    Deer,
    DeerBed,
    HuntingSpear,
    DeerObservation,
    Bee,
    BeeNest,
    BeeHotel,
    Beeswax,
    BeeObservation,
    FarmSoil,
    WheatCrop,
    WoodenHoe,
    WheatSeeds,
    WheatItem,
    CarrotCrop,
    CarrotItem,
    OakFenceBlock,
    OakFenceItem,
    OakFenceGateBlock,
    OakFenceGateItem,
    Rabbit,
    RabbitBurrow,
    RabbitObservation,
    WildlifeRemains,
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
        id: "mclone-deer-natural-spawn",
        ordinary_producer: "forest-edge-qualified passive natural spawning",
        contract: "mclone-server::entity::spawning::live::tests::mclone_forest_edges_admit_bounded_deer_groups",
        subject: LiveInstantiationSubject::Deer,
    },
    LiveInstantiationEvidence {
        id: "mclone-deer-repeated-bed-sign",
        ordinary_producer: "sustained use of a covered deer bedding site",
        contract: "mclone-server::entity::store::tests::sustained_deer_bedding_creates_one_durable_sign",
        subject: LiveInstantiationSubject::DeerBed,
    },
    LiveInstantiationEvidence {
        id: "mclone-deer-hunting-spear",
        ordinary_producer: "fresh ordinary Mclone-world starter inventory",
        contract: "mclone-server::integrated::tests::entities::hunting_spear_damage_uses_fall_then_persistent_species_drops",
        subject: LiveInstantiationSubject::HuntingSpear,
    },
    LiveInstantiationEvidence {
        id: "mclone-deer-field-guide-observation",
        ordinary_producer: "ordinary deer proximity, state, sign, pickup, and harvest observation paths",
        contract: "mclone-server::integrated::route_deer_observations",
        subject: LiveInstantiationSubject::DeerObservation,
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
        id: "mclone-wildlife-natural-death-remains",
        ordinary_producer: "loaded wildlife old-age or starvation death",
        contract: "mclone-server::entity::store::tests::natural_mallard_death_creates_typed_durable_remains",
        subject: LiveInstantiationSubject::WildlifeRemains,
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
    LiveInstantiationEvidence {
        id: "mclone-bee-natural-colony",
        ordinary_producer: "flowering-habitat-qualified passive natural spawning",
        contract: "mclone-server::entity::spawning::live::tests::mclone_flowering_habitat_admits_one_bounded_bee_colony",
        subject: LiveInstantiationSubject::BeeNest,
    },
    LiveInstantiationEvidence {
        id: "mclone-bee-colony-member",
        ordinary_producer: "persistent natural or managed bee colony occupancy",
        contract: "mclone-server::entity::store::tests::bee_colony_forages_returns_persists_and_produces_bounded_work",
        subject: LiveInstantiationSubject::Bee,
    },
    LiveInstantiationEvidence {
        id: "mclone-bee-hotel-placement",
        ordinary_producer: "using a bee hotel on a supported flowering-habitat site",
        contract: "mclone-server::integrated::handle_bee_hotel_use_item_on_for_target",
        subject: LiveInstantiationSubject::BeeHotel,
    },
    LiveInstantiationEvidence {
        id: "mclone-bee-beeswax-harvest",
        ordinary_producer: "authoritative use of a fully worked colony",
        contract: "mclone-server::entity::store::tests::worked_bee_colony_yields_one_beeswax_and_resets_work",
        subject: LiveInstantiationSubject::Beeswax,
    },
    LiveInstantiationEvidence {
        id: "mclone-bee-field-guide-observation",
        ordinary_producer: "ordinary bee proximity, behavior, pollination, and harvest observation paths",
        contract: "mclone-server::integrated::route_bee_ecology_cues",
        subject: LiveInstantiationSubject::BeeObservation,
    },
    LiveInstantiationEvidence {
        id: "mclone-farmland-tilling",
        ordinary_producer: "authoritative use of a wooden hoe on grass or dirt",
        contract: "mclone-server::integrated::tests::debug_interactions::wooden_hoe_seed_and_harvest_form_an_authoritative_inventory_loop",
        subject: LiveInstantiationSubject::FarmSoil,
    },
    LiveInstantiationEvidence {
        id: "mclone-wheat-planting-and-growth",
        ordinary_producer: "seed planting followed by loaded-world random crop ticks",
        contract: "mclone-server::farming::tests::hydrated_farmland_advances_wheat_through_the_reference_probability",
        subject: LiveInstantiationSubject::WheatCrop,
    },
    LiveInstantiationEvidence {
        id: "mclone-farming-starter-hoe",
        ordinary_producer: "fresh ordinary Mclone-world starter inventory",
        contract: "mclone-server::integrated::tests::debug_interactions::wooden_hoe_seed_and_harvest_form_an_authoritative_inventory_loop",
        subject: LiveInstantiationSubject::WoodenHoe,
    },
    LiveInstantiationEvidence {
        id: "mclone-farming-starter-seeds",
        ordinary_producer: "fresh ordinary Mclone-world starter inventory",
        contract: "mclone-server::integrated::tests::debug_interactions::wooden_hoe_seed_and_harvest_form_an_authoritative_inventory_loop",
        subject: LiveInstantiationSubject::WheatSeeds,
    },
    LiveInstantiationEvidence {
        id: "mclone-mature-wheat-harvest",
        ordinary_producer: "authoritative mature wheat harvest",
        contract: "mclone-server::integrated::tests::debug_interactions::wooden_hoe_seed_and_harvest_form_an_authoritative_inventory_loop",
        subject: LiveInstantiationSubject::WheatItem,
    },
    LiveInstantiationEvidence {
        id: "mclone-carrot-planting-and-growth",
        ordinary_producer: "carrot planting followed by loaded-world random crop ticks",
        contract: "mclone-server::farming::tests::hydrated_farmland_advances_carrots_through_the_shared_crop_system",
        subject: LiveInstantiationSubject::CarrotCrop,
    },
    LiveInstantiationEvidence {
        id: "mclone-farming-starter-carrots",
        ordinary_producer: "fresh ordinary Mclone-world starter inventory",
        contract: "mclone-server::integrated::tests::debug_interactions::carrot_plant_and_harvest_use_the_shared_crop_and_item_drop_loop",
        subject: LiveInstantiationSubject::CarrotItem,
    },
    LiveInstantiationEvidence {
        id: "mclone-oak-fence-placement",
        ordinary_producer: "ordinary oak-fence item placement with neighbor refresh",
        contract: "mclone-server::integrated::tests::debug_interactions::fences_connect_and_gate_toggles_authoritatively",
        subject: LiveInstantiationSubject::OakFenceBlock,
    },
    LiveInstantiationEvidence {
        id: "mclone-farming-starter-fences",
        ordinary_producer: "fresh ordinary Mclone-world starter inventory",
        contract: "mclone-server::integrated::tests::debug_interactions::fences_connect_and_gate_toggles_authoritatively",
        subject: LiveInstantiationSubject::OakFenceItem,
    },
    LiveInstantiationEvidence {
        id: "mclone-oak-fence-gate-placement-and-use",
        ordinary_producer: "ordinary oak-fence-gate placement and manual block use",
        contract: "mclone-server::integrated::tests::debug_interactions::fences_connect_and_gate_toggles_authoritatively",
        subject: LiveInstantiationSubject::OakFenceGateBlock,
    },
    LiveInstantiationEvidence {
        id: "mclone-farming-starter-gates",
        ordinary_producer: "fresh ordinary Mclone-world starter inventory",
        contract: "mclone-server::integrated::tests::debug_interactions::fences_connect_and_gate_toggles_authoritatively",
        subject: LiveInstantiationSubject::OakFenceGateItem,
    },
    LiveInstantiationEvidence {
        id: "mclone-rabbit-natural-founder",
        ordinary_producer: "browse-and-bank-qualified passive natural spawning",
        contract: "mclone-server::entity::spawning::habitat::tests::rabbit_habitat_requires_browse_and_a_real_soil_bank",
        subject: LiveInstantiationSubject::Rabbit,
    },
    LiveInstantiationEvidence {
        id: "mclone-rabbit-warren-family",
        ordinary_producer: "carrot-fed adult rabbits sharing a persistent warren with capacity",
        contract: "mclone-server::entity::store::tests::rabbit_and_warren_identity_round_trip_together",
        subject: LiveInstantiationSubject::Rabbit,
    },
    LiveInstantiationEvidence {
        id: "mclone-rabbit-burrow-excavation",
        ordinary_producer: "founder rabbit excavation of a validated live soil bank",
        contract: "mclone-server::entity::mob::tests::founder_rabbit_chooses_a_real_bank_and_completes_one_dig",
        subject: LiveInstantiationSubject::RabbitBurrow,
    },
    LiveInstantiationEvidence {
        id: "mclone-rabbit-field-guide-observation",
        ordinary_producer: "ordinary rabbit proximity, burrow, threshold, raid, and family observation paths",
        contract: "mclone-server::integrated::route_rabbit_ecology_cues",
        subject: LiveInstantiationSubject::RabbitObservation,
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
            "minecraft:farmland[moisture=0]" | "minecraft:farmland[moisture=7]" => {
                LiveInstantiationSubject::FarmSoil
            }
            value if wheat_block_age(value).is_some() => LiveInstantiationSubject::WheatCrop,
            value if carrots_block_age(value).is_some() => LiveInstantiationSubject::CarrotCrop,
            value if is_oak_fence_block(value) => LiveInstantiationSubject::OakFenceBlock,
            value if is_oak_fence_gate_block(value) => LiveInstantiationSubject::OakFenceGateBlock,
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
            ShowcaseEntityState::Mallard { age_ticks, .. }
                if *age_ticks < crate::entity::MALLARD_GROWTH_REQUIRED_TICKS =>
            {
                LiveInstantiationSubject::MallardDuckling
            }
            ShowcaseEntityState::Mallard { .. } => LiveInstantiationSubject::AdultMallard,
            ShowcaseEntityState::MallardNest { .. } => LiveInstantiationSubject::MallardNest,
            ShowcaseEntityState::Deer { .. } => LiveInstantiationSubject::Deer,
            ShowcaseEntityState::DeerBed { .. } => LiveInstantiationSubject::DeerBed,
            ShowcaseEntityState::Bee { .. } => LiveInstantiationSubject::Bee,
            ShowcaseEntityState::BeeNest { .. } => LiveInstantiationSubject::BeeNest,
            ShowcaseEntityState::BeeHotel { .. } => LiveInstantiationSubject::BeeHotel,
            ShowcaseEntityState::Rabbit { .. } => LiveInstantiationSubject::Rabbit,
            ShowcaseEntityState::RabbitBurrow { .. } => LiveInstantiationSubject::RabbitBurrow,
            ShowcaseEntityState::WildlifeRemains { .. } => {
                LiveInstantiationSubject::WildlifeRemains
            }
        };
        validate_evidence(&entity.live_instantiation, expected)?;
    }
    let rabbit_burrow_ids = recipe
        .entities
        .iter()
        .filter_map(|entity| {
            matches!(&entity.state, ShowcaseEntityState::RabbitBurrow { .. })
                .then_some(entity.id.as_str())
        })
        .collect::<BTreeSet<_>>();
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
        if let ShowcaseEntityState::DeerBed { source } = &entity.state
            && !entity_ids.contains(source.as_str())
        {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase deer bed `{}` references missing source `{source}`",
                entity.id
            )));
        }
        if let ShowcaseEntityState::Deer {
            sex,
            life_stage,
            antlered,
            health,
            max_health,
            ..
        } = &entity.state
            && (*max_health == 0
                || *health > *max_health
                || (*antlered
                    && (*sex != ShowcaseDeerSex::Male
                        || *life_stage != ShowcaseDeerLifeStage::Adult)))
        {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase deer `{}` has impossible biological state",
                entity.id
            )));
        }
        if let ShowcaseEntityState::Bee { home, flower, .. } = &entity.state {
            if !entity_ids.contains(home.as_str()) {
                return Err(PlayableShowcaseError::invalid(format!(
                    "showcase bee `{}` references missing colony `{home}`",
                    entity.id
                )));
            }
            if let Some(flower) = flower {
                validate_block_position(*flower)?;
            }
        }
        if let ShowcaseEntityState::BeeNest {
            stored_work,
            work_capacity,
            ..
        }
        | ShowcaseEntityState::BeeHotel {
            stored_work,
            work_capacity,
            ..
        } = &entity.state
            && (*work_capacity == 0 || stored_work > work_capacity)
        {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase bee colony `{}` has invalid work {stored_work}/{work_capacity}",
                entity.id
            )));
        }
        if let ShowcaseEntityState::Rabbit {
            known_refuges,
            sheltered_in,
            dig_target,
            life_stage,
            age_ticks,
            health,
            max_health,
            behavior,
            ..
        } = &entity.state
        {
            let distinct_refuges = known_refuges.iter().collect::<BTreeSet<_>>();
            if known_refuges.len() > 3
                || distinct_refuges.len() != known_refuges.len()
                || known_refuges
                    .iter()
                    .any(|refuge| !rabbit_burrow_ids.contains(refuge.as_str()))
                || sheltered_in
                    .as_ref()
                    .is_some_and(|shelter| !known_refuges.iter().any(|refuge| refuge == shelter))
                || sheltered_in.is_some()
                    != matches!(
                        behavior.protocol(),
                        RabbitBehavior::EnterBurrow
                            | RabbitBehavior::Underground
                            | RabbitBehavior::Emerge
                    )
            {
                return Err(PlayableShowcaseError::invalid(format!(
                    "showcase rabbit `{}` has invalid bounded refuge references",
                    entity.id
                )));
            }
            if let Some(target) = dig_target {
                validate_block_position(*target)?;
            }
            if *max_health == 0
                || *health > *max_health
                || (*life_stage == ShowcaseRabbitLifeStage::Kit
                    && *age_ticks >= crate::entity::RABBIT_GROWTH_REQUIRED_TICKS)
                || (*life_stage == ShowcaseRabbitLifeStage::Adult
                    && *age_ticks < crate::entity::RABBIT_GROWTH_REQUIRED_TICKS)
            {
                return Err(PlayableShowcaseError::invalid(format!(
                    "showcase rabbit `{}` has impossible biological state",
                    entity.id
                )));
            }
        }
        if let ShowcaseEntityState::RabbitBurrow {
            capacity, damage, ..
        } = &entity.state
        {
            if *capacity == 0 || *capacity > 6 || *damage >= 3 {
                return Err(PlayableShowcaseError::invalid(format!(
                    "showcase rabbit burrow `{}` has invalid capacity or condition",
                    entity.id
                )));
            }
        }
        if let ShowcaseEntityState::WildlifeRemains { biomass, .. } = &entity.state
            && !(1..=2_000).contains(biomass)
        {
            return Err(PlayableShowcaseError::invalid(format!(
                "showcase wildlife remains `{}` has unbounded biomass {biomass}",
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
            "mclone:hunting_spear" => LiveInstantiationSubject::HuntingSpear,
            "mclone:bee_hotel" => LiveInstantiationSubject::BeeHotel,
            "mclone:beeswax" => LiveInstantiationSubject::Beeswax,
            "minecraft:wooden_hoe" => LiveInstantiationSubject::WoodenHoe,
            "minecraft:wheat_seeds" => LiveInstantiationSubject::WheatSeeds,
            "minecraft:wheat" => LiveInstantiationSubject::WheatItem,
            "minecraft:carrot" => LiveInstantiationSubject::CarrotItem,
            "minecraft:oak_fence" => LiveInstantiationSubject::OakFenceItem,
            "minecraft:oak_fence_gate" => LiveInstantiationSubject::OakFenceGateItem,
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
            if observation.kind.deer_protocol_kind().is_some() {
                LiveInstantiationSubject::DeerObservation
            } else if observation.kind.bee_protocol_kind().is_some() {
                LiveInstantiationSubject::BeeObservation
            } else if observation.kind.rabbit_protocol_kind().is_some() {
                LiveInstantiationSubject::RabbitObservation
            } else {
                LiveInstantiationSubject::MallardObservation
            },
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

fn wheat_block_age(block: &str) -> Option<u8> {
    let age = block
        .strip_prefix("minecraft:wheat[age=")?
        .strip_suffix(']')?
        .parse::<u8>()
        .ok()?;
    (age <= 7).then_some(age)
}

fn carrots_block_age(block: &str) -> Option<u8> {
    let age = block
        .strip_prefix("minecraft:carrots[age=")?
        .strip_suffix(']')?
        .parse::<u8>()
        .ok()?;
    (age <= 7).then_some(age)
}

fn is_oak_fence_block(block: &str) -> bool {
    block.starts_with("minecraft:oak_fence[")
        && raw_block_state_for_canonical_key(block)
            .and_then(mclone_worldgen::block::oak_fence_state)
            .is_some()
}

fn is_oak_fence_gate_block(block: &str) -> bool {
    block.starts_with("minecraft:oak_fence_gate[")
        && raw_block_state_for_canonical_key(block)
            .and_then(mclone_worldgen::block::oak_fence_gate_state)
            .is_some()
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
            "minecraft:farmland[moisture=0]" => generated_block_state_id(FARMLAND_MOISTURE_0),
            "minecraft:farmland[moisture=7]" => generated_block_state_id(FARMLAND_MOISTURE_7),
            value if wheat_block_age(value).is_some() => generated_block_state_id(
                wheat_for_age(wheat_block_age(value).expect("guarded wheat age"))
                    .expect("validated wheat age exists"),
            ),
            value
                if carrots_block_age(value).is_some()
                    || is_oak_fence_block(value)
                    || is_oak_fence_gate_block(value) =>
            {
                generated_block_state_id(
                    raw_block_state_for_canonical_key(value)
                        .expect("validated canonical garden state exists"),
                )
            }
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
    let entity_positions = recipes
        .iter()
        .map(|recipe| {
            (
                recipe.id.as_str(),
                BlockPos::containing(Vec3d::new(
                    recipe.position[0],
                    recipe.position[1],
                    recipe.position[2],
                )),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut chunks: BTreeMap<ChunkPos, Vec<EntitySaveRecord>> = BTreeMap::new();
    for recipe in recipes {
        let persistent_id = *persistent_ids
            .get(recipe.id.as_str())
            .expect("validated showcase entity id");
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
                sex: crate::entity::identity_mallard_sex(persistent_id),
                life_stage: if *age_ticks < crate::entity::MALLARD_GROWTH_REQUIRED_TICKS {
                    mclone_protocol::MallardLifeStage::Duckling
                } else {
                    mclone_protocol::MallardLifeStage::Adult
                },
                age_ticks: *age_ticks,
                parents,
                feather_time: *feather_time,
                call_time: *call_time,
                nest_target: None,
                lifespan_ticks: 0,
                energy: 800,
                deficit_ticks: 0,
                recent_intake: 0,
                reproductive_condition: 640,
                reproduction_cooldown: 0,
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
            ShowcaseEntityState::Deer {
                sex,
                life_stage,
                antlered,
                behavior,
                behavior_ticks,
                health,
                max_health,
                antler_shed_time,
            } => EntitySavePayload::Deer {
                sex: sex.protocol(),
                life_stage: life_stage.protocol(),
                antlered: *antlered,
                behavior: behavior.protocol(),
                behavior_ticks: *behavior_ticks,
                health: *health,
                max_health: *max_health,
                antler_shed_time: *antler_shed_time,
                age_ticks: if life_stage.protocol() == mclone_protocol::DeerLifeStage::Fawn {
                    0
                } else {
                    120_000
                },
                lifespan_ticks: 0,
                energy: 800,
                deficit_ticks: 0,
                recent_intake: 0,
                reproductive_condition: 640,
                reproduction_cooldown: 0,
            },
            ShowcaseEntityState::DeerBed { source } => EntitySavePayload::DeerBed {
                source: *persistent_ids
                    .get(source.as_str())
                    .expect("validated deer bed source"),
            },
            ShowcaseEntityState::Bee {
                home,
                flower,
                behavior,
                behavior_ticks,
                carrying_pollen,
            } => EntitySavePayload::Bee {
                home: *persistent_ids
                    .get(home.as_str())
                    .expect("validated bee colony reference"),
                flower: flower.map(|position| {
                    mclone_core::BlockPos::new(position[0], position[1], position[2])
                }),
                behavior: behavior.protocol(),
                behavior_ticks: *behavior_ticks,
                carrying_pollen: *carrying_pollen,
            },
            ShowcaseEntityState::BeeNest {
                colonized,
                stored_work,
                work_capacity,
                spread_cooldown,
            }
            | ShowcaseEntityState::BeeHotel {
                colonized,
                stored_work,
                work_capacity,
                spread_cooldown,
            } => EntitySavePayload::BeeColony {
                colonized: *colonized,
                stored_work: *stored_work,
                work_capacity: *work_capacity,
                spread_cooldown: *spread_cooldown,
            },
            ShowcaseEntityState::Rabbit {
                known_refuges,
                sheltered_in,
                dig_target,
                life_stage,
                age_ticks,
                behavior,
                behavior_ticks,
                health,
                max_health,
                love_ticks,
                breed_cooldown,
                raid_cooldown,
                ..
            } => EntitySavePayload::Rabbit {
                known_refuges: {
                    let mut saved = [None; 3];
                    for (slot, refuge) in saved.iter_mut().zip(known_refuges) {
                        *slot = Some(RabbitRefugeSaveRecord {
                            persistent_id: *persistent_ids
                                .get(refuge.as_str())
                                .expect("validated rabbit burrow reference"),
                            last_known_position: Some(
                                *entity_positions
                                    .get(refuge.as_str())
                                    .expect("validated rabbit burrow position"),
                            ),
                            revision: None,
                            last_confirmed_tick: 0,
                            familiarity: 1,
                        });
                    }
                    saved
                },
                sheltered_in: sheltered_in.as_ref().map(|shelter| {
                    *persistent_ids
                        .get(shelter.as_str())
                        .expect("validated current rabbit shelter")
                }),
                dig_target: dig_target.map(|target| BlockPos::new(target[0], target[1], target[2])),
                next_decision_tick: 0,
                decision_generation: 0,
                dig_cooldown: 0,
                life_stage: life_stage.protocol(),
                age_ticks: *age_ticks,
                parents,
                behavior: behavior.protocol(),
                behavior_ticks: *behavior_ticks,
                health: *health,
                max_health: *max_health,
                love_ticks: *love_ticks,
                breed_cooldown: *breed_cooldown,
                raid_cooldown: *raid_cooldown,
                lifespan_ticks: 0,
                energy: 800,
                deficit_ticks: 0,
                recent_intake: 0,
                reproductive_condition: 640,
            },
            ShowcaseEntityState::RabbitBurrow {
                capacity,
                disturbance_ticks,
                damage,
                last_used_tick,
            } => EntitySavePayload::RabbitBurrow {
                capacity: *capacity,
                disturbance_ticks: *disturbance_ticks,
                damage: *damage,
                last_used_tick: *last_used_tick,
            },
            ShowcaseEntityState::WildlifeRemains {
                source_species,
                biomass,
                cause,
                creation_tick,
            } => EntitySavePayload::WildlifeRemains {
                source_species: source_species.persistence(),
                source: persistent_entity_id(showcase_id, &format!("{}:source", recipe.id)),
                biomass: *biomass,
                cause: cause.persistence(),
                creation_tick: *creation_tick,
                decay_remainder: 0,
            },
        };
        let kind = match &recipe.state {
            ShowcaseEntityState::Mallard { .. } => "mclone:mallard",
            ShowcaseEntityState::MallardNest { .. } => "mclone:mallard_nest",
            ShowcaseEntityState::Deer { .. } => "mclone:deer",
            ShowcaseEntityState::DeerBed { .. } => "mclone:deer_bed",
            ShowcaseEntityState::Bee { .. } => "mclone:bee",
            ShowcaseEntityState::BeeNest { .. } => "mclone:bee_nest",
            ShowcaseEntityState::BeeHotel { .. } => "mclone:bee_hotel",
            ShowcaseEntityState::Rabbit { .. } => "mclone:rabbit",
            ShowcaseEntityState::RabbitBurrow { .. } => "mclone:rabbit_burrow",
            ShowcaseEntityState::WildlifeRemains { .. } => "mclone:wildlife_remains",
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
                animation: None,
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
            "mclone:hunting_spear" => ItemKind::HuntingSpear,
            "mclone:bee_hotel" => ItemKind::BeeHotel,
            "mclone:beeswax" => ItemKind::Beeswax,
            "minecraft:wooden_hoe" => ItemKind::WoodenHoe,
            "minecraft:wheat_seeds" => ItemKind::WheatSeeds,
            "minecraft:wheat" => ItemKind::Wheat,
            "minecraft:carrot" => ItemKind::Carrot,
            "minecraft:oak_fence" => ItemKind::OakFence,
            "minecraft:oak_fence_gate" => ItemKind::OakFenceGate,
            _ => unreachable!("validated inventory item"),
        };
        player.inventory[usize::from(item.slot)] = Some(ItemStackSnapshot {
            kind,
            count: item.count,
        });
    }
    for observation in &recipe.player.observations {
        if let Some(kind) = observation.kind.mallard_protocol_kind() {
            player.mallard_field_guide.observe(kind);
        }
        if let Some(kind) = observation.kind.deer_protocol_kind() {
            player.deer_field_guide.observe(kind);
        }
        if let Some(kind) = observation.kind.bee_protocol_kind() {
            player.bee_field_guide.observe(kind);
        }
        if let Some(kind) = observation.kind.rabbit_protocol_kind() {
            player.rabbit_field_guide.observe(kind);
        }
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
    let mut deer_guide = DeerFieldGuideProgress::default();
    let mut bee_guide = BeeFieldGuideProgress::default();
    let mut rabbit_guide = RabbitFieldGuideProgress::default();
    for observation in &recipe.player.observations {
        if let Some(kind) = observation.kind.mallard_protocol_kind() {
            guide.observe(kind);
        }
        if let Some(kind) = observation.kind.deer_protocol_kind() {
            deer_guide.observe(kind);
        }
        if let Some(kind) = observation.kind.bee_protocol_kind() {
            bee_guide.observe(kind);
        }
        if let Some(kind) = observation.kind.rabbit_protocol_kind() {
            rabbit_guide.observe(kind);
        }
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
        deer_count: recipe
            .entities
            .iter()
            .filter(|entity| matches!(&entity.state, ShowcaseEntityState::Deer { .. }))
            .count(),
        deer_bed_count: recipe
            .entities
            .iter()
            .filter(|entity| matches!(&entity.state, ShowcaseEntityState::DeerBed { .. }))
            .count(),
        deer_field_guide_bits: deer_guide.bits(),
        bee_count: recipe
            .entities
            .iter()
            .filter(|entity| matches!(&entity.state, ShowcaseEntityState::Bee { .. }))
            .count(),
        bee_nest_count: recipe
            .entities
            .iter()
            .filter(|entity| matches!(&entity.state, ShowcaseEntityState::BeeNest { .. }))
            .count(),
        bee_hotel_count: recipe
            .entities
            .iter()
            .filter(|entity| matches!(&entity.state, ShowcaseEntityState::BeeHotel { .. }))
            .count(),
        bee_field_guide_bits: bee_guide.bits(),
        rabbit_count: recipe
            .entities
            .iter()
            .filter(|entity| matches!(&entity.state, ShowcaseEntityState::Rabbit { .. }))
            .count(),
        rabbit_burrow_count: recipe
            .entities
            .iter()
            .filter(|entity| matches!(&entity.state, ShowcaseEntityState::RabbitBurrow { .. }))
            .count(),
        rabbit_field_guide_bits: rabbit_guide.bits(),
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
    DeerForestEdgeV1,
    BeeFloweringMeadowV1,
    WheatFarmingV1,
    RabbitMeadowV1,
}

impl ShowcaseBaseTerrain {
    const fn fixture_kind(self) -> AuthoredWorldFixtureKind {
        match self {
            Self::AuthoredIslandV1 => AuthoredWorldFixtureKind::Island,
            Self::MallardWetlandV1 => AuthoredWorldFixtureKind::MallardWetland,
            Self::DeerForestEdgeV1 => AuthoredWorldFixtureKind::DeerForestEdge,
            Self::BeeFloweringMeadowV1 => AuthoredWorldFixtureKind::BeeFloweringMeadow,
            Self::WheatFarmingV1 => AuthoredWorldFixtureKind::WheatFarming,
            Self::RabbitMeadowV1 => AuthoredWorldFixtureKind::RabbitMeadow,
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
    Deer {
        sex: ShowcaseDeerSex,
        #[serde(rename = "lifeStage")]
        life_stage: ShowcaseDeerLifeStage,
        antlered: bool,
        behavior: ShowcaseDeerBehavior,
        #[serde(rename = "behaviorTicks")]
        behavior_ticks: u32,
        health: u8,
        #[serde(rename = "maxHealth")]
        max_health: u8,
        #[serde(rename = "antlerShedTime")]
        antler_shed_time: i32,
    },
    DeerBed {
        source: String,
    },
    Bee {
        home: String,
        flower: Option<[i32; 3]>,
        behavior: ShowcaseBeeBehavior,
        #[serde(rename = "behaviorTicks")]
        behavior_ticks: u32,
        #[serde(rename = "carryingPollen")]
        carrying_pollen: bool,
    },
    BeeNest {
        colonized: bool,
        #[serde(rename = "storedWork")]
        stored_work: u32,
        #[serde(rename = "workCapacity")]
        work_capacity: u32,
        #[serde(rename = "spreadCooldown")]
        spread_cooldown: u32,
    },
    BeeHotel {
        colonized: bool,
        #[serde(rename = "storedWork")]
        stored_work: u32,
        #[serde(rename = "workCapacity")]
        work_capacity: u32,
        #[serde(rename = "spreadCooldown")]
        spread_cooldown: u32,
    },
    Rabbit {
        #[serde(rename = "knownRefuges")]
        known_refuges: Vec<String>,
        #[serde(rename = "shelteredIn")]
        sheltered_in: Option<String>,
        #[serde(rename = "digTarget")]
        dig_target: Option<[i32; 3]>,
        #[serde(rename = "lifeStage")]
        life_stage: ShowcaseRabbitLifeStage,
        #[serde(rename = "ageTicks")]
        age_ticks: u32,
        parents: [Option<String>; 2],
        behavior: ShowcaseRabbitBehavior,
        #[serde(rename = "behaviorTicks")]
        behavior_ticks: u32,
        health: u8,
        #[serde(rename = "maxHealth")]
        max_health: u8,
        #[serde(rename = "loveTicks")]
        love_ticks: u32,
        #[serde(rename = "breedCooldown")]
        breed_cooldown: u32,
        #[serde(rename = "raidCooldown")]
        raid_cooldown: u32,
    },
    RabbitBurrow {
        capacity: u8,
        #[serde(rename = "disturbanceTicks")]
        disturbance_ticks: u32,
        damage: u8,
        #[serde(rename = "lastUsedTick")]
        last_used_tick: u64,
    },
    WildlifeRemains {
        #[serde(rename = "sourceSpecies")]
        source_species: ShowcaseWildlifeRemainsSpecies,
        biomass: u32,
        cause: ShowcaseWildlifeRemainsCause,
        #[serde(rename = "creationTick")]
        creation_tick: u64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseWildlifeRemainsSpecies {
    Rabbit,
    Deer,
    Mallard,
}

impl ShowcaseWildlifeRemainsSpecies {
    const fn persistence(self) -> WildlifeRemainsSpecies {
        match self {
            Self::Rabbit => WildlifeRemainsSpecies::Rabbit,
            Self::Deer => WildlifeRemainsSpecies::Deer,
            Self::Mallard => WildlifeRemainsSpecies::Mallard,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseWildlifeRemainsCause {
    OldAge,
    Starvation,
}

impl ShowcaseWildlifeRemainsCause {
    const fn persistence(self) -> WildlifeRemainsCause {
        match self {
            Self::OldAge => WildlifeRemainsCause::OldAge,
            Self::Starvation => WildlifeRemainsCause::Starvation,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseRabbitLifeStage {
    Kit,
    Adult,
}

impl ShowcaseRabbitLifeStage {
    const fn protocol(self) -> RabbitLifeStage {
        match self {
            Self::Kit => RabbitLifeStage::Kit,
            Self::Adult => RabbitLifeStage::Adult,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseRabbitBehavior {
    Idle,
    Hop,
    Dig,
    Emerge,
    Forage,
    Raid,
    Flee,
    EnterBurrow,
    Underground,
    Courtship,
}

impl ShowcaseRabbitBehavior {
    const fn protocol(self) -> RabbitBehavior {
        match self {
            Self::Idle => RabbitBehavior::Idle,
            Self::Hop => RabbitBehavior::Hop,
            Self::Dig => RabbitBehavior::Dig,
            Self::Emerge => RabbitBehavior::Emerge,
            Self::Forage => RabbitBehavior::Forage,
            Self::Raid => RabbitBehavior::Raid,
            Self::Flee => RabbitBehavior::Flee,
            Self::EnterBurrow => RabbitBehavior::EnterBurrow,
            Self::Underground => RabbitBehavior::Underground,
            Self::Courtship => RabbitBehavior::Courtship,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseBeeBehavior {
    Hover,
    FlyToFlower,
    Forage,
    ReturnHome,
    AtNest,
}

impl ShowcaseBeeBehavior {
    const fn protocol(self) -> BeeBehavior {
        match self {
            Self::Hover => BeeBehavior::Hover,
            Self::FlyToFlower => BeeBehavior::FlyToFlower,
            Self::Forage => BeeBehavior::Forage,
            Self::ReturnHome => BeeBehavior::ReturnHome,
            Self::AtNest => BeeBehavior::AtNest,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseDeerSex {
    Female,
    Male,
}

impl ShowcaseDeerSex {
    const fn protocol(self) -> DeerSex {
        match self {
            Self::Female => DeerSex::Female,
            Self::Male => DeerSex::Male,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseDeerLifeStage {
    Fawn,
    Adult,
}

impl ShowcaseDeerLifeStage {
    const fn protocol(self) -> DeerLifeStage {
        match self {
            Self::Fawn => DeerLifeStage::Fawn,
            Self::Adult => DeerLifeStage::Adult,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ShowcaseDeerBehavior {
    Idle,
    Walk,
    Graze,
    Drink,
    Alert,
    Flee,
    LieDown,
    Bedded,
    StandUp,
    Hit,
    Fall,
}

impl ShowcaseDeerBehavior {
    const fn protocol(self) -> DeerBehavior {
        match self {
            Self::Idle => DeerBehavior::Idle,
            Self::Walk => DeerBehavior::Walk,
            Self::Graze => DeerBehavior::Graze,
            Self::Drink => DeerBehavior::Drink,
            Self::Alert => DeerBehavior::Alert,
            Self::Flee => DeerBehavior::Flee,
            Self::LieDown => DeerBehavior::LieDown,
            Self::Bedded => DeerBehavior::Bedded,
            Self::StandUp => DeerBehavior::StandUp,
            Self::Hit => DeerBehavior::Hit,
            Self::Fall => DeerBehavior::Fall,
        }
    }
}

impl ShowcaseEntityState {
    fn parents(&self) -> [Option<&str>; 2] {
        let parents = match self {
            Self::Mallard { parents, .. } | Self::MallardNest { parents, .. } => parents,
            Self::Deer { .. }
            | Self::DeerBed { .. }
            | Self::Bee { .. }
            | Self::BeeNest { .. }
            | Self::BeeHotel { .. }
            | Self::RabbitBurrow { .. }
            | Self::WildlifeRemains { .. } => return [None, None],
            Self::Rabbit { parents, .. } => parents,
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
    DeerSeen,
    DeerFoundSign,
    DeerWitnessedAlert,
    DeerWitnessedFlee,
    DeerFoundAntler,
    DeerHarvested,
    BeeSeen,
    BeeFoundNest,
    BeeWitnessedForage,
    BeeWitnessedReturn,
    BeeWitnessedPollination,
    BeeCollectedBeeswax,
    RabbitSeen,
    RabbitFoundBurrow,
    RabbitWitnessedDig,
    RabbitWitnessedThresholdUse,
    RabbitWitnessedRaid,
    RabbitWitnessedFamily,
}

impl ShowcaseObservationKind {
    const fn mallard_protocol_kind(self) -> Option<MallardObservationKind> {
        Some(match self {
            Self::Seen => MallardObservationKind::Seen,
            Self::HeardCall => MallardObservationKind::HeardCall,
            Self::FoundFeather => MallardObservationKind::FoundFeather,
            Self::FoundTrack => MallardObservationKind::FoundTrack,
            Self::FoundNest => MallardObservationKind::FoundNest,
            Self::WitnessedHatch => MallardObservationKind::WitnessedHatch,
            _ => return None,
        })
    }

    const fn deer_protocol_kind(self) -> Option<DeerObservationKind> {
        Some(match self {
            Self::DeerSeen => DeerObservationKind::Seen,
            Self::DeerFoundSign => DeerObservationKind::FoundSign,
            Self::DeerWitnessedAlert => DeerObservationKind::WitnessedAlert,
            Self::DeerWitnessedFlee => DeerObservationKind::WitnessedFlee,
            Self::DeerFoundAntler => DeerObservationKind::FoundAntler,
            Self::DeerHarvested => DeerObservationKind::Harvested,
            _ => return None,
        })
    }

    const fn bee_protocol_kind(self) -> Option<BeeObservationKind> {
        Some(match self {
            Self::BeeSeen => BeeObservationKind::Seen,
            Self::BeeFoundNest => BeeObservationKind::FoundNest,
            Self::BeeWitnessedForage => BeeObservationKind::WitnessedForage,
            Self::BeeWitnessedReturn => BeeObservationKind::WitnessedReturn,
            Self::BeeWitnessedPollination => BeeObservationKind::WitnessedPollination,
            Self::BeeCollectedBeeswax => BeeObservationKind::CollectedBeeswax,
            _ => return None,
        })
    }

    const fn rabbit_protocol_kind(self) -> Option<RabbitObservationKind> {
        Some(match self {
            Self::RabbitSeen => RabbitObservationKind::Seen,
            Self::RabbitFoundBurrow => RabbitObservationKind::FoundBurrow,
            Self::RabbitWitnessedDig => RabbitObservationKind::WitnessedDig,
            Self::RabbitWitnessedThresholdUse => RabbitObservationKind::WitnessedThresholdUse,
            Self::RabbitWitnessedRaid => RabbitObservationKind::WitnessedRaid,
            Self::RabbitWitnessedFamily => RabbitObservationKind::WitnessedFamily,
            _ => return None,
        })
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
        assert_eq!(first_manifest.revision, 3);
        assert_eq!(first_manifest.entity_count, 5);
        assert_eq!(first_manifest.mallard_count, 3);
        assert_eq!(first_manifest.mallard_nest_count, 1);
        assert_eq!(
            first_manifest.field_guide_bits,
            MallardObservationKind::Seen.bit()
        );
        assert_eq!(first_manifest.deer_count, 0);
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
        assert_eq!(first_entities.entities.len(), 5);
        assert!(first_entities.entities.iter().any(|entity| {
            entity.kind == "mclone:wildlife_remains"
                && matches!(
                    entity.payload,
                    EntitySavePayload::WildlifeRemains {
                        source_species: WildlifeRemainsSpecies::Mallard,
                        biomass: 180,
                        cause: WildlifeRemainsCause::OldAge,
                        ..
                    }
                )
        }));
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
    fn deer_forest_edge_recipe_is_data_only_and_deterministic() {
        let identity = ClientIdentity::test_default();
        let (manifest, store) =
            playable_showcase_memory_store(PlayableShowcaseId::DeerForestEdge, &identity).unwrap();
        assert_eq!(manifest.revision, 1);
        assert_eq!(manifest.seed, 17_504);
        assert_eq!(manifest.entity_count, 4);
        assert_eq!(manifest.deer_count, 3);
        assert_eq!(manifest.deer_bed_count, 1);
        assert_eq!(
            manifest.deer_field_guide_bits,
            DeerObservationKind::Seen.bit() | DeerObservationKind::FoundSign.bit()
        );
        assert_eq!(
            [
                ChunkPos::new(-1, 0),
                ChunkPos::new(0, 0),
                ChunkPos::new(0, 1)
            ]
            .into_iter()
            .flat_map(|chunk| store.entity_chunk(chunk).unwrap().entities.iter())
            .filter(|entity| entity.kind == "mclone:deer")
            .count(),
            3
        );
        let bed_entities = store.entity_chunk(ChunkPos::new(-1, 0)).unwrap();
        assert!(bed_entities.entities.iter().any(|entity| {
            entity.kind == "mclone:deer_bed"
                && matches!(entity.payload, EntitySavePayload::DeerBed { .. })
        }));
        let player = store
            .player(&PlayerRecordKey::from_profile_id(identity.profile_id))
            .unwrap();
        assert_eq!(
            player.inventory[0],
            Some(ItemStackSnapshot {
                kind: ItemKind::HuntingSpear,
                count: 1,
            })
        );
        assert_eq!(
            player.deer_field_guide.bits(),
            manifest.deer_field_guide_bits
        );
    }

    #[test]
    fn bee_pollination_recipe_is_data_only_and_deterministic() {
        let identity = ClientIdentity::test_default();
        let (manifest, store) =
            playable_showcase_memory_store(PlayableShowcaseId::BeePollination, &identity).unwrap();
        assert_eq!(manifest.revision, 2);
        assert_eq!(manifest.seed, 17_505);
        assert_eq!(manifest.entity_count, 4);
        assert_eq!(manifest.bee_count, 3);
        assert_eq!(manifest.bee_nest_count, 1);
        assert_eq!(manifest.bee_hotel_count, 0);
        assert_eq!(
            manifest.bee_field_guide_bits,
            BeeObservationKind::Seen.bit() | BeeObservationKind::FoundNest.bit()
        );
        let entities = store.entity_chunk(ChunkPos::new(0, 0)).unwrap();
        assert_eq!(
            entities
                .entities
                .iter()
                .filter(|entity| entity.kind == "mclone:bee")
                .count(),
            3
        );
        assert!(entities.entities.iter().any(|entity| {
            entity.kind == "mclone:bee_nest"
                && matches!(
                    entity.payload,
                    EntitySavePayload::BeeColony {
                        colonized: true,
                        stored_work: 2,
                        ..
                    }
                )
        }));
        let player = store
            .player(&PlayerRecordKey::from_profile_id(identity.profile_id))
            .unwrap();
        assert_eq!(
            player.inventory[8],
            Some(ItemStackSnapshot {
                kind: ItemKind::BeeHotel,
                count: 1,
            })
        );
        assert_eq!(player.bee_field_guide.bits(), manifest.bee_field_guide_bits);
    }

    #[test]
    fn wheat_farming_recipe_is_data_only_and_uses_live_crop_states() {
        let identity = ClientIdentity::test_default();
        let (manifest, store) =
            playable_showcase_memory_store(PlayableShowcaseId::WheatFarming, &identity).unwrap();
        assert_eq!(manifest.revision, 3);
        assert_eq!(manifest.seed, 17_506);
        assert_eq!(manifest.entity_count, 0);
        assert_eq!(manifest.entry_feet, [8.5, 64.0, 14.5]);
        assert_eq!(manifest.entry_eye, [8.5, 65.62, 14.5]);
        assert_eq!(manifest.entry_look_at, [11.5, 64.5, 10.5]);
        let center = store.chunk(ChunkPos::new(0, 0)).unwrap();
        let block_at = |pos: BlockPos| {
            center
                .snapshot
                .sections
                .iter()
                .find(|section| section.section_y == block_to_section_coord(pos.y))
                .map(|section| {
                    section.block_state_id_at(mclone_core::chunk_section_index(
                        pos.x.rem_euclid(16),
                        pos.y.rem_euclid(16),
                        pos.z.rem_euclid(16),
                    ))
                })
                .unwrap_or(mclone_core::AIR_BLOCK_STATE_ID)
        };
        assert_eq!(
            block_at(BlockPos::new(5, 63, 5)),
            generated_block_state_id(FARMLAND_MOISTURE_0)
        );
        assert_eq!(
            block_at(BlockPos::new(6, 63, 5)),
            generated_block_state_id(FARMLAND_MOISTURE_7)
        );
        assert_eq!(
            block_at(BlockPos::new(9, 64, 6)),
            generated_block_state_id(wheat_for_age(7).unwrap())
        );
        assert_eq!(
            block_at(BlockPos::new(11, 64, 10)),
            generated_block_state_id(wheat_for_age(7).unwrap())
        );
        let player = store
            .player(&PlayerRecordKey::from_profile_id(identity.profile_id))
            .unwrap();
        assert_eq!(
            player.inventory[7],
            Some(ItemStackSnapshot {
                kind: ItemKind::WoodenHoe,
                count: 1,
            })
        );
        assert_eq!(
            player.inventory[8],
            Some(ItemStackSnapshot {
                kind: ItemKind::WheatSeeds,
                count: 8,
            })
        );
    }

    #[test]
    fn kitchen_garden_recipe_compiles_real_boundary_and_crop_facts() {
        let identity = ClientIdentity::test_default();
        let (manifest, store) =
            playable_showcase_memory_store(PlayableShowcaseId::KitchenGarden, &identity).unwrap();
        assert_eq!(manifest.revision, 1);
        assert_eq!(manifest.seed, 17_506);
        assert_eq!(manifest.entity_count, 0);
        assert_eq!(manifest.entry_feet, [2.5, 64.0, 8.5]);
        assert_eq!(manifest.entry_eye, [2.5, 65.62, 8.5]);
        assert_eq!(manifest.entry_look_at, [5.5, 64.75, 8.5]);

        let center = store.chunk(ChunkPos::new(0, 0)).unwrap();
        let block_at = |pos: BlockPos| {
            center
                .snapshot
                .sections
                .iter()
                .find(|section| section.section_y == block_to_section_coord(pos.y))
                .map(|section| {
                    section.block_state_id_at(mclone_core::chunk_section_index(
                        pos.x.rem_euclid(16),
                        pos.y.rem_euclid(16),
                        pos.z.rem_euclid(16),
                    ))
                })
                .unwrap_or(mclone_core::AIR_BLOCK_STATE_ID)
        };
        let west_gate = block_at(BlockPos::new(5, 64, 8));
        let gate = mclone_worldgen::block::oak_fence_gate_state(west_gate.0 as u16).unwrap();
        assert_eq!(gate.facing, 1);
        assert!(!gate.open);
        let adjacent =
            mclone_worldgen::block::oak_fence_state(block_at(BlockPos::new(5, 64, 7)).0 as u16)
                .unwrap();
        assert!(adjacent.north && adjacent.south);
        assert_eq!(
            block_at(BlockPos::new(6, 64, 9)),
            generated_block_state_id(mclone_worldgen::block::CARROTS_AGE_7)
        );
        assert_eq!(
            block_at(BlockPos::new(10, 64, 9)),
            generated_block_state_id(wheat_for_age(7).unwrap())
        );

        let player = store
            .player(&PlayerRecordKey::from_profile_id(identity.profile_id))
            .unwrap();
        assert_eq!(player.selected_hotbar_slot, 3);
        for (slot, kind, count) in [
            (1, ItemKind::OakFence, 32),
            (2, ItemKind::OakFenceGate, 4),
            (3, ItemKind::Carrot, 8),
            (7, ItemKind::WoodenHoe, 1),
            (8, ItemKind::WheatSeeds, 8),
        ] {
            assert_eq!(
                player.inventory[slot],
                Some(ItemStackSnapshot { kind, count })
            );
        }
    }

    #[test]
    fn rabbit_burrow_recipe_has_bounded_memories_and_two_reusable_mouths() {
        let identity = ClientIdentity::test_default();
        let (manifest, store) =
            playable_showcase_memory_store(PlayableShowcaseId::RabbitBurrow, &identity).unwrap();
        assert_eq!(manifest.revision, 4);
        assert_eq!(manifest.seed, 17_507);
        assert_eq!(manifest.entity_count, 6);
        assert_eq!(manifest.rabbit_count, 4);
        assert_eq!(manifest.rabbit_burrow_count, 2);
        assert_eq!(
            manifest.rabbit_field_guide_bits,
            RabbitObservationKind::Seen.bit() | RabbitObservationKind::FoundBurrow.bit()
        );

        let center_entities = store.entity_chunk(ChunkPos::new(0, 0)).unwrap();
        let burrow = center_entities
            .entities
            .iter()
            .find(|entity| entity.kind == "mclone:rabbit_burrow")
            .expect("semantic burrow record");
        assert!(matches!(
            burrow.payload,
            EntitySavePayload::RabbitBurrow { capacity: 1, .. }
        ));
        assert_eq!(
            center_entities
                .entities
                .iter()
                .filter(|entity| matches!(entity.payload, EntitySavePayload::Rabbit { .. }))
                .count(),
            3
        );
        assert!(center_entities.entities.iter().any(|entity| {
            matches!(
                entity.payload,
                EntitySavePayload::Rabbit {
                    known_refuges: [Some(RabbitRefugeSaveRecord {
                        persistent_id: home,
                        ..
                    }), None, None],
                    life_stage: RabbitLifeStage::Kit,
                    ..
                } if home == burrow.persistent_id
            )
        }));

        let west_entities = store.entity_chunk(ChunkPos::new(-1, 0)).unwrap();
        assert_eq!(
            west_entities
                .entities
                .iter()
                .filter(|entity| matches!(entity.payload, EntitySavePayload::RabbitBurrow { .. }))
                .count(),
            1
        );
        assert!(west_entities.entities.iter().any(|entity| {
            matches!(
                entity.payload,
                EntitySavePayload::Rabbit {
                    known_refuges: [Some(RabbitRefugeSaveRecord {
                        persistent_id: home,
                        last_known_position: Some(BlockPos { x: 8, y: 65, z: 8 }),
                        ..
                    }), None, None],
                    sheltered_in: None,
                    ..
                } if home == burrow.persistent_id
            )
        }));

        let center = store.chunk(ChunkPos::new(1, 0)).unwrap();
        let block_at = |pos: BlockPos| {
            center
                .snapshot
                .sections
                .iter()
                .find(|section| section.section_y == block_to_section_coord(pos.y))
                .map(|section| {
                    section.block_state_id_at(mclone_core::chunk_section_index(
                        pos.x.rem_euclid(16),
                        pos.y.rem_euclid(16),
                        pos.z.rem_euclid(16),
                    ))
                })
                .unwrap_or(mclone_core::AIR_BLOCK_STATE_ID)
        };
        assert_eq!(
            block_at(BlockPos::new(18, 65, 10)),
            generated_block_state_id(mclone_worldgen::block::CARROTS_AGE_7)
        );
        assert!(
            mclone_worldgen::block::oak_fence_gate_state(
                block_at(BlockPos::new(16, 65, 14)).0 as u16
            )
            .is_some_and(|gate| !gate.open)
        );
        assert!(
            mclone_worldgen::block::oak_fence_gate_state(
                block_at(BlockPos::new(16, 65, 10)).0 as u16
            )
            .is_none(),
            "the old direct-line opening must remain a fence"
        );
        let player = store
            .player(&PlayerRecordKey::from_profile_id(identity.profile_id))
            .unwrap();
        assert_eq!(
            player.inventory[3],
            Some(ItemStackSnapshot {
                kind: ItemKind::Carrot,
                count: 12,
            })
        );
        assert_eq!(
            player.rabbit_field_guide.bits(),
            manifest.rabbit_field_guide_bits
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

        let unbounded_remains =
            MALLARD_ECOLOGY_RECIPE.replacen("\"biomass\": 180", "\"biomass\": 0", 1);
        let error =
            parse_and_validate_recipe(PlayableShowcaseId::MallardEcology, &unbounded_remains)
                .unwrap_err()
                .to_string();
        assert!(error.contains("unbounded biomass"), "{error}");
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
    fn rabbit_schema_requires_typed_bounded_refuges_and_consistent_shelter() {
        let non_burrow = RABBIT_BURROW_RECIPE.replacen(
            "\"knownRefuges\": [\"home-warren\"]",
            "\"knownRefuges\": [\"doe\"]",
            1,
        );
        let error = parse_and_validate_recipe(PlayableShowcaseId::RabbitBurrow, &non_burrow)
            .unwrap_err()
            .to_string();
        assert!(error.contains("bounded refuge references"), "{error}");

        let impossible_shelter = RABBIT_BURROW_RECIPE.replacen(
            "\"behavior\": \"underground\"",
            "\"behavior\": \"idle\"",
            1,
        );
        let error =
            parse_and_validate_recipe(PlayableShowcaseId::RabbitBurrow, &impossible_shelter)
                .unwrap_err()
                .to_string();
        assert!(error.contains("bounded refuge references"), "{error}");
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
