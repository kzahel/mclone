use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

pub const MAX_STATISTIC_RESOURCE_KEY_BYTES: usize = 255;
pub const MAX_PLAYER_STATISTIC_ENTRIES: usize = 4_096;
pub const MAX_PLAYER_STATISTIC_VALUE: u32 = i32::MAX as u32;

pub const CUSTOM_STATISTIC_TYPE_KEY: &str = "minecraft:custom";
pub const JUMP_STATISTIC_VALUE_KEY: &str = "minecraft:jump";
pub const DEATHS_STATISTIC_VALUE_KEY: &str = "minecraft:deaths";
pub const MCLONE_CUSTOM_STATISTIC_TYPE_KEY: &str = "mclone:custom";
pub const SUCCESSFUL_BLOCK_PLACEMENT_STATISTIC_VALUE_KEY: &str =
    "mclone:successful_block_placements";

/// Vanilla-shaped identity for one statistic: a registered statistic type and
/// a registered value within that type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatisticKey {
    statistic_type: String,
    value: String,
}

impl StatisticKey {
    pub fn new(
        statistic_type: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<Self, StatisticKeyError> {
        let statistic_type = statistic_type.as_ref();
        let value = value.as_ref();
        validate_resource_key(statistic_type)?;
        validate_resource_key(value)?;
        Ok(Self {
            statistic_type: statistic_type.to_owned(),
            value: value.to_owned(),
        })
    }

    pub fn jump() -> Self {
        Self::new(CUSTOM_STATISTIC_TYPE_KEY, JUMP_STATISTIC_VALUE_KEY)
            .expect("built-in jump statistic key must remain valid")
    }

    pub fn successful_block_placement() -> Self {
        Self::new(
            MCLONE_CUSTOM_STATISTIC_TYPE_KEY,
            SUCCESSFUL_BLOCK_PLACEMENT_STATISTIC_VALUE_KEY,
        )
        .expect("built-in block-placement statistic key must remain valid")
    }

    pub fn deaths() -> Self {
        Self::new(CUSTOM_STATISTIC_TYPE_KEY, DEATHS_STATISTIC_VALUE_KEY)
            .expect("built-in death statistic key must remain valid")
    }

    pub fn statistic_type(&self) -> &str {
        &self.statistic_type
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatisticKeyError {
    MissingNamespace,
    MultipleSeparators,
    EmptyNamespace,
    EmptyPath,
    InvalidNamespaceCharacter(char),
    InvalidPathCharacter(char),
    TooLong { len: usize, max: usize },
}

impl fmt::Display for StatisticKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNamespace => {
                f.write_str("statistic resource key must contain an explicit namespace")
            }
            Self::MultipleSeparators => {
                f.write_str("statistic resource key must contain exactly one ':' separator")
            }
            Self::EmptyNamespace => {
                f.write_str("statistic resource key namespace must not be empty")
            }
            Self::EmptyPath => f.write_str("statistic resource key path must not be empty"),
            Self::InvalidNamespaceCharacter(ch) => {
                write!(f, "invalid statistic resource namespace character `{ch}`")
            }
            Self::InvalidPathCharacter(ch) => {
                write!(f, "invalid statistic resource path character `{ch}`")
            }
            Self::TooLong { len, max } => {
                write!(f, "statistic resource key is {len} bytes; maximum is {max}")
            }
        }
    }
}

impl Error for StatisticKeyError {}

/// Authoritative per-player values in one realm.
///
/// Vanilla 1.17.1 caps statistics at `Integer.MAX_VALUE`; the same cap keeps
/// increments deterministic across persistence and protocol boundaries.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlayerStatistics {
    values: BTreeMap<StatisticKey, u32>,
}

impl PlayerStatistics {
    pub fn get(&self, key: &StatisticKey) -> u32 {
        self.values.get(key).copied().unwrap_or(0)
    }

    pub fn jump_count(&self) -> u32 {
        self.get(&StatisticKey::jump())
    }

    pub fn successful_block_placement_count(&self) -> u32 {
        self.get(&StatisticKey::successful_block_placement())
    }

    pub fn death_count(&self) -> u32 {
        self.get(&StatisticKey::deaths())
    }

    pub fn set(&mut self, key: StatisticKey, value: u32) {
        if value == 0 {
            self.values.remove(&key);
        } else {
            self.values
                .insert(key, value.min(MAX_PLAYER_STATISTIC_VALUE));
        }
    }

    pub fn increment(&mut self, key: StatisticKey, amount: u32) -> u32 {
        let value = self
            .get(&key)
            .saturating_add(amount)
            .min(MAX_PLAYER_STATISTIC_VALUE);
        self.set(key, value);
        value
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&StatisticKey, &u32)> {
        self.values.iter()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

fn validate_resource_key(value: &str) -> Result<(), StatisticKeyError> {
    if value.len() > MAX_STATISTIC_RESOURCE_KEY_BYTES {
        return Err(StatisticKeyError::TooLong {
            len: value.len(),
            max: MAX_STATISTIC_RESOURCE_KEY_BYTES,
        });
    }
    let Some((namespace, path)) = value.split_once(':') else {
        return Err(StatisticKeyError::MissingNamespace);
    };
    if path.contains(':') {
        return Err(StatisticKeyError::MultipleSeparators);
    }
    if namespace.is_empty() {
        return Err(StatisticKeyError::EmptyNamespace);
    }
    if path.is_empty() {
        return Err(StatisticKeyError::EmptyPath);
    }
    if let Some(ch) = namespace
        .chars()
        .find(|ch| !matches!(ch, 'a'..='z' | '0'..='9' | '_' | '-' | '.'))
    {
        return Err(StatisticKeyError::InvalidNamespaceCharacter(ch));
    }
    if let Some(ch) = path
        .chars()
        .find(|ch| !matches!(ch, 'a'..='z' | '0'..='9' | '_' | '-' | '.' | '/'))
    {
        return Err(StatisticKeyError::InvalidPathCharacter(ch));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_keys_match_vanilla_shaped_identifiers() {
        let jump = StatisticKey::jump();
        assert_eq!(jump.statistic_type(), "minecraft:custom");
        assert_eq!(jump.value(), "minecraft:jump");

        let placed = StatisticKey::successful_block_placement();
        assert_eq!(placed.statistic_type(), "mclone:custom");
        assert_eq!(placed.value(), "mclone:successful_block_placements");

        let deaths = StatisticKey::deaths();
        assert_eq!(deaths.statistic_type(), "minecraft:custom");
        assert_eq!(deaths.value(), "minecraft:deaths");
    }

    #[test]
    fn statistics_default_to_zero_and_cap_like_vanilla() {
        let mut statistics = PlayerStatistics::default();
        let jump = StatisticKey::jump();
        assert_eq!(statistics.get(&jump), 0);
        assert_eq!(statistics.increment(jump.clone(), 1), 1);
        assert_eq!(
            statistics.increment(jump.clone(), u32::MAX),
            i32::MAX as u32
        );
        assert_eq!(statistics.get(&jump), i32::MAX as u32);
    }

    #[test]
    fn resource_keys_require_explicit_lowercase_namespaces() {
        assert!(StatisticKey::new("minecraft:custom", "mclone:planets/red_mars").is_ok());
        assert_eq!(
            StatisticKey::new("custom", "minecraft:jump"),
            Err(StatisticKeyError::MissingNamespace)
        );
        assert!(StatisticKey::new("Minecraft:custom", "minecraft:jump").is_err());
        assert!(StatisticKey::new("minecraft:custom", "minecraft:the jump").is_err());
    }
}
