use std::f64::consts::TAU;

use mclone_core::Vec3d;

use crate::entity::mob::{MobGoalContext, MobPlayerTarget};

use super::{Goal, GoalFlags, GoalSelector};

const RANDOM_STROLL_DEFAULT_INTERVAL: i32 = 120;
const WATER_AVOIDING_PROBABILITY: f32 = 0.001;
const LOOK_AT_PLAYER_PROBABILITY: f32 = 0.02;
const RANDOM_LOOK_AROUND_PROBABILITY: f32 = 0.02;

pub(crate) fn register_cow_goals(selector: &mut GoalSelector<MobGoalContext>) {
    selector.add_goal(5, WaterAvoidingRandomStrollGoal::new(1.0));
    selector.add_goal(6, LookAtPlayerGoal::new(6.0));
    selector.add_goal(7, RandomLookAroundGoal::default());
}

#[derive(Debug)]
struct WaterAvoidingRandomStrollGoal {
    wanted_position: Option<Vec3d>,
    speed_modifier: f64,
    interval: i32,
    force_trigger: bool,
    check_no_action_time: bool,
    probability: f32,
}

impl WaterAvoidingRandomStrollGoal {
    fn new(speed_modifier: f64) -> Self {
        Self {
            wanted_position: None,
            speed_modifier,
            interval: RANDOM_STROLL_DEFAULT_INTERVAL,
            force_trigger: false,
            check_no_action_time: true,
            probability: WATER_AVOIDING_PROBABILITY,
        }
    }

    fn get_position(&mut self, context: &mut MobGoalContext) -> Vec3d {
        let _use_default_random_pos =
            !context.is_in_water_or_bubble() && context.random_float() < self.probability;
        land_random_pos(context, 10.0)
    }
}

impl Goal<MobGoalContext> for WaterAvoidingRandomStrollGoal {
    fn can_use(&mut self, context: &mut MobGoalContext) -> bool {
        if !self.force_trigger {
            if self.check_no_action_time && context.no_action_time() >= 100 {
                return false;
            }
            if context.random_int_bound(self.interval) != 0 {
                return false;
            }
        }

        self.wanted_position = Some(self.get_position(context));
        self.force_trigger = false;
        true
    }

    fn can_continue_to_use(&mut self, context: &mut MobGoalContext) -> bool {
        context.has_move_target()
    }

    fn start(&mut self, context: &mut MobGoalContext) {
        if let Some(position) = self.wanted_position.take() {
            context.set_move_target(position, self.speed_modifier);
        }
    }

    fn stop(&mut self, context: &mut MobGoalContext) {
        context.stop_navigation();
    }

    fn flags(&self) -> GoalFlags {
        GoalFlags::MOVE
    }
}

#[derive(Debug)]
struct LookAtPlayerGoal {
    look_at: Option<MobPlayerTarget>,
    look_distance: f64,
    look_time: i32,
    probability: f32,
    only_horizontal: bool,
}

impl LookAtPlayerGoal {
    fn new(look_distance: f64) -> Self {
        Self {
            look_at: None,
            look_distance,
            look_time: 0,
            probability: LOOK_AT_PLAYER_PROBABILITY,
            only_horizontal: false,
        }
    }
}

impl Goal<MobGoalContext> for LookAtPlayerGoal {
    fn can_use(&mut self, context: &mut MobGoalContext) -> bool {
        if context.random_float() >= self.probability {
            return false;
        }
        self.look_at = context.nearest_player_within(self.look_distance);
        self.look_at.is_some()
    }

    fn can_continue_to_use(&mut self, context: &mut MobGoalContext) -> bool {
        let Some(target) = self.look_at else {
            return false;
        };
        self.look_time > 0 && context.distance_to_sqr(target.position) <= self.look_distance.powi(2)
    }

    fn start(&mut self, context: &mut MobGoalContext) {
        self.look_time = 40 + context.random_int_bound(40);
    }

    fn stop(&mut self, _context: &mut MobGoalContext) {
        self.look_at = None;
    }

    fn tick(&mut self, context: &mut MobGoalContext) {
        if let Some(target) = self.look_at {
            let y = if self.only_horizontal {
                context.eye_y()
            } else {
                target.eye_y
            };
            context.set_look_at(Vec3d::new(target.position.x, y, target.position.z));
        }
        self.look_time -= 1;
    }

    fn flags(&self) -> GoalFlags {
        GoalFlags::LOOK
    }
}

#[derive(Debug)]
struct RandomLookAroundGoal {
    rel_x: f64,
    rel_z: f64,
    look_time: i32,
}

impl Default for RandomLookAroundGoal {
    fn default() -> Self {
        Self {
            rel_x: 0.0,
            rel_z: 0.0,
            look_time: 0,
        }
    }
}

impl Goal<MobGoalContext> for RandomLookAroundGoal {
    fn can_use(&mut self, context: &mut MobGoalContext) -> bool {
        context.random_float() < RANDOM_LOOK_AROUND_PROBABILITY
    }

    fn can_continue_to_use(&mut self, _context: &mut MobGoalContext) -> bool {
        self.look_time >= 0
    }

    fn start(&mut self, context: &mut MobGoalContext) {
        let angle = TAU * context.random_double();
        self.rel_x = angle.cos();
        self.rel_z = angle.sin();
        self.look_time = 20 + context.random_int_bound(20);
    }

    fn tick(&mut self, context: &mut MobGoalContext) {
        self.look_time -= 1;
        let position = context.position();
        context.set_look_at(Vec3d::new(
            position.x + self.rel_x,
            context.eye_y(),
            position.z + self.rel_z,
        ));
    }

    fn flags(&self) -> GoalFlags {
        GoalFlags::of([super::GoalFlag::Move, super::GoalFlag::Look])
    }
}

fn land_random_pos(context: &mut MobGoalContext, horizontal_range: f64) -> Vec3d {
    // `LandRandomPos` and `PathNavigation` need chunk/block collision context.
    // Until that slice lands, use the same control boundary with a deterministic
    // local horizontal target so behavior updates flow through the real AI stack.
    let angle = TAU * context.random_double();
    let distance = 2.0 + context.random_double() * (horizontal_range - 2.0);
    let position = context.position();
    Vec3d::new(
        position.x + angle.cos() * distance,
        position.y,
        position.z + angle.sin() * distance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::metadata::EntityMetadata;
    use crate::entity::state::ServerEntityState;
    use mclone_protocol::{EntityId, EntityKind};
    use mclone_worldgen::prng::SimpleRandomSource;

    #[test]
    fn look_at_player_goal_sets_look_control_target() {
        let metadata = EntityMetadata::for_kind(EntityKind::Cow).unwrap();
        let entity = ServerEntityState::from_metadata(
            EntityId(1),
            metadata,
            Vec3d::new(0.0, 64.0, 0.0),
            0.0,
            0.0,
            None,
            true,
        );
        let mut context = MobGoalContext::from_parts_for_test(
            entity,
            metadata.standing_eye_height() as f64,
            vec![MobPlayerTarget::from_position(Vec3d::new(-4.0, 64.0, 0.0))],
            SimpleRandomSource::new(1),
        );
        let mut goal = LookAtPlayerGoal {
            probability: 1.0,
            ..LookAtPlayerGoal::new(6.0)
        };

        assert!(goal.can_use(&mut context));
        goal.start(&mut context);
        goal.tick(&mut context);
        let mut entity = entity;
        assert!(context.apply_controls(&mut entity));

        assert_eq!(entity.y_rot_degrees, 10.0);
    }
}
