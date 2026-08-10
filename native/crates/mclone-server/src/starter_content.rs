use serde::{Deserialize, Serialize};

/// Versioned content layered over a world's base generation descriptor.
///
/// This identity is deliberately orthogonal to terrain generation. `Wild`
/// preserves the historical behavior; other variants opt into an explicit
/// overlay without changing the base profile's seed parity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StarterContentDescriptor {
    #[default]
    Wild,
    IntroHomesteadV1,
}

impl StarterContentDescriptor {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Wild => "wild",
            Self::IntroHomesteadV1 => "intro-homestead-v1",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Wild => "Wild Start",
            Self::IntroHomesteadV1 => "Homestead Start",
        }
    }

    pub(crate) const fn codec_tag(self) -> u8 {
        match self {
            Self::Wild => 0,
            Self::IntroHomesteadV1 => 1,
        }
    }

    pub(crate) const fn from_codec_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Wild),
            1 => Some(Self::IntroHomesteadV1),
            _ => None,
        }
    }
}

/// Stable identity for a persisted realized starter plan.
///
/// The plan body is stored independently. This compact identity lets world
/// metadata bind that body to the planner/composition revisions and checksum
/// that produced it before any plan-owned chunks become authoritative.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealizedStarterPlanIdentity {
    pub planner_revision: u32,
    pub composition_revision: u32,
    pub checksum: [u8; 32],
}

impl RealizedStarterPlanIdentity {
    pub const fn new(planner_revision: u32, composition_revision: u32, checksum: [u8; 32]) -> Self {
        Self {
            planner_revision,
            composition_revision,
            checksum,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_content_json_ids_are_stable() {
        assert_eq!(
            serde_json::to_string(&StarterContentDescriptor::Wild).unwrap(),
            "\"wild\""
        );
        assert_eq!(
            serde_json::to_string(&StarterContentDescriptor::IntroHomesteadV1).unwrap(),
            "\"intro-homestead-v1\""
        );
    }
}
