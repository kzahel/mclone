use mclone_blocks::{collide_movement, collide_movement_result, collision_aabb_for_feet_position};
use mclone_core::{BlockPos, BlockStateId, Vec3d};
use mclone_protocol::{EntityId, ItemStackSnapshot};
use mclone_worldgen::prng::SimpleRandomSource;

use crate::item_stack::{item_stack_has_room, merged_item_stack};

use super::ServerEntityState;

pub(crate) const ITEM_ENTITY_LIFETIME_TICKS: u64 = 6_000;

const ITEM_GRAVITY: f64 = 0.04;
const ITEM_AIR_DRAG: f64 = 0.98;
const ITEM_GROUND_FRICTION: f64 = 0.6 * ITEM_AIR_DRAG;
const ITEM_GROUND_BOUNCE: f64 = -0.5;
const ITEM_DEFAULT_PICKUP_DELAY: i32 = 10;
const ITEM_INFINITE_PICKUP_DELAY: i32 = 32_767;
const ITEM_COLLISION_EPSILON: f64 = 1.0e-7;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ItemEntityRuntimeState {
    stack: ItemStackSnapshot,
    delta_movement: Vec3d,
    pickup_delay: i32,
}

impl ItemEntityRuntimeState {
    pub(crate) fn from_spawn(id: EntityId, stack: ItemStackSnapshot) -> Self {
        let mut random = SimpleRandomSource::new(item_random_seed(id, stack));
        Self {
            stack,
            delta_movement: Vec3d::new(
                random.next_double() * 0.2 - 0.1,
                0.2,
                random.next_double() * 0.2 - 0.1,
            ),
            pickup_delay: ITEM_DEFAULT_PICKUP_DELAY,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn from_saved(
        stack: ItemStackSnapshot,
        delta_movement: Vec3d,
        pickup_delay: i32,
    ) -> Self {
        Self {
            stack,
            delta_movement,
            pickup_delay,
        }
    }

    pub(crate) fn stack(&self) -> ItemStackSnapshot {
        self.stack
    }

    pub(crate) fn can_pick_up(&self) -> bool {
        self.pickup_delay == 0
    }

    pub(crate) fn replace_stack(&mut self, stack: ItemStackSnapshot) {
        self.stack = stack;
    }

    pub(crate) fn is_mergeable(&self, entity: ServerEntityState) -> bool {
        entity.alive
            && self.pickup_delay != ITEM_INFINITE_PICKUP_DELAY
            && entity.age_ticks < ITEM_ENTITY_LIFETIME_TICKS
            && item_stack_has_room(self.stack)
    }

    pub(crate) fn merged_with(&self, other: &Self) -> Option<(ItemStackSnapshot, i32)> {
        merged_item_stack(self.stack, other.stack)
            .map(|stack| (stack, self.pickup_delay.max(other.pickup_delay)))
    }

    #[allow(dead_code)]
    pub(crate) fn delta_movement(&self) -> Vec3d {
        self.delta_movement
    }

    #[cfg(test)]
    pub(crate) fn set_pickup_delay_for_test(&mut self, pickup_delay: i32) {
        self.pickup_delay = pickup_delay;
    }

    #[cfg(test)]
    pub(crate) fn has_pickup_delay(&self) -> bool {
        self.pickup_delay > 0
    }

    #[allow(dead_code)]
    pub(crate) fn pickup_delay(&self) -> i32 {
        self.pickup_delay
    }

    pub(crate) fn set_pickup_delay(&mut self, pickup_delay: i32) {
        self.pickup_delay = pickup_delay;
    }

    pub(crate) fn tick_entity<F>(&mut self, entity: &mut ServerEntityState, block_state_at: &F)
    where
        F: Fn(BlockPos) -> Option<BlockStateId>,
    {
        if self.pickup_delay > 0 && self.pickup_delay != ITEM_INFINITE_PICKUP_DELAY {
            self.pickup_delay -= 1;
        }

        self.delta_movement = self.delta_movement.add(Vec3d::new(0.0, -ITEM_GRAVITY, 0.0));
        let previous_position = entity.position;
        let requested = self.delta_movement;
        let bounding_box = collision_aabb_for_feet_position(
            previous_position,
            f64::from(entity.width),
            f64::from(entity.height),
        );
        let traveled = collide_movement(block_state_at, bounding_box, requested);
        if traveled.length_sqr() > ITEM_COLLISION_EPSILON * ITEM_COLLISION_EPSILON {
            entity.position = entity.position.add(traveled);
        }

        let collision = collide_movement_result(requested, traveled);
        entity.on_ground = collision.on_ground;

        let horizontal_drag = if entity.on_ground {
            ITEM_GROUND_FRICTION
        } else {
            ITEM_AIR_DRAG
        };
        self.delta_movement = Vec3d::new(
            self.delta_movement.x * horizontal_drag,
            self.delta_movement.y * ITEM_AIR_DRAG,
            self.delta_movement.z * horizontal_drag,
        );
        if entity.on_ground && self.delta_movement.y < 0.0 {
            self.delta_movement = Vec3d::new(
                self.delta_movement.x,
                self.delta_movement.y * ITEM_GROUND_BOUNCE,
                self.delta_movement.z,
            );
        }
    }
}

fn item_random_seed(id: EntityId, stack: ItemStackSnapshot) -> i64 {
    let item_id = match stack.kind {
        mclone_protocol::ItemKind::Egg => 0x0000_0001_u64,
    };
    (id.0.wrapping_mul(0xbf58_476d_1ce4_e5b9).rotate_left(23) ^ item_id ^ u64::from(stack.count))
        as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use mclone_protocol::{EntityKind, ItemKind};

    fn no_blocks(_pos: BlockPos) -> Option<BlockStateId> {
        None
    }

    fn flat_ground(pos: BlockPos) -> Option<BlockStateId> {
        Some(if pos.y == 63 {
            BlockStateId(1)
        } else {
            BlockStateId(mclone_blocks::terrain_id::AIR)
        })
    }

    fn item_state() -> ServerEntityState {
        ServerEntityState {
            id: EntityId(1),
            persistent_id: mclone_protocol::EntityPersistentId::new(0, 1),
            kind: EntityKind::Item,
            item_stack: Some(ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            }),
            position: Vec3d::new(0.5, 64.0, 0.5),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: false,
            width: 0.25,
            height: 0.25,
            age_ticks: 0,
            alive: true,
        }
    }

    #[test]
    fn item_entity_spawn_velocity_matches_java_shape() {
        let stack = ItemStackSnapshot {
            kind: ItemKind::Egg,
            count: 1,
        };
        let item = ItemEntityRuntimeState::from_spawn(EntityId(5), stack);

        assert_eq!(item.stack(), stack);
        assert_eq!(item.pickup_delay(), ITEM_DEFAULT_PICKUP_DELAY);
        assert!(item.has_pickup_delay());
        assert!(!item.can_pick_up());
        assert!((-0.1..0.1).contains(&item.delta_movement().x));
        assert_eq!(item.delta_movement().y, 0.2);
        assert!((-0.1..0.1).contains(&item.delta_movement().z));
    }

    #[test]
    fn item_entity_applies_gravity_and_drag() {
        let mut entity = item_state();
        let mut item = ItemEntityRuntimeState {
            stack: entity.item_stack.unwrap(),
            delta_movement: Vec3d::new(0.0, 0.0, 0.0),
            pickup_delay: ITEM_DEFAULT_PICKUP_DELAY,
        };

        item.tick_entity(&mut entity, &no_blocks);

        assert!(entity.position.y < 64.0);
        assert!(!entity.on_ground);
        assert!(item.delta_movement().y < 0.0);
        assert_eq!(item.pickup_delay(), ITEM_DEFAULT_PICKUP_DELAY - 1);
    }

    #[test]
    fn item_entity_bounces_on_ground() {
        let mut entity = item_state();
        let mut item = ItemEntityRuntimeState {
            stack: entity.item_stack.unwrap(),
            delta_movement: Vec3d::new(0.0, -0.2, 0.0),
            pickup_delay: 0,
        };

        item.tick_entity(&mut entity, &flat_ground);

        assert!(entity.on_ground);
        assert!(item.delta_movement().y > 0.0);
    }

    #[test]
    fn item_entity_merges_same_stack_when_capacity_allows() {
        let mut entity = item_state();
        entity.age_ticks = 12;
        let mut left = ItemEntityRuntimeState {
            stack: ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            delta_movement: Vec3d::ZERO,
            pickup_delay: 2,
        };
        let right = ItemEntityRuntimeState {
            stack: ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 3,
            },
            delta_movement: Vec3d::ZERO,
            pickup_delay: 7,
        };

        assert!(left.is_mergeable(entity));
        let (stack, pickup_delay) = left.merged_with(&right).expect("mergeable egg stacks");
        left.replace_stack(stack);
        left.set_pickup_delay(pickup_delay);

        assert_eq!(
            left.stack(),
            ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 4,
            }
        );
        assert_eq!(left.pickup_delay(), 7);
    }

    #[test]
    fn item_entity_does_not_merge_full_stack() {
        let mut left = ItemEntityRuntimeState {
            stack: ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: crate::item_stack::EGG_MAX_STACK_SIZE,
            },
            delta_movement: Vec3d::ZERO,
            pickup_delay: 0,
        };
        let right = ItemEntityRuntimeState {
            stack: ItemStackSnapshot {
                kind: ItemKind::Egg,
                count: 1,
            },
            delta_movement: Vec3d::ZERO,
            pickup_delay: 0,
        };

        assert_eq!(left.merged_with(&right), None);
        left.set_pickup_delay_for_test(ITEM_INFINITE_PICKUP_DELAY);
        assert!(!left.is_mergeable(item_state()));
    }
}
