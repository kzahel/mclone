use anyhow::{Result, bail};
use mclone_core::ChunkPos;
use mclone_render::placement::EmbeddedChunkRegion;
use serde::{Deserialize, Serialize};

pub const MAX_SCENARIO_PREVIEW_CHUNK_SPAN: u32 = 16;

/// Path-free source bounds resolved around a destination's accepted entry.
///
/// Chunk and section offsets are inclusive. They remain independent of the
/// destination runtime's interest/tracking radius: a bounded 2x2 tabletop crop
/// can activate into an ordinary unbounded world.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioPreviewBounds {
    pub min_chunk_offset: [i32; 2],
    pub max_chunk_offset: [i32; 2],
    pub min_section_offset: i32,
    pub max_section_offset: i32,
}

impl ScenarioPreviewBounds {
    pub const DEFAULT: Self = Self {
        min_chunk_offset: [-1, -1],
        max_chunk_offset: [0, 0],
        min_section_offset: -1,
        max_section_offset: 1,
    };

    pub fn square(chunk_span: u32) -> Result<Self> {
        if chunk_span == 0 || chunk_span > MAX_SCENARIO_PREVIEW_CHUNK_SPAN {
            bail!(
                "scenario preview chunk span must be between 1 and {MAX_SCENARIO_PREVIEW_CHUNK_SPAN}"
            );
        }
        let span = i32::try_from(chunk_span).expect("bounded chunk span fits i32");
        let min = -(span / 2);
        let max = min + span - 1;
        Ok(Self {
            min_chunk_offset: [min, min],
            max_chunk_offset: [max, max],
            ..Self::DEFAULT
        })
    }

    pub fn validated(self) -> Result<Self> {
        if self.min_chunk_offset[0] > self.max_chunk_offset[0]
            || self.min_chunk_offset[1] > self.max_chunk_offset[1]
        {
            bail!("scenario preview minimum chunk offsets must not exceed maximum offsets");
        }
        if self.min_section_offset > self.max_section_offset {
            bail!("scenario preview minimum section offset must not exceed maximum offset");
        }
        let width = inclusive_offset_span(self.min_chunk_offset[0], self.max_chunk_offset[0])?;
        let depth = inclusive_offset_span(self.min_chunk_offset[1], self.max_chunk_offset[1])?;
        if width > MAX_SCENARIO_PREVIEW_CHUNK_SPAN || depth > MAX_SCENARIO_PREVIEW_CHUNK_SPAN {
            bail!(
                "scenario preview chunk bounds may span at most {MAX_SCENARIO_PREVIEW_CHUNK_SPAN} chunks per axis"
            );
        }
        Ok(self)
    }

    pub fn resolve(
        self,
        entry_chunk: ChunkPos,
        entry_section_y: i32,
    ) -> Result<EmbeddedChunkRegion> {
        let validated = self.validated()?;
        EmbeddedChunkRegion::from_chunk_bounds(
            ChunkPos::new(
                checked_offset(entry_chunk.x, validated.min_chunk_offset[0])?,
                checked_offset(entry_chunk.z, validated.min_chunk_offset[1])?,
            ),
            ChunkPos::new(
                checked_offset(entry_chunk.x, validated.max_chunk_offset[0])?,
                checked_offset(entry_chunk.z, validated.max_chunk_offset[1])?,
            ),
            checked_offset(entry_section_y, validated.min_section_offset)?,
            checked_offset(entry_section_y, validated.max_section_offset)?,
        )
    }
}

impl Default for ScenarioPreviewBounds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

fn inclusive_offset_span(min: i32, max: i32) -> Result<u32> {
    u32::try_from(i64::from(max) - i64::from(min) + 1)
        .map_err(|_| anyhow::anyhow!("scenario preview chunk offset span is invalid"))
}

fn checked_offset(value: i32, offset: i32) -> Result<i32> {
    value
        .checked_add(offset)
        .ok_or_else(|| anyhow::anyhow!("scenario preview bounds exceed coordinate range"))
}

fn scenario_preview_bounds_are_default(bounds: &ScenarioPreviewBounds) -> bool {
    *bounds == ScenarioPreviewBounds::DEFAULT
}

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
    #[serde(default, skip_serializing_if = "scenario_preview_bounds_are_default")]
    pub preview_bounds: ScenarioPreviewBounds,
}

impl ScenarioLaunchIntent {
    pub const fn new(id: BuiltInScenarioId) -> Self {
        Self {
            id,
            preview_bounds: ScenarioPreviewBounds::DEFAULT,
        }
    }

    pub const fn lobby_preview() -> Self {
        Self::new(BuiltInScenarioId::LobbyPreview)
    }

    pub fn with_preview_bounds(mut self, preview_bounds: ScenarioPreviewBounds) -> Result<Self> {
        self.preview_bounds = preview_bounds.validated()?;
        Ok(self)
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

    #[test]
    fn preview_bounds_resolve_exact_two_and_four_chunk_squares() {
        let entry = ChunkPos::new(7, -3);
        let two = ScenarioPreviewBounds::square(2)
            .unwrap()
            .resolve(entry, 5)
            .unwrap();
        let four = ScenarioPreviewBounds::square(4)
            .unwrap()
            .resolve(entry, 5)
            .unwrap();

        assert_eq!((two.chunk_width(), two.chunk_depth()), (2, 2));
        assert_eq!(two.min_chunk(), ChunkPos::new(6, -4));
        assert_eq!(two.max_chunk(), ChunkPos::new(7, -3));
        assert_eq!((four.chunk_width(), four.chunk_depth()), (4, 4));
        assert_eq!(four.min_chunk(), ChunkPos::new(5, -5));
        assert_eq!(four.max_chunk(), ChunkPos::new(8, -2));
        assert_eq!((four.min_section_y(), four.max_section_y()), (4, 6));
    }

    #[test]
    fn preview_bounds_are_validated_and_nondefault_bounds_serialize() {
        assert!(ScenarioPreviewBounds::square(0).is_err());
        assert!(ScenarioPreviewBounds::square(17).is_err());
        let intent = ScenarioLaunchIntent::lobby_preview()
            .with_preview_bounds(ScenarioPreviewBounds::square(4).unwrap())
            .unwrap();
        let json = serde_json::to_string(&intent).unwrap();
        assert!(json.contains(r#""minChunkOffset":[-2,-2]"#));
        assert_eq!(
            serde_json::from_str::<ScenarioLaunchIntent>(&json).unwrap(),
            intent
        );
    }
}
