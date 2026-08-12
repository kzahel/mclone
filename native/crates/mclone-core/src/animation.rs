use std::error::Error;
use std::fmt;

/// Maximum UTF-8 byte length of an authored animation clip identifier.
///
/// Canonical clip identifiers are deliberately ASCII resource-like names, so
/// this is also the maximum character count on the wire.
pub const MAX_ANIMATION_CLIP_ID_BYTES: usize = 48;

/// A compact, copyable authored animation clip identifier shared by gameplay,
/// protocol, presentation, and rendering without depending on an asset crate.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnimationClipId {
    len: u8,
    bytes: [u8; MAX_ANIMATION_CLIP_ID_BYTES],
}

impl AnimationClipId {
    pub const fn from_static(value: &'static str) -> Self {
        let source = value.as_bytes();
        assert!(!source.is_empty(), "animation clip id must not be empty");
        assert!(
            source.len() <= MAX_ANIMATION_CLIP_ID_BYTES,
            "animation clip id exceeds the maximum length"
        );
        let mut bytes = [0; MAX_ANIMATION_CLIP_ID_BYTES];
        let mut index = 0;
        while index < source.len() {
            assert!(
                valid_animation_clip_id_byte(source[index]),
                "animation clip id contains an invalid byte"
            );
            bytes[index] = source[index];
            index += 1;
        }
        Self {
            len: source.len() as u8,
            bytes,
        }
    }

    pub fn parse(value: &str) -> Result<Self, AnimationClipIdError> {
        if value.is_empty() {
            return Err(AnimationClipIdError::Empty);
        }
        if value.len() > MAX_ANIMATION_CLIP_ID_BYTES {
            return Err(AnimationClipIdError::TooLong { len: value.len() });
        }
        if !value.bytes().all(valid_animation_clip_id_byte) {
            return Err(AnimationClipIdError::InvalidCharacter);
        }
        let mut bytes = [0; MAX_ANIMATION_CLIP_ID_BYTES];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(Self {
            len: value.len() as u8,
            bytes,
        })
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .expect("validated animation clip ids are UTF-8")
    }
}

impl fmt::Debug for AnimationClipId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AnimationClipId")
            .field(&self.as_str())
            .finish()
    }
}

impl fmt::Display for AnimationClipId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnimationClipIdError {
    Empty,
    TooLong { len: usize },
    InvalidCharacter,
}

impl fmt::Display for AnimationClipIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("animation clip id must not be empty"),
            Self::TooLong { len } => write!(
                formatter,
                "animation clip id has {len} bytes; maximum is {MAX_ANIMATION_CLIP_ID_BYTES}"
            ),
            Self::InvalidCharacter => formatter.write_str(
                "animation clip id may contain only lowercase ASCII letters, digits, '_', and '-'",
            ),
        }
    }
}

impl Error for AnimationClipIdError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnimationPhaseSource {
    Distance,
    Elapsed,
}

/// Authoritative selection and transition timing for one semantic figure clip.
///
/// Distance clips use client presentation displacement for their phase.
/// Elapsed clips derive time from the replicated world tick and `start_tick`,
/// so hydration, delayed delivery, and render pauses do not restart actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnimationState {
    pub clip: AnimationClipId,
    pub phase_source: AnimationPhaseSource,
    pub epoch: u32,
    pub start_tick: u64,
}

impl AnimationState {
    pub const fn distance(clip: AnimationClipId, epoch: u32) -> Self {
        Self {
            clip,
            phase_source: AnimationPhaseSource::Distance,
            epoch,
            start_tick: 0,
        }
    }

    pub const fn elapsed(clip: AnimationClipId, epoch: u32, start_tick: u64) -> Self {
        Self {
            clip,
            phase_source: AnimationPhaseSource::Elapsed,
            epoch,
            start_tick,
        }
    }
}

const fn valid_animation_clip_id_byte(value: u8) -> bool {
    matches!(value, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_clip_id_round_trips_safe_authored_names() {
        const LIE_DOWN: AnimationClipId = AnimationClipId::from_static("lie_down");

        assert_eq!(LIE_DOWN.as_str(), "lie_down");
        assert_eq!(
            AnimationClipId::parse("bedded_idle"),
            Ok(AnimationClipId::from_static("bedded_idle"))
        );
    }

    #[test]
    fn animation_clip_id_rejects_unsafe_or_oversized_names() {
        assert_eq!(AnimationClipId::parse(""), Err(AnimationClipIdError::Empty));
        assert_eq!(
            AnimationClipId::parse("Walk"),
            Err(AnimationClipIdError::InvalidCharacter)
        );
        assert_eq!(
            AnimationClipId::parse(&"x".repeat(MAX_ANIMATION_CLIP_ID_BYTES + 1)),
            Err(AnimationClipIdError::TooLong {
                len: MAX_ANIMATION_CLIP_ID_BYTES + 1
            })
        );
    }

    #[test]
    fn animation_state_distinguishes_distance_and_elapsed_phases() {
        assert_eq!(
            AnimationState::distance(AnimationClipId::from_static("walk"), 3).phase_source,
            AnimationPhaseSource::Distance
        );
        assert_eq!(
            AnimationState::elapsed(AnimationClipId::from_static("lie_down"), 4, 120),
            AnimationState {
                clip: AnimationClipId::from_static("lie_down"),
                phase_source: AnimationPhaseSource::Elapsed,
                epoch: 4,
                start_tick: 120,
            }
        );
    }
}
