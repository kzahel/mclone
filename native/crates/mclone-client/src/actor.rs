use std::collections::{BTreeMap, BTreeSet};

use mclone_core::Vec3d;
use mclone_protocol::{EntityId, EntityKind, EntitySnapshot, RemotePlayerId, RemotePlayerUpdate};

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorPresentation {
    pub id: ActorPresentationId,
    pub kind: ActorPresentationKind,
    /// Authoritative or interpolated feet position in world coordinates.
    pub feet_position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub on_ground: bool,
    pub width: f32,
    pub height: f32,
}

impl ActorPresentation {
    pub fn remote_player(update: RemotePlayerUpdate) -> Self {
        Self {
            id: ActorPresentationId::RemotePlayer(update.id),
            kind: ActorPresentationKind::RemotePlayer,
            feet_position: update.position,
            y_rot_degrees: update.y_rot_degrees,
            x_rot_degrees: update.x_rot_degrees,
            on_ground: update.on_ground,
            width: 0.6,
            height: 1.8,
        }
    }

    pub fn entity(snapshot: EntitySnapshot) -> Self {
        Self {
            id: ActorPresentationId::Entity(snapshot.id),
            kind: ActorPresentationKind::Entity(snapshot.kind),
            feet_position: snapshot.position,
            y_rot_degrees: snapshot.y_rot_degrees,
            x_rot_degrees: snapshot.x_rot_degrees,
            on_ground: snapshot.on_ground,
            width: snapshot.width,
            height: snapshot.height,
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
        self.rendered.on_ground = actor.on_ground;
        self.rendered.width = actor.width;
        self.rendered.height = actor.height;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(id: u64, x: f64, y_rot_degrees: f32) -> ActorPresentation {
        ActorPresentation {
            id: ActorPresentationId::RemotePlayer(RemotePlayerId(id)),
            kind: ActorPresentationKind::RemotePlayer,
            feet_position: Vec3d::new(x, 64.0, 2.0),
            y_rot_degrees,
            x_rot_degrees: 0.0,
            on_ground: true,
            width: 0.6,
            height: 1.8,
        }
    }

    #[test]
    fn actor_presentation_converts_remote_player_update() {
        let update = RemotePlayerUpdate {
            id: RemotePlayerId(3),
            position: Vec3d::new(10.0, 64.0, -4.0),
            y_rot_degrees: -90.0,
            x_rot_degrees: 15.0,
            on_ground: true,
        };

        assert_eq!(
            ActorPresentation::remote_player(update),
            ActorPresentation {
                id: ActorPresentationId::RemotePlayer(update.id),
                kind: ActorPresentationKind::RemotePlayer,
                feet_position: update.position,
                y_rot_degrees: update.y_rot_degrees,
                x_rot_degrees: update.x_rot_degrees,
                on_ground: update.on_ground,
                width: 0.6,
                height: 1.8,
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
                feet_position: snapshot.position,
                y_rot_degrees: snapshot.y_rot_degrees,
                x_rot_degrees: snapshot.x_rot_degrees,
                on_ground: snapshot.on_ground,
                width: snapshot.width,
                height: snapshot.height,
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
