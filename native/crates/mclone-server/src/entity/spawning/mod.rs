//! Java-shaped natural spawning boundaries.
//!
//! These modules intentionally separate spawn caps, spawn tables, placement
//! facts, and natural-spawn orchestration from individual entity behavior.
//! Live persistent natural spawning remains disabled until the server has the
//! full lifecycle policy wired through this boundary. Bounded volatile passive
//! spawning may use the same planner explicitly while entity persistence is
//! still absent.

#![allow(dead_code)]

pub(crate) mod biome_tables;
pub(crate) mod dry_run;
pub(crate) mod habitat;
pub(crate) mod live;
pub(crate) mod mob_category;
pub(crate) mod natural;
pub(crate) mod placements;
pub(crate) mod spawn_state;
