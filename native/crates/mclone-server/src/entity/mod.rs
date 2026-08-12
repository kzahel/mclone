//! Server-owned entity replicas and visibility routing.
//!
//! This is intentionally narrower than Java's full `ChunkMap.TrackedEntity`
//! stack. It owns authoritative passive actors, chunk-addressed persistence,
//! and the first bounded live natural-spawn path while combat and broader mob
//! categories remain out of scope.

mod item;
mod metadata;
mod mob;
pub(crate) mod spawning;
mod state;
mod store;
mod tick_list;
mod tracking;
mod visibility;

pub(crate) use mob::{MALLARD_GROWTH_REQUIRED_TICKS, MobPlayerTarget};
pub(crate) use state::ServerEntityState;
#[cfg(feature = "physics-engine")]
pub(crate) use store::DebugPhysicsCubeEntitySpawn;
pub(crate) use store::{BeePollinationEvent, ItemPickupTarget, ServerEntityStore};
pub(crate) use tracking::{EntityTracking, RoutedEntityUpdate};
