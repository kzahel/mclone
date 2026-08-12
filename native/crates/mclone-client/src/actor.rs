use std::collections::{BTreeMap, BTreeSet};

use mclone_assets::{ActorFigureId, default_player_figure_id, upright_bear_figure_id};
use mclone_core::{HorizontalTopology, Vec3d};
use mclone_protocol::{
    EntityId, EntityKind, EntityRotation, EntitySnapshot, ItemStackSnapshot, MallardLifeStage,
    MallardNestSnapshotData, PlayerAppearance, PlayerModelKind, RemotePlayerId, RemotePlayerUpdate,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ActorPresentationId {
    RemotePlayer(RemotePlayerId),
    Entity(EntityId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorPresentationKind {
    RemotePlayer,
    Entity(EntityKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActorAppearance {
    pub figure: Option<ActorFigureId>,
}

impl ActorAppearance {
    pub const NONE: Self = Self { figure: None };

    pub const fn figure(figure: ActorFigureId) -> Self {
        Self {
            figure: Some(figure),
        }
    }

    pub const fn default_player() -> Self {
        Self::figure(default_player_figure_id())
    }
}

pub const fn actor_appearance_for_player_appearance(
    appearance: PlayerAppearance,
) -> ActorAppearance {
    match appearance.model {
        PlayerModelKind::Player => ActorAppearance::figure(default_player_figure_id()),
        PlayerModelKind::UprightBear => ActorAppearance::figure(upright_bear_figure_id()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorPresentation {
    pub id: ActorPresentationId,
    pub kind: ActorPresentationKind,
    pub appearance: ActorAppearance,
    pub item_stack: Option<ItemStackSnapshot>,
    /// Authoritative or interpolated feet position in world coordinates.
    pub feet_position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub rotation: Option<EntityRotation>,
    pub on_ground: bool,
    pub width: f32,
    pub height: f32,
    pub walk_animation_distance: f32,
    pub chicken_wing_flap_radians: Option<f32>,
    pub mallard_life_stage: Option<MallardLifeStage>,
    pub in_water: bool,
    pub mallard_nest: Option<MallardNestSnapshotData>,
}

impl ActorPresentation {
    pub fn remote_player(update: RemotePlayerUpdate, walk_animation_distance: f32) -> Self {
        Self {
            id: ActorPresentationId::RemotePlayer(update.id),
            kind: ActorPresentationKind::RemotePlayer,
            appearance: actor_appearance_for_player_appearance(update.appearance),
            item_stack: None,
            feet_position: update.position,
            y_rot_degrees: update.y_rot_degrees,
            x_rot_degrees: update.x_rot_degrees,
            rotation: None,
            on_ground: update.on_ground,
            width: 0.6,
            height: 1.8,
            walk_animation_distance,
            chicken_wing_flap_radians: None,
            mallard_life_stage: None,
            in_water: false,
            mallard_nest: None,
        }
    }

    pub fn entity(snapshot: EntitySnapshot) -> Self {
        Self {
            id: ActorPresentationId::Entity(snapshot.id),
            kind: ActorPresentationKind::Entity(snapshot.kind),
            appearance: ActorAppearance::NONE,
            item_stack: snapshot.item_stack,
            feet_position: snapshot.position,
            y_rot_degrees: snapshot.y_rot_degrees,
            x_rot_degrees: snapshot.x_rot_degrees,
            rotation: snapshot.rotation,
            on_ground: snapshot.on_ground,
            width: snapshot.width,
            height: snapshot.height,
            walk_animation_distance: 0.0,
            chicken_wing_flap_radians: None,
            mallard_life_stage: snapshot.mallard.map(|data| data.life_stage),
            in_water: snapshot.mallard.is_some_and(|data| data.in_water),
            mallard_nest: snapshot.mallard_nest,
        }
    }
}

const CHICKEN_WING_FLAP_MAX_RADIANS: f32 = 0.65;
const CHICKEN_WING_FLAP_TICKS_PER_SECOND: f32 = 20.0;
const CHICKEN_WING_FLAP_AIRBORNE_SPEED_DELTA_PER_TICK: f32 = 1.2;
const CHICKEN_WING_FLAP_GROUND_SPEED_DELTA_PER_TICK: f32 = -0.3;
const CHICKEN_WING_FLAP_PHASE_DELTA_PER_TICK: f32 = 2.0;
const CHICKEN_WING_VERTICAL_MOVE_EPSILON: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorInterpolationConfig {
    pub half_life_seconds: f32,
}

impl Default for ActorInterpolationConfig {
    fn default() -> Self {
        Self {
            half_life_seconds: 0.08,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActorInterpolationState {
    tracks: BTreeMap<ActorPresentationId, ActorTrack>,
}

impl ActorInterpolationState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_authoritative(actors: impl IntoIterator<Item = ActorPresentation>) -> Self {
        let mut state = Self::new();
        state.reconcile_authoritative(actors);
        state
    }

    pub fn reconcile_authoritative(&mut self, actors: impl IntoIterator<Item = ActorPresentation>) {
        self.reconcile_authoritative_in(HorizontalTopology::UNBOUNDED, actors);
    }

    pub fn reconcile_authoritative_in(
        &mut self,
        topology: HorizontalTopology,
        actors: impl IntoIterator<Item = ActorPresentation>,
    ) {
        let mut retained = BTreeSet::new();
        for actor in actors {
            retained.insert(actor.id);
            self.tracks
                .entry(actor.id)
                .and_modify(|track| track.set_target_in(topology, actor))
                .or_insert_with(|| ActorTrack::new(actor));
        }
        self.tracks.retain(|id, _| retained.contains(id));
    }

    pub fn step(&mut self, dt_seconds: f32, config: ActorInterpolationConfig) {
        self.step_with_remote_policy(dt_seconds, config, false);
    }

    pub fn step_with_preinterpolated_remote_players(
        &mut self,
        dt_seconds: f32,
        config: ActorInterpolationConfig,
    ) {
        self.step_with_remote_policy(dt_seconds, config, true);
    }

    fn step_with_remote_policy(
        &mut self,
        dt_seconds: f32,
        config: ActorInterpolationConfig,
        remote_players_preinterpolated: bool,
    ) {
        let factor = interpolation_factor(dt_seconds, config.half_life_seconds);
        for track in self.tracks.values_mut() {
            track.step(factor, dt_seconds, remote_players_preinterpolated);
        }
    }

    pub fn presentations(&self) -> Vec<ActorPresentation> {
        self.tracks
            .values()
            .map(|track| track.rendered)
            .collect::<Vec<_>>()
    }

    pub fn actor_count(&self) -> usize {
        self.tracks.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorTrack {
    rendered: ActorPresentation,
    target: ActorPresentation,
    derived: ActorDerivedAnimationState,
}

impl ActorTrack {
    fn new(actor: ActorPresentation) -> Self {
        Self {
            rendered: actor,
            target: actor,
            derived: ActorDerivedAnimationState::new(actor),
        }
    }

    fn set_target_in(&mut self, topology: HorizontalTopology, mut actor: ActorPresentation) {
        let previous_kind = self.rendered.kind;
        actor.feet_position =
            topology.nearest_position_lift(actor.feet_position, self.rendered.feet_position);
        self.target = actor;
        self.rendered.id = actor.id;
        self.rendered.kind = actor.kind;
        self.rendered.appearance = actor.appearance;
        self.rendered.item_stack = actor.item_stack;
        self.rendered.mallard_life_stage = actor.mallard_life_stage;
        self.rendered.in_water = actor.in_water;
        self.rendered.mallard_nest = actor.mallard_nest;
        self.rendered.on_ground = actor.on_ground;
        self.rendered.width = actor.width;
        self.rendered.height = actor.height;
        if uses_movement_derived_travel_phase(actor.kind) {
            if actor.kind != previous_kind {
                self.derived = ActorDerivedAnimationState::new(actor);
                self.rendered.walk_animation_distance = actor.walk_animation_distance;
            }
        } else {
            self.rendered.walk_animation_distance = actor.walk_animation_distance;
        }
        if !matches!(
            self.rendered.kind,
            ActorPresentationKind::Entity(EntityKind::Chicken)
        ) {
            self.derived = ActorDerivedAnimationState::default();
            self.rendered.chicken_wing_flap_radians = None;
        }
    }

    fn step(&mut self, factor: f32, dt_seconds: f32, remote_players_preinterpolated: bool) {
        let factor = if remote_players_preinterpolated
            && matches!(self.rendered.kind, ActorPresentationKind::RemotePlayer)
        {
            1.0
        } else {
            factor
        };
        let previous_position = self.rendered.feet_position;
        self.rendered.feet_position = lerp_vec3d(
            self.rendered.feet_position,
            self.target.feet_position,
            factor,
        );
        self.rendered.y_rot_degrees = lerp_degrees(
            self.rendered.y_rot_degrees,
            self.target.y_rot_degrees,
            factor,
        );
        self.rendered.x_rot_degrees = lerp_degrees(
            self.rendered.x_rot_degrees,
            self.target.x_rot_degrees,
            factor,
        );
        self.rendered.rotation =
            lerp_optional_entity_rotation(self.rendered.rotation, self.target.rotation, factor);
        self.rendered.on_ground = self.target.on_ground;
        self.derived.step(
            self.rendered.kind,
            previous_position,
            self.rendered.feet_position,
            self.rendered.on_ground,
            dt_seconds,
        );
        if uses_movement_derived_travel_phase(self.rendered.kind) {
            self.rendered.walk_animation_distance = self.derived.travel_distance;
        }
        self.rendered.chicken_wing_flap_radians =
            self.derived.chicken_wing_flap_radians(self.rendered.kind);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ActorDerivedAnimationState {
    travel_distance: f32,
    chicken: ChickenWingAnimationState,
}

impl ActorDerivedAnimationState {
    fn new(actor: ActorPresentation) -> Self {
        Self {
            travel_distance: actor.walk_animation_distance.max(0.0),
            chicken: ChickenWingAnimationState::default(),
        }
    }

    fn step(
        &mut self,
        kind: ActorPresentationKind,
        previous_position: Vec3d,
        current_position: Vec3d,
        on_ground: bool,
        dt_seconds: f32,
    ) {
        if uses_movement_derived_travel_phase(kind) {
            let distance = horizontal_distance(previous_position, current_position);
            if distance.is_finite() {
                self.travel_distance += distance;
            }
        }
        if !matches!(kind, ActorPresentationKind::Entity(EntityKind::Chicken)) {
            self.chicken = ChickenWingAnimationState::default();
            return;
        }
        self.chicken.step(
            previous_position.y,
            current_position.y,
            on_ground,
            dt_seconds,
        );
    }

    fn chicken_wing_flap_radians(self, kind: ActorPresentationKind) -> Option<f32> {
        matches!(kind, ActorPresentationKind::Entity(EntityKind::Chicken))
            .then_some(self.chicken.wing_flap_radians())
    }
}

const fn uses_movement_derived_travel_phase(kind: ActorPresentationKind) -> bool {
    matches!(
        kind,
        ActorPresentationKind::RemotePlayer
            | ActorPresentationKind::Entity(
                EntityKind::Cow | EntityKind::Chicken | EntityKind::Mallard | EntityKind::Mannequin
            )
    )
}

fn horizontal_distance(from: Vec3d, to: Vec3d) -> f32 {
    let dx = to.x - from.x;
    let dz = to.z - from.z;
    dx.mul_add(dx, dz * dz).sqrt() as f32
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ChickenWingAnimationState {
    phase: f32,
    speed: f32,
}

impl ChickenWingAnimationState {
    fn step(&mut self, previous_y: f64, current_y: f64, on_ground: bool, dt_seconds: f32) {
        if !dt_seconds.is_finite() || dt_seconds <= 0.0 {
            return;
        }
        let tick_delta = dt_seconds * CHICKEN_WING_FLAP_TICKS_PER_SECOND;
        let moved_vertically = (current_y - previous_y).abs() > CHICKEN_WING_VERTICAL_MOVE_EPSILON;
        let airborne = !on_ground || moved_vertically;
        let speed_delta = if airborne {
            CHICKEN_WING_FLAP_AIRBORNE_SPEED_DELTA_PER_TICK
        } else {
            CHICKEN_WING_FLAP_GROUND_SPEED_DELTA_PER_TICK
        };
        self.speed = (self.speed + speed_delta * tick_delta).clamp(0.0, 1.0);
        if self.speed > 0.0 {
            self.phase += CHICKEN_WING_FLAP_PHASE_DELTA_PER_TICK * tick_delta;
        }
    }

    fn wing_flap_radians(self) -> f32 {
        (self.phase.sin() + 1.0) * 0.5 * self.speed * CHICKEN_WING_FLAP_MAX_RADIANS
    }
}

fn interpolation_factor(dt_seconds: f32, half_life_seconds: f32) -> f32 {
    if !dt_seconds.is_finite() || dt_seconds <= 0.0 {
        return 0.0;
    }
    if !half_life_seconds.is_finite() || half_life_seconds <= 0.0 {
        return 1.0;
    }
    (1.0 - 2.0_f32.powf(-dt_seconds / half_life_seconds)).clamp(0.0, 1.0)
}

fn lerp_vec3d(from: Vec3d, to: Vec3d, factor: f32) -> Vec3d {
    from.add(to.subtract(from).scale(f64::from(factor.clamp(0.0, 1.0))))
}

fn lerp_degrees(from: f32, to: f32, factor: f32) -> f32 {
    let factor = factor.clamp(0.0, 1.0);
    from + wrap_degrees(to - from) * factor
}

fn wrap_degrees(value: f32) -> f32 {
    let mut wrapped = value % 360.0;
    if wrapped >= 180.0 {
        wrapped -= 360.0;
    }
    if wrapped < -180.0 {
        wrapped += 360.0;
    }
    wrapped
}

fn lerp_optional_entity_rotation(
    from: Option<EntityRotation>,
    to: Option<EntityRotation>,
    factor: f32,
) -> Option<EntityRotation> {
    match (from, to) {
        (Some(from), Some(to)) => Some(nlerp_entity_rotation(from, to, factor)),
        (_, to) => to,
    }
}

fn nlerp_entity_rotation(
    from: EntityRotation,
    mut to: EntityRotation,
    factor: f32,
) -> EntityRotation {
    let factor = factor.clamp(0.0, 1.0);
    let dot = from.x * to.x + from.y * to.y + from.z * to.z + from.w * to.w;
    if dot < 0.0 {
        to.x = -to.x;
        to.y = -to.y;
        to.z = -to.z;
        to.w = -to.w;
    }
    normalize_entity_rotation(EntityRotation {
        x: from.x + (to.x - from.x) * factor,
        y: from.y + (to.y - from.y) * factor,
        z: from.z + (to.z - from.z) * factor,
        w: from.w + (to.w - from.w) * factor,
    })
    .unwrap_or(to)
}

fn normalize_entity_rotation(rotation: EntityRotation) -> Option<EntityRotation> {
    let len_sqr = rotation.x * rotation.x
        + rotation.y * rotation.y
        + rotation.z * rotation.z
        + rotation.w * rotation.w;
    if !len_sqr.is_finite() || len_sqr <= f32::EPSILON {
        return None;
    }
    let inv_len = len_sqr.sqrt().recip();
    Some(EntityRotation {
        x: rotation.x * inv_len,
        y: rotation.y * inv_len,
        z: rotation.z * inv_len,
        w: rotation.w * inv_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(id: u64, x: f64, y_rot_degrees: f32) -> ActorPresentation {
        ActorPresentation {
            id: ActorPresentationId::RemotePlayer(RemotePlayerId(id)),
            kind: ActorPresentationKind::RemotePlayer,
            appearance: ActorAppearance::default_player(),
            item_stack: None,
            mallard_life_stage: None,
            in_water: false,
            mallard_nest: None,
            feet_position: Vec3d::new(x, 64.0, 2.0),
            y_rot_degrees,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.6,
            height: 1.8,
            walk_animation_distance: 0.0,
            chicken_wing_flap_radians: None,
        }
    }

    fn chicken_actor(id: u64, y: f64, on_ground: bool) -> ActorPresentation {
        ActorPresentation {
            id: ActorPresentationId::Entity(EntityId(id)),
            kind: ActorPresentationKind::Entity(EntityKind::Chicken),
            appearance: ActorAppearance::NONE,
            item_stack: None,
            mallard_life_stage: None,
            in_water: false,
            mallard_nest: None,
            feet_position: Vec3d::new(0.0, y, 0.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground,
            width: 0.4,
            height: 0.7,
            walk_animation_distance: 0.0,
            chicken_wing_flap_radians: None,
        }
    }

    fn mannequin_actor(id: u64, x: f64) -> ActorPresentation {
        ActorPresentation {
            id: ActorPresentationId::Entity(EntityId(id)),
            kind: ActorPresentationKind::Entity(EntityKind::Mannequin),
            appearance: ActorAppearance::NONE,
            item_stack: None,
            mallard_life_stage: None,
            in_water: false,
            mallard_nest: None,
            feet_position: Vec3d::new(x, 64.0, 0.0),
            y_rot_degrees: 0.0,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.6,
            height: 1.8,
            walk_animation_distance: 0.0,
            chicken_wing_flap_radians: None,
        }
    }

    #[test]
    fn actor_presentation_converts_remote_player_update() {
        let update = RemotePlayerUpdate {
            id: RemotePlayerId(3),
            appearance: PlayerAppearance::default(),
            position: Vec3d::new(10.0, 64.0, -4.0),
            y_rot_degrees: -90.0,
            x_rot_degrees: 15.0,
            on_ground: true,
        };

        assert_eq!(
            ActorPresentation::remote_player(update, 1.25),
            ActorPresentation {
                id: ActorPresentationId::RemotePlayer(update.id),
                kind: ActorPresentationKind::RemotePlayer,
                appearance: ActorAppearance::default_player(),
                item_stack: None,
                mallard_life_stage: None,
                in_water: false,
                mallard_nest: None,
                feet_position: update.position,
                y_rot_degrees: update.y_rot_degrees,
                x_rot_degrees: update.x_rot_degrees,
                rotation: None,
                on_ground: update.on_ground,
                width: 0.6,
                height: 1.8,
                walk_animation_distance: 1.25,
                chicken_wing_flap_radians: None,
            }
        );
    }

    #[test]
    fn actor_presentation_converts_entity_snapshot() {
        let snapshot = EntitySnapshot {
            id: EntityId(7),
            persistent_id: mclone_protocol::EntityPersistentId::new(0, 7),
            kind: EntityKind::Cow,
            item_stack: None,
            mallard: None,
            mallard_nest: None,
            position: Vec3d::new(10.0, 64.0, -4.0),
            y_rot_degrees: -90.0,
            x_rot_degrees: 0.0,
            rotation: Some(EntityRotation::IDENTITY),
            on_ground: true,
            width: 0.9,
            height: 1.4,
            tick_count: 12,
        };

        assert_eq!(
            ActorPresentation::entity(snapshot),
            ActorPresentation {
                id: ActorPresentationId::Entity(snapshot.id),
                kind: ActorPresentationKind::Entity(snapshot.kind),
                appearance: ActorAppearance::NONE,
                item_stack: snapshot.item_stack,
                mallard_life_stage: None,
                in_water: false,
                mallard_nest: None,
                feet_position: snapshot.position,
                y_rot_degrees: snapshot.y_rot_degrees,
                x_rot_degrees: snapshot.x_rot_degrees,
                rotation: snapshot.rotation,
                on_ground: snapshot.on_ground,
                width: snapshot.width,
                height: snapshot.height,
                walk_animation_distance: 0.0,
                chicken_wing_flap_radians: None,
            }
        );
    }

    #[test]
    fn actor_interpolation_reconciles_adds_updates_and_removes() {
        let mut state = ActorInterpolationState::new();

        state.reconcile_authoritative([actor(1, 0.0, 0.0), actor(2, 8.0, 0.0)]);
        assert_eq!(state.actor_count(), 2);

        state.reconcile_authoritative([actor(1, 10.0, 0.0)]);
        assert_eq!(state.actor_count(), 1);
        state.step(
            0.08,
            ActorInterpolationConfig {
                half_life_seconds: 0.08,
            },
        );
        let presentations = state.presentations();

        assert_eq!(presentations.len(), 1);
        assert_eq!(
            presentations[0].id,
            ActorPresentationId::RemotePlayer(RemotePlayerId(1))
        );
        assert!((presentations[0].feet_position.x - 5.0).abs() < 1.0e-9);
    }

    #[test]
    fn actor_interpolation_treats_a_periodic_seam_crossing_as_half_a_block() {
        let topology = HorizontalTopology::cylinder_x(0, 32);
        let mut state = ActorInterpolationState::from_authoritative([actor(1, 511.75, 0.0)]);

        state.reconcile_authoritative_in(topology, [actor(1, 0.25, 0.0)]);
        state.step(
            1.0,
            ActorInterpolationConfig {
                half_life_seconds: 0.0,
            },
        );
        let presentation = state.presentations()[0];

        assert_eq!(presentation.feet_position.x, 512.25);
        assert!((presentation.walk_animation_distance - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn actor_interpolation_wraps_rotations_across_degrees_boundary() {
        let mut state = ActorInterpolationState::from_authoritative([actor(1, 0.0, 170.0)]);
        state.reconcile_authoritative([actor(1, 0.0, -170.0)]);
        state.step(
            0.08,
            ActorInterpolationConfig {
                half_life_seconds: 0.08,
            },
        );

        let y_rot = state.presentations()[0].y_rot_degrees;

        assert!((y_rot - 180.0).abs() < 1.0e-6);
    }

    #[test]
    fn actor_interpolation_nlerps_entity_rotation() {
        let initial = ActorPresentation {
            rotation: Some(EntityRotation::IDENTITY),
            ..actor(1, 0.0, 0.0)
        };
        let target = ActorPresentation {
            rotation: Some(EntityRotation {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 0.0,
            }),
            ..actor(1, 0.0, 0.0)
        };
        let mut state = ActorInterpolationState::from_authoritative([initial]);
        state.reconcile_authoritative([target]);
        state.step(
            0.08,
            ActorInterpolationConfig {
                half_life_seconds: 0.08,
            },
        );

        let rotation = state.presentations()[0]
            .rotation
            .expect("interpolated rotation");
        assert!(rotation.x.abs() < 1.0e-6);
        assert!(rotation.y.abs() < 1.0e-6);
        assert!((rotation.z - 0.707_106_77).abs() < 1.0e-6);
        assert!((rotation.w - 0.707_106_77).abs() < 1.0e-6);
    }

    #[test]
    fn actor_interpolation_snaps_when_half_life_is_zero() {
        let mut state = ActorInterpolationState::from_authoritative([actor(1, 0.0, 0.0)]);
        state.reconcile_authoritative([actor(1, 12.0, 90.0)]);
        state.step(
            1.0,
            ActorInterpolationConfig {
                half_life_seconds: 0.0,
            },
        );
        let presentation = state.presentations()[0];

        assert_eq!(presentation.feet_position.x, 12.0);
        assert_eq!(presentation.y_rot_degrees, 90.0);
    }

    #[test]
    fn actor_interpolation_derives_chicken_wing_flap_locally() {
        let mut state = ActorInterpolationState::from_authoritative([chicken_actor(1, 64.0, true)]);
        state.reconcile_authoritative([chicken_actor(1, 63.8, false)]);
        state.step(
            1.0 / CHICKEN_WING_FLAP_TICKS_PER_SECOND,
            ActorInterpolationConfig {
                half_life_seconds: 0.0,
            },
        );
        let airborne_flap = state.presentations()[0]
            .chicken_wing_flap_radians
            .expect("airborne chicken wing flap");
        assert!(airborne_flap > 0.0);

        state.reconcile_authoritative([chicken_actor(1, 63.8, true)]);
        for _ in 0..4 {
            state.step(
                1.0 / CHICKEN_WING_FLAP_TICKS_PER_SECOND,
                ActorInterpolationConfig {
                    half_life_seconds: 0.0,
                },
            );
        }
        let grounded_flap = state.presentations()[0]
            .chicken_wing_flap_radians
            .expect("grounded chicken keeps local animation field");
        assert!(grounded_flap < airborne_flap);
    }

    #[test]
    fn actor_interpolation_derives_travel_phase_from_presented_displacement() {
        let mut state = ActorInterpolationState::from_authoritative([mannequin_actor(1, 0.0)]);
        state.reconcile_authoritative([mannequin_actor(1, 8.0)]);
        assert_eq!(state.presentations()[0].walk_animation_distance, 0.0);

        state.step(
            1.0 / 120.0,
            ActorInterpolationConfig {
                half_life_seconds: 0.08,
            },
        );
        let presented = state.presentations()[0];
        assert!(presented.feet_position.x > 0.0);
        assert!(
            (presented.walk_animation_distance as f64 - presented.feet_position.x).abs() < 1.0e-6
        );

        let phase_before_reconcile = presented.walk_animation_distance;
        state.reconcile_authoritative([mannequin_actor(1, 12.0)]);
        assert_eq!(
            state.presentations()[0].walk_animation_distance,
            phase_before_reconcile
        );
    }

    #[test]
    fn movement_derived_phase_has_no_display_frequency_ceiling() {
        fn simulate(dt_seconds: f32, frame_count: usize) -> ActorPresentation {
            let mut state = ActorInterpolationState::from_authoritative([mannequin_actor(1, 0.0)]);
            state.reconcile_authoritative([mannequin_actor(1, 4.0)]);
            for _ in 0..frame_count {
                state.step(
                    dt_seconds,
                    ActorInterpolationConfig {
                        half_life_seconds: 0.08,
                    },
                );
            }
            state.presentations()[0]
        }

        let sixty_hz = simulate(1.0 / 60.0, 60);
        let five_hundred_hz = simulate(1.0 / 500.0, 500);

        assert!((sixty_hz.feet_position.x - five_hundred_hz.feet_position.x).abs() < 1.0e-6);
        assert!(
            (sixty_hz.walk_animation_distance - five_hundred_hz.walk_animation_distance).abs()
                < 1.0e-5
        );
    }

    #[test]
    fn movement_derived_phase_is_identity_stable_and_resets_after_despawn() {
        let mut state = ActorInterpolationState::from_authoritative([
            mannequin_actor(2, 10.0),
            mannequin_actor(1, 0.0),
        ]);
        state.reconcile_authoritative([mannequin_actor(1, 1.0), mannequin_actor(2, 13.0)]);
        state.step(
            1.0,
            ActorInterpolationConfig {
                half_life_seconds: 0.0,
            },
        );
        let presentations = state.presentations();
        assert_eq!(
            presentations[0].id,
            ActorPresentationId::Entity(EntityId(1))
        );
        assert_eq!(presentations[0].walk_animation_distance, 1.0);
        assert_eq!(
            presentations[1].id,
            ActorPresentationId::Entity(EntityId(2))
        );
        assert_eq!(presentations[1].walk_animation_distance, 3.0);

        state.reconcile_authoritative([mannequin_actor(2, 13.0)]);
        state.reconcile_authoritative([mannequin_actor(1, 20.0), mannequin_actor(2, 13.0)]);
        assert_eq!(state.presentations()[0].walk_animation_distance, 0.0);

        state.step(1.0 / 500.0, ActorInterpolationConfig::default());
        let phase = state.presentations()[1].walk_animation_distance;
        for _ in 0..500 {
            state.step(1.0 / 500.0, ActorInterpolationConfig::default());
        }
        assert_eq!(state.presentations()[1].walk_animation_distance, phase);
    }
}
