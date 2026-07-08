//! Server-owned entity replicas and visibility routing.
//!
//! This is intentionally narrower than Java's full `ChunkMap.TrackedEntity`
//! stack. It gives native clients authoritative snapshots for simple passive
//! actor rendering without enabling live natural spawning, persistence, or
//! combat.

mod item;
mod metadata;
mod mob;
pub(crate) mod spawning;
mod state;
mod store;
mod tick_list;
mod tracking;
mod visibility;

pub(crate) use mob::MobPlayerTarget;
pub(crate) use state::ServerEntityState;
#[cfg(feature = "physics-engine")]
pub(crate) use store::DebugPhysicsCubeEntitySpawn;
pub(crate) use store::{ItemPickupTarget, ServerEntityStore};
pub(crate) use tracking::{EntityTracking, RoutedEntityUpdate};
