use std::error::Error;
use std::fmt;

pub const DEFAULT_PLAYER_MAX_HEALTH: f32 = 20.0;
pub const PLAYER_STANDING_WIDTH: f64 = 0.6;
pub const PLAYER_STANDING_HEIGHT: f64 = 1.8;

/// Authoritative health facts for one player life.
///
/// Maximum health is explicit on the replica contract even though the first
/// ruleset fixes it at 20. This leaves room for attributes without encoding
/// death as a separate replacement for health.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerVitals {
    health: f32,
    max_health: f32,
}

impl PlayerVitals {
    pub fn new(health: f32, max_health: f32) -> Result<Self, PlayerVitalsError> {
        if !health.is_finite() || !max_health.is_finite() {
            return Err(PlayerVitalsError::NonFinite);
        }
        if max_health <= 0.0 {
            return Err(PlayerVitalsError::NonPositiveMaximum);
        }
        if health < 0.0 || health > max_health {
            return Err(PlayerVitalsError::HealthOutOfRange);
        }
        Ok(Self { health, max_health })
    }

    pub const fn full_health() -> Self {
        Self {
            health: DEFAULT_PLAYER_MAX_HEALTH,
            max_health: DEFAULT_PLAYER_MAX_HEALTH,
        }
    }

    pub const fn health(self) -> f32 {
        self.health
    }

    pub const fn max_health(self) -> f32 {
        self.max_health
    }

    pub fn is_dead(self) -> bool {
        self.health <= 0.0
    }

    pub fn with_health(self, health: f32) -> Result<Self, PlayerVitalsError> {
        Self::new(health, self.max_health)
    }
}

impl Default for PlayerVitals {
    fn default() -> Self {
        Self::full_health()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerVitalsError {
    NonFinite,
    NonPositiveMaximum,
    HealthOutOfRange,
}

impl fmt::Display for PlayerVitalsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => f.write_str("player health must be finite"),
            Self::NonPositiveMaximum => {
                f.write_str("player maximum health must be greater than zero")
            }
            Self::HealthOutOfRange => {
                f.write_str("player health must be between zero and maximum health")
            }
        }
    }
}

impl Error for PlayerVitalsError {}

/// Stable machine-readable cause for one completed player death.
///
/// Presentation derives localized copy from this value. It is never a
/// user-facing string in protocol or persistence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerDamageCause {
    Lava,
}

/// One atomic owner-only lifecycle snapshot.
///
/// Health and the completed-death cause travel together so clients cannot
/// observe zero health without the screen-driving cause. The monotonic epoch
/// rejects stale snapshots around later respawns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerLifeState {
    epoch: u32,
    vitals: PlayerVitals,
    death_cause: Option<PlayerDamageCause>,
}

impl PlayerLifeState {
    pub fn new(
        epoch: u32,
        vitals: PlayerVitals,
        death_cause: Option<PlayerDamageCause>,
    ) -> Result<Self, PlayerLifeStateError> {
        if vitals.is_dead() != death_cause.is_some() {
            return Err(PlayerLifeStateError::InconsistentDeathCause);
        }
        Ok(Self {
            epoch,
            vitals,
            death_cause,
        })
    }

    pub const fn living() -> Self {
        Self {
            epoch: 0,
            vitals: PlayerVitals::full_health(),
            death_cause: None,
        }
    }

    pub const fn epoch(self) -> u32 {
        self.epoch
    }

    pub const fn vitals(self) -> PlayerVitals {
        self.vitals
    }

    pub const fn death_cause(self) -> Option<PlayerDamageCause> {
        self.death_cause
    }

    pub fn is_dead(self) -> bool {
        self.vitals.is_dead()
    }
}

impl Default for PlayerLifeState {
    fn default() -> Self {
        Self::living()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerLifeStateError {
    InconsistentDeathCause,
}

impl fmt::Display for PlayerLifeStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InconsistentDeathCause => {
                f.write_str("player death cause must be present exactly when health is zero")
            }
        }
    }
}

impl Error for PlayerLifeStateError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_vitals_are_twenty_out_of_twenty() {
        let vitals = PlayerVitals::default();
        assert_eq!(vitals.health(), 20.0);
        assert_eq!(vitals.max_health(), 20.0);
        assert!(!vitals.is_dead());
    }

    #[test]
    fn vitals_reject_non_finite_and_out_of_range_values() {
        assert_eq!(
            PlayerVitals::new(f32::NAN, 20.0),
            Err(PlayerVitalsError::NonFinite)
        );
        assert_eq!(
            PlayerVitals::new(1.0, f32::INFINITY),
            Err(PlayerVitalsError::NonFinite)
        );
        assert_eq!(
            PlayerVitals::new(0.0, 0.0),
            Err(PlayerVitalsError::NonPositiveMaximum)
        );
        assert_eq!(
            PlayerVitals::new(-1.0, 20.0),
            Err(PlayerVitalsError::HealthOutOfRange)
        );
        assert_eq!(
            PlayerVitals::new(21.0, 20.0),
            Err(PlayerVitalsError::HealthOutOfRange)
        );
        assert!(PlayerVitals::new(0.0, 20.0).unwrap().is_dead());
    }

    #[test]
    fn life_state_keeps_zero_health_and_death_cause_atomic() {
        let dead = PlayerLifeState::new(
            7,
            PlayerVitals::new(0.0, 20.0).unwrap(),
            Some(PlayerDamageCause::Lava),
        )
        .unwrap();
        assert!(dead.is_dead());
        assert_eq!(dead.epoch(), 7);
        assert_eq!(dead.death_cause(), Some(PlayerDamageCause::Lava));
        assert_eq!(
            PlayerLifeState::new(
                8,
                PlayerVitals::full_health(),
                Some(PlayerDamageCause::Lava)
            ),
            Err(PlayerLifeStateError::InconsistentDeathCause)
        );
    }
}
