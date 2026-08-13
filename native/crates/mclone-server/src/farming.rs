use mclone_core::{BlockPos, ChunkPos, Direction};
use mclone_protocol::{ItemKind, ItemStackSnapshot};
use mclone_worldgen::block::{
    AIR, DIRT, FARMLAND_MOISTURE_0, FARMLAND_MOISTURE_7, GRASS_BLOCK, RawBlockId,
    farmland_for_moisture, farmland_moisture, is_air_like, is_water, wheat_age, wheat_for_age,
};
use mclone_worldgen::prng::SimpleRandomSource;

pub(crate) const RANDOM_TICK_SPEED: usize = 3;
const SECTION_COUNT: i32 = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FarmingBlockAction {
    Till { pos: BlockPos, state: RawBlockId },
    Plant { pos: BlockPos, state: RawBlockId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WheatHarvest {
    pub(crate) seed_count: u8,
    pub(crate) wheat_count: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RandomFarmingTick {
    pub(crate) pos: BlockPos,
    pub(crate) next_state: RawBlockId,
}

pub(crate) fn plan_till<F>(
    hit_pos: BlockPos,
    face: Direction,
    block_at: F,
) -> Option<FarmingBlockAction>
where
    F: Fn(BlockPos) -> Option<RawBlockId>,
{
    if face == Direction::Down || !matches!(block_at(hit_pos)?, GRASS_BLOCK | DIRT) {
        return None;
    }
    let above = hit_pos.offset(0, 1, 0);
    if !is_air_like(block_at(above)?) {
        return None;
    }
    Some(FarmingBlockAction::Till {
        pos: hit_pos,
        state: if farmland_is_near_water(hit_pos, &block_at) {
            FARMLAND_MOISTURE_7
        } else {
            FARMLAND_MOISTURE_0
        },
    })
}

pub(crate) fn plan_plant<F>(
    hit_pos: BlockPos,
    face: Direction,
    block_at: F,
) -> Option<FarmingBlockAction>
where
    F: Fn(BlockPos) -> Option<RawBlockId>,
{
    farmland_moisture(block_at(hit_pos)?)?;
    let crop_pos = hit_pos.offset(0, 1, 0);
    (face != Direction::Down && is_air_like(block_at(crop_pos)?)).then_some(
        FarmingBlockAction::Plant {
            pos: crop_pos,
            state: wheat_for_age(0).expect("wheat age zero exists"),
        },
    )
}

pub(crate) fn wheat_harvest(
    block: RawBlockId,
    random: &mut SimpleRandomSource,
) -> Option<WheatHarvest> {
    let age = wheat_age(block)?;
    if age < 7 {
        return Some(WheatHarvest {
            seed_count: 1,
            wheat_count: 0,
        });
    }
    let extra_seeds = (0..3).filter(|_| random.next_float() < 0.571_428_6).count() as u8;
    Some(WheatHarvest {
        seed_count: extra_seeds,
        wheat_count: 1,
    })
}

pub(crate) fn harvest_stacks(harvest: WheatHarvest) -> impl Iterator<Item = ItemStackSnapshot> {
    [
        (harvest.wheat_count > 0).then_some(ItemStackSnapshot {
            kind: ItemKind::Wheat,
            count: harvest.wheat_count,
        }),
        (harvest.seed_count > 0).then_some(ItemStackSnapshot {
            kind: ItemKind::WheatSeeds,
            count: harvest.seed_count,
        }),
    ]
    .into_iter()
    .flatten()
}

pub(crate) fn random_farming_tick_candidates(
    seed: i64,
    game_time: u64,
    chunks: &[ChunkPos],
) -> Vec<BlockPos> {
    let mut chunks = chunks.to_vec();
    chunks.sort_unstable();
    let mut candidates =
        Vec::with_capacity(chunks.len() * SECTION_COUNT as usize * RANDOM_TICK_SPEED);
    for chunk in chunks {
        for section_y in 0..SECTION_COUNT {
            let mut random =
                SimpleRandomSource::new(random_tick_seed(seed, game_time, chunk, section_y));
            for _ in 0..RANDOM_TICK_SPEED {
                candidates.push(BlockPos::new(
                    chunk.min_block_x() + random.next_int_bound(16),
                    section_y * 16 + random.next_int_bound(16),
                    chunk.min_block_z() + random.next_int_bound(16),
                ));
            }
        }
    }
    candidates
}

pub(crate) fn random_farming_tick<F>(
    pos: BlockPos,
    random: &mut SimpleRandomSource,
    block_at: F,
    brightness: Option<u8>,
) -> Option<RandomFarmingTick>
where
    F: Fn(BlockPos) -> Option<RawBlockId>,
{
    let block = block_at(pos)?;
    if let Some(moisture) = farmland_moisture(block) {
        let wet = farmland_is_near_water(pos, &block_at);
        let next_state = if wet && moisture < 7 {
            FARMLAND_MOISTURE_7
        } else if !wet && moisture > 0 {
            farmland_for_moisture(moisture - 1).expect("lower moisture exists")
        } else if !wet && moisture == 0 && wheat_age(block_at(pos.offset(0, 1, 0))?).is_none() {
            DIRT
        } else {
            return None;
        };
        return Some(RandomFarmingTick { pos, next_state });
    }

    let age = wheat_age(block)?;
    if age >= 7 || brightness.unwrap_or(0) < 9 {
        return None;
    }
    if farmland_moisture(block_at(pos.below())?).is_none() {
        return Some(RandomFarmingTick {
            pos,
            next_state: AIR,
        });
    }
    let speed = wheat_growth_speed(pos, &block_at);
    let bound = (25.0 / speed) as i32 + 1;
    (random.next_int_bound(bound) == 0).then(|| RandomFarmingTick {
        pos,
        next_state: wheat_for_age(age + 1).expect("next wheat age exists"),
    })
}

fn farmland_is_near_water<F>(pos: BlockPos, block_at: &F) -> bool
where
    F: Fn(BlockPos) -> Option<RawBlockId>,
{
    (-4..=4).any(|dx| {
        (-4..=4).any(|dz| (0..=1).any(|dy| block_at(pos.offset(dx, dy, dz)).is_some_and(is_water)))
    })
}

fn wheat_growth_speed<F>(pos: BlockPos, block_at: &F) -> f32
where
    F: Fn(BlockPos) -> Option<RawBlockId>,
{
    let below = pos.below();
    let mut speed = 1.0;
    for dx in -1..=1 {
        for dz in -1..=1 {
            let mut contribution =
                farmland_moisture(block_at(below.offset(dx, 0, dz)).unwrap_or(AIR))
                    .map_or(0.0, |moisture| if moisture > 0 { 3.0 } else { 1.0 });
            if dx != 0 || dz != 0 {
                contribution /= 4.0;
            }
            speed += contribution;
        }
    }

    let wheat_at = |candidate| block_at(candidate).and_then(wheat_age).is_some();
    let east_west = wheat_at(pos.offset(-1, 0, 0)) || wheat_at(pos.offset(1, 0, 0));
    let north_south = wheat_at(pos.offset(0, 0, -1)) || wheat_at(pos.offset(0, 0, 1));
    let diagonal = [(-1, -1), (1, -1), (1, 1), (-1, 1)]
        .into_iter()
        .any(|(dx, dz)| wheat_at(pos.offset(dx, 0, dz)));
    if east_west && north_south || (!east_west || !north_south) && diagonal {
        speed /= 2.0;
    }
    speed
}

fn random_tick_seed(seed: i64, game_time: u64, chunk: ChunkPos, section_y: i32) -> i64 {
    seed ^ (game_time as i64).wrapping_mul(6_364_136_223_846_793_005)
        ^ i64::from(chunk.x).wrapping_mul(341_873_128_712)
        ^ i64::from(chunk.z).wrapping_mul(132_897_987_541)
        ^ i64::from(section_y).wrapping_mul(31_337)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use mclone_worldgen::block::{WATER, WHEAT_AGE_0, WHEAT_AGE_7};

    #[test]
    fn till_and_plant_require_reference_support_and_clearance() {
        let ground = BlockPos::new(3, 64, 5);
        let blocks = BTreeMap::from([(ground, GRASS_BLOCK), (ground.offset(0, 1, 0), AIR)]);
        assert_eq!(
            plan_till(ground, Direction::Up, |pos| blocks.get(&pos).copied()),
            Some(FarmingBlockAction::Till {
                pos: ground,
                state: FARMLAND_MOISTURE_0
            })
        );
        assert_eq!(
            plan_till(ground, Direction::Down, |pos| blocks.get(&pos).copied()),
            None
        );
        let blocks = BTreeMap::from([(ground, FARMLAND_MOISTURE_0), (ground.offset(0, 1, 0), AIR)]);
        assert_eq!(
            plan_plant(ground, Direction::Up, |pos| blocks.get(&pos).copied()),
            Some(FarmingBlockAction::Plant {
                pos: ground.offset(0, 1, 0),
                state: WHEAT_AGE_0
            })
        );
    }

    #[test]
    fn water_hydrates_and_unwatered_bare_soil_reverts() {
        let soil = BlockPos::new(0, 64, 0);
        let wet = BTreeMap::from([(soil, FARMLAND_MOISTURE_0), (soil.offset(4, 0, 0), WATER)]);
        assert_eq!(
            random_farming_tick(
                soil,
                &mut SimpleRandomSource::new(1),
                |pos| wet.get(&pos).copied().or(Some(AIR)),
                Some(15)
            ),
            Some(RandomFarmingTick {
                pos: soil,
                next_state: FARMLAND_MOISTURE_7
            })
        );
        let dry = BTreeMap::from([(soil, FARMLAND_MOISTURE_0), (soil.offset(0, 1, 0), AIR)]);
        assert_eq!(
            random_farming_tick(
                soil,
                &mut SimpleRandomSource::new(1),
                |pos| dry.get(&pos).copied().or(Some(AIR)),
                Some(15)
            ),
            Some(RandomFarmingTick {
                pos: soil,
                next_state: DIRT
            })
        );
    }

    #[test]
    fn tilling_beside_water_is_immediately_hydrated() {
        let soil = BlockPos::new(0, 64, 0);
        let wet = BTreeMap::from([
            (soil, GRASS_BLOCK),
            (soil.offset(0, 1, 0), AIR),
            (soil.offset(4, 1, 0), WATER),
        ]);
        assert_eq!(
            plan_till(soil, Direction::Up, |pos| {
                wet.get(&pos).copied().or(Some(AIR))
            }),
            Some(FarmingBlockAction::Till {
                pos: soil,
                state: FARMLAND_MOISTURE_7,
            })
        );

        let dry = BTreeMap::from([(soil, DIRT), (soil.offset(0, 1, 0), AIR)]);
        assert_eq!(
            plan_till(soil, Direction::Up, |pos| {
                dry.get(&pos).copied().or(Some(AIR))
            }),
            Some(FarmingBlockAction::Till {
                pos: soil,
                state: FARMLAND_MOISTURE_0,
            })
        );
    }

    #[test]
    fn mature_and_immature_harvests_are_renewable() {
        let immature = wheat_harvest(WHEAT_AGE_0, &mut SimpleRandomSource::new(5)).unwrap();
        assert_eq!(
            immature,
            WheatHarvest {
                seed_count: 1,
                wheat_count: 0
            }
        );
        let mature = wheat_harvest(WHEAT_AGE_7, &mut SimpleRandomSource::new(5)).unwrap();
        assert_eq!(mature.wheat_count, 1);
        assert!(mature.seed_count <= 3);
    }

    #[test]
    fn hydrated_farmland_advances_wheat_through_the_reference_probability() {
        let crop = BlockPos::new(0, 65, 0);
        let blocks = BTreeMap::from([(crop, WHEAT_AGE_0), (crop.below(), FARMLAND_MOISTURE_7)]);
        let mut advanced = None;
        for seed in 0..128 {
            let plan = random_farming_tick(
                crop,
                &mut SimpleRandomSource::new(seed),
                |pos| blocks.get(&pos).copied().or(Some(AIR)),
                Some(15),
            );
            if plan.is_some() {
                advanced = plan;
                break;
            }
        }
        assert_eq!(
            advanced,
            Some(RandomFarmingTick {
                pos: crop,
                next_state: mclone_worldgen::block::WHEAT_AGE_1,
            })
        );
        assert_eq!(
            random_farming_tick(
                crop,
                &mut SimpleRandomSource::new(1),
                |pos| blocks.get(&pos).copied().or(Some(AIR)),
                Some(8),
            ),
            None
        );
    }

    #[test]
    fn candidate_sampling_is_order_independent_and_bounded() {
        let a = ChunkPos::new(-1, 2);
        let b = ChunkPos::new(3, 4);
        let forward = random_farming_tick_candidates(17, 90, &[a, b]);
        let reverse = random_farming_tick_candidates(17, 90, &[b, a]);
        assert_eq!(forward, reverse);
        assert_eq!(forward.len(), 2 * 16 * RANDOM_TICK_SPEED);
        assert!(forward.iter().all(|pos| (0..256).contains(&pos.y)));
    }
}
