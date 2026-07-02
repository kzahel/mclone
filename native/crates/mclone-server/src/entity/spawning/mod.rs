//! Java-shaped natural spawning boundaries.
//!
//! These modules intentionally separate spawn caps, spawn tables, placement
//! facts, and natural-spawn orchestration from individual entity behavior.
//! Live natural spawning remains disabled until the server has the required
//! player-distance chunk set, category counts, world predicates, and lifecycle
//! policy wired through this boundary.

#![allow(dead_code)]

pub(crate) mod biome_tables;
pub(crate) mod mob_category;
pub(crate) mod natural;
pub(crate) mod placements;
pub(crate) mod spawn_state;
