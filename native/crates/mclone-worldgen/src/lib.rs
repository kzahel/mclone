#![forbid(unsafe_code)]

pub mod levelgen;
pub mod noise;
pub mod prng;

pub fn target_minecraft_version() -> &'static str {
    mclone_core::TARGET_MINECRAFT_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_core_target_version() {
        assert_eq!(target_minecraft_version(), "1.17.1");
    }
}
