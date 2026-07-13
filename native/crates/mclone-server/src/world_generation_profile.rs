use serde::{Deserialize, Serialize};

pub const AUTHORED_WORLD_MIN_Y: i32 = 0;
pub const AUTHORED_WORLD_HEIGHT: i32 = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldGenerationProfile {
    #[default]
    Overworld,
    AuthoredOnly {
        #[serde(rename = "missingChunk")]
        missing_chunk: AuthoredMissingChunk,
    },
}

impl WorldGenerationProfile {
    pub const fn authored_only() -> Self {
        Self::AuthoredOnly {
            missing_chunk: AuthoredMissingChunk::Void,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Overworld => "overworld",
            Self::AuthoredOnly { .. } => "authored-only",
        }
    }

    pub fn parse_label(value: &str) -> Result<Self, String> {
        match value.trim() {
            "overworld" | "default" => Ok(Self::Overworld),
            "authored-only" | "authored_only" | "authoredOnly" => Ok(Self::authored_only()),
            value => Err(format!(
                "world generation profile must be overworld or authored-only, got `{value}`"
            )),
        }
    }

    pub const fn authored_missing_chunk(self) -> Option<AuthoredMissingChunk> {
        match self {
            Self::Overworld => None,
            Self::AuthoredOnly { missing_chunk } => Some(missing_chunk),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthoredMissingChunk {
    Void,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_defaults_and_labels_are_backward_compatible() {
        assert_eq!(
            WorldGenerationProfile::default(),
            WorldGenerationProfile::Overworld
        );
        assert_eq!(
            WorldGenerationProfile::parse_label("authored-only").unwrap(),
            WorldGenerationProfile::authored_only()
        );
        assert_eq!(
            serde_json::from_str::<WorldGenerationProfile>("\"overworld\"").unwrap(),
            WorldGenerationProfile::Overworld
        );
        assert_eq!(
            serde_json::from_str::<WorldGenerationProfile>(
                r#"{"authoredOnly":{"missingChunk":"void"}}"#,
            )
            .unwrap(),
            WorldGenerationProfile::authored_only()
        );
    }
}
