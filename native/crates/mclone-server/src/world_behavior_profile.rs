use serde::{Deserialize, Serialize};

/// Authoritative player-mutation policy for one hosted world.
///
/// This is deliberately separate from terrain generation. A protected lobby
/// still has ordinary writable persistence and simulation bookkeeping; it
/// simply refuses player break/place commands at the server boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorldBehaviorProfile {
    #[default]
    Mutable,
    ProtectedLobby,
}

impl WorldBehaviorProfile {
    pub const fn allows_player_break(self) -> bool {
        matches!(self, Self::Mutable)
    }

    pub const fn allows_player_place(self) -> bool {
        matches!(self, Self::Mutable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutable_is_default_and_protected_lobby_denies_player_mutation() {
        assert_eq!(
            WorldBehaviorProfile::default(),
            WorldBehaviorProfile::Mutable
        );
        assert!(WorldBehaviorProfile::Mutable.allows_player_break());
        assert!(WorldBehaviorProfile::Mutable.allows_player_place());
        assert!(!WorldBehaviorProfile::ProtectedLobby.allows_player_break());
        assert!(!WorldBehaviorProfile::ProtectedLobby.allows_player_place());
        assert_eq!(
            serde_json::to_string(&WorldBehaviorProfile::ProtectedLobby).unwrap(),
            r#""protected-lobby""#
        );
    }
}
