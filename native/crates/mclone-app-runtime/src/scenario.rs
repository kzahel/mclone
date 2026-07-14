use serde::{Deserialize, Serialize};

/// Stable product identity for a built-in multi-world experience.
///
/// This value is intentionally path-free. Platform content executors resolve
/// managed storage before a scene receives a prepared scenario request.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltInScenarioId {
    LobbyPreview,
}

impl BuiltInScenarioId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::LobbyPreview => "Lobby Preview",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioLaunchIntent {
    pub id: BuiltInScenarioId,
}

impl ScenarioLaunchIntent {
    pub const fn new(id: BuiltInScenarioId) -> Self {
        Self { id }
    }

    pub const fn lobby_preview() -> Self {
        Self::new(BuiltInScenarioId::LobbyPreview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_intent_is_path_free_and_stably_serialized() {
        let intent = ScenarioLaunchIntent::lobby_preview();
        assert_eq!(intent.id.label(), "Lobby Preview");
        assert_eq!(
            serde_json::to_string(&intent).unwrap(),
            r#"{"id":"lobby-preview"}"#
        );
        assert_eq!(
            serde_json::from_str::<ScenarioLaunchIntent>(r#"{"id":"lobby-preview"}"#).unwrap(),
            intent
        );
    }
}
