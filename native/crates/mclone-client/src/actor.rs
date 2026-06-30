use std::collections::{BTreeMap, BTreeSet};

use mclone_assets::{ActorFigureId, default_player_figure_id, upright_bear_figure_id};
use mclone_core::Vec3d;
use mclone_protocol::{
    EntityId, EntityKind, EntityRotation, EntitySnapshot, PlayerAppearance, PlayerModelKind,
    RemotePlayerId, RemotePlayerUpdate,
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
    /// Authoritative or interpolated feet position in world coordinates.
    pub feet_position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub rotation: Option<EntityRotation>,
    pub on_ground: bool,
    pub width: f32,
    pub height: f32,
    pub walk_animation_distance: f32,
}

impl ActorPresentation {
    pub fn remote_player(update: RemotePlayerUpdate, walk_animation_distance: f32) -> Self {
        Self {
            id: ActorPresentationId::RemotePlayer(update.id),
            kind: ActorPresentationKind::RemotePlayer,
            appearance: actor_appearance_for_player_appearance(update.appearance),
            feet_position: update.position,
            y_rot_degrees: update.y_rot_degrees,
            x_rot_degrees: update.x_rot_degrees,
            rotation: None,
            on_ground: update.on_ground,
            width: 0.6,
            height: 1.8,
            walk_animation_distance,
        }
    }

    pub fn entity(snapshot: EntitySnapshot) -> Self {
        Self {
            id: ActorPresentationId::Entity(snapshot.id),
            kind: ActorPresentationKind::Entity(snapshot.kind),
            appearance: ActorAppearance::NONE,
            feet_position: snapshot.position,
            y_rot_degrees: snapshot.y_rot_degrees,
            x_rot_degrees: snapshot.x_rot_degrees,
            rotation: snapshot.rotation,
            on_ground: snapshot.on_ground,
            width: snapshot.width,
            height: snapshot.height,
            walk_animation_distance: 0.0,
        }
    }
}

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
        let mut retained = BTreeSet::new();
        for actor in actors {
            retained.insert(actor.id);
            self.tracks
                .entry(actor.id)
                .and_modify(|track| track.set_target(actor))
                .or_insert_with(|| ActorTrack::new(actor));
        }
        self.tracks.retain(|id, _| retained.contains(id));
    }

    pub fn step(&mut self, dt_seconds: f32, config: ActorInterpolationConfig) {
        let factor = interpolation_factor(dt_seconds, config.half_life_seconds);
        for track in self.tracks.values_mut() {
            track.step(factor);
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
}

impl ActorTrack {
    fn new(actor: ActorPresentation) -> Self {
        Self {
            rendered: actor,
            target: actor,
        }
    }

    fn set_target(&mut self, actor: ActorPresentation) {
        self.target = actor;
        self.rendered.id = actor.id;
        self.rendered.kind = actor.kind;
        self.rendered.appearance = actor.appearance;
        self.rendered.on_ground = actor.on_ground;
        self.rendered.width = actor.width;
        self.rendered.height = actor.height;
        self.rendered.walk_animation_distance = actor.walk_animation_distance;
    }

    fn step(&mut self, factor: f32) {
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
            feet_position: Vec3d::new(x, 64.0, 2.0),
            y_rot_degrees,
            x_rot_degrees: 0.0,
            rotation: None,
            on_ground: true,
            width: 0.6,
            height: 1.8,
            walk_animation_distance: 0.0,
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
                feet_position: update.position,
                y_rot_degrees: update.y_rot_degrees,
                x_rot_degrees: update.x_rot_degrees,
                rotation: None,
                on_ground: update.on_ground,
                width: 0.6,
                height: 1.8,
                walk_animation_distance: 1.25,
            }
        );
    }

    #[test]
    fn actor_presentation_converts_entity_snapshot() {
        let snapshot = EntitySnapshot {
            id: EntityId(7),
            kind: EntityKind::Cow,
            position: Vec3d::new(10.0, 64.0, -4.0),
            y_rot_degrees: -90.0,
            x_rot_degrees: 0.0,
            rotation: Some(EntityRotation::IDENTITY),
            on_ground: true,
            width: 0.9,
            height: 1.4,
            age_ticks: 12,
        };

        assert_eq!(
            ActorPresentation::entity(snapshot),
            ActorPresentation {
                id: ActorPresentationId::Entity(snapshot.id),
                kind: ActorPresentationKind::Entity(snapshot.kind),
                appearance: ActorAppearance::NONE,
                feet_position: snapshot.position,
                y_rot_degrees: snapshot.y_rot_degrees,
                x_rot_degrees: snapshot.x_rot_degrees,
                rotation: snapshot.rotation,
                on_ground: snapshot.on_ground,
                width: snapshot.width,
                height: snapshot.height,
                walk_animation_distance: 0.0,
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
}
