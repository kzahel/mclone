use mclone_worldgen::biome::{OverworldBiomeSource, possible_overworld_biomes};

const DEFAULT_START_SEED: i64 = 0;
const DEFAULT_MAX_SEEDS: u64 = 100_000;
const DEFAULT_COUNT: usize = 1;
const DEFAULT_CHUNK_X: i32 = 0;
const DEFAULT_CHUNK_Z: i32 = 0;
const MAX_COUNT: usize = 1_000;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(std::env::args().skip(1))?;
    if config.list_biomes {
        print_biomes();
        return Ok(());
    }

    let target_biome = config
        .biome_key
        .as_deref()
        .ok_or_else(|| format!("--biome is required\n{}", usage()))?;
    let matches = find_matches(&config, target_biome)?;
    print_matches(&config, target_biome, &matches);
    if matches.is_empty() {
        return Err(format!(
            "no seed found for {target_biome} at chunk ({}, {}) after searching {} seeds from {}",
            config.chunk_x, config.chunk_z, config.max_seeds, config.start_seed
        ));
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Config {
    biome_key: Option<String>,
    start_seed: i64,
    max_seeds: u64,
    count: usize,
    chunk_x: i32,
    chunk_z: i32,
    legacy_biome_init_layer: bool,
    large_biomes: bool,
    list_biomes: bool,
}

impl Config {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            biome_key: None,
            start_seed: DEFAULT_START_SEED,
            max_seeds: DEFAULT_MAX_SEEDS,
            count: DEFAULT_COUNT,
            chunk_x: DEFAULT_CHUNK_X,
            chunk_z: DEFAULT_CHUNK_Z,
            legacy_biome_init_layer: false,
            large_biomes: false,
            list_biomes: false,
        };

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--biome" => {
                    config.biome_key = Some(normalize_biome_key(&next_arg(&mut args, "--biome")?));
                }
                "--start-seed" => config.start_seed = parse_next(&mut args, "--start-seed")?,
                "--max-seeds" => config.max_seeds = parse_next(&mut args, "--max-seeds")?,
                "--count" => config.count = parse_next(&mut args, "--count")?,
                "--chunk-x" => config.chunk_x = parse_next(&mut args, "--chunk-x")?,
                "--chunk-z" => config.chunk_z = parse_next(&mut args, "--chunk-z")?,
                "--legacy-biome-init-layer" => config.legacy_biome_init_layer = true,
                "--large-biomes" => config.large_biomes = true,
                "--list-biomes" => config.list_biomes = true,
                "--" => {}
                "--help" | "-h" => return Err(usage()),
                _ => return Err(format!("unknown argument {arg}\n{}", usage())),
            }
        }

        if config.max_seeds == 0 {
            return Err("--max-seeds must be at least 1".to_owned());
        }
        if !(1..=MAX_COUNT).contains(&config.count) {
            return Err(format!("--count must be between 1 and {MAX_COUNT}"));
        }

        Ok(config)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Match {
    seed: i64,
    biome_key: &'static str,
}

fn find_matches(config: &Config, target_biome: &str) -> Result<Vec<Match>, String> {
    let mut matches = Vec::new();
    for offset in 0..config.max_seeds {
        let offset = i64::try_from(offset)
            .map_err(|_| "--max-seeds is too large for i64 seed scanning".to_owned())?;
        let seed = config
            .start_seed
            .checked_add(offset)
            .ok_or_else(|| "seed search overflowed i64".to_owned())?;
        let biome_source =
            OverworldBiomeSource::new(seed, config.legacy_biome_init_layer, config.large_biomes);
        let biome = biome_source.get_primary_biome_definition(config.chunk_x, config.chunk_z);
        if biome.key() == target_biome {
            matches.push(Match {
                seed,
                biome_key: biome.key(),
            });
            if matches.len() >= config.count {
                break;
            }
        }
    }
    Ok(matches)
}

fn print_matches(config: &Config, target_biome: &str, matches: &[Match]) {
    println!("{{");
    println!("  \"target_biome\": \"{target_biome}\",");
    println!("  \"chunk_x\": {},", config.chunk_x);
    println!("  \"chunk_z\": {},", config.chunk_z);
    println!("  \"start_seed\": {},", config.start_seed);
    println!("  \"searched_seeds_limit\": {},", config.max_seeds);
    println!(
        "  \"legacy_biome_init_layer\": {},",
        config.legacy_biome_init_layer
    );
    println!("  \"large_biomes\": {},", config.large_biomes);
    println!("  \"matches\": [");
    for (index, matched) in matches.iter().enumerate() {
        let comma = if index + 1 == matches.len() { "" } else { "," };
        println!(
            "    {{\"seed\": {}, \"chunk_x\": {}, \"chunk_z\": {}, \"primary_biome\": \"{}\"}}{comma}",
            matched.seed, config.chunk_x, config.chunk_z, matched.biome_key
        );
    }
    println!("  ]");
    println!("}}");
}

fn print_biomes() {
    for biome in possible_overworld_biomes() {
        println!("{}", biome.key());
    }
}

fn normalize_biome_key(value: &str) -> String {
    if value.contains(':') {
        value.to_owned()
    } else {
        format!("minecraft:{value}")
    }
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<T, String> {
    let value = next_arg(args, name)?;
    value
        .parse::<T>()
        .map_err(|_| format!("invalid value for {name}: {value}"))
}

fn next_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn usage() -> String {
    "usage: find_biome_seed --biome <key> [--start-seed N] [--max-seeds N] [--count N] [--chunk-x N] [--chunk-z N] [--legacy-biome-init-layer] [--large-biomes]\n       find_biome_seed --list-biomes"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_biome_keys_adds_default_namespace() {
        assert_eq!(normalize_biome_key("plains"), "minecraft:plains");
        assert_eq!(normalize_biome_key("minecraft:desert"), "minecraft:desert");
    }

    #[test]
    fn parse_defaults_to_zero_zero_seed_search() {
        let config = Config::parse(["--biome", "plains"].into_iter().map(str::to_owned)).unwrap();

        assert_eq!(
            config,
            Config {
                biome_key: Some("minecraft:plains".to_owned()),
                start_seed: 0,
                max_seeds: DEFAULT_MAX_SEEDS,
                count: DEFAULT_COUNT,
                chunk_x: 0,
                chunk_z: 0,
                legacy_biome_init_layer: false,
                large_biomes: false,
                list_biomes: false,
            }
        );
    }

    #[test]
    fn find_plains_seed_at_origin_from_small_range() {
        let config = Config {
            biome_key: Some("minecraft:plains".to_owned()),
            start_seed: 0,
            max_seeds: 20,
            count: 1,
            chunk_x: 0,
            chunk_z: 0,
            legacy_biome_init_layer: false,
            large_biomes: false,
            list_biomes: false,
        };

        let matches = find_matches(&config, "minecraft:plains").unwrap();
        assert_eq!(
            matches,
            vec![Match {
                seed: 16,
                biome_key: "minecraft:plains",
            }]
        );
    }
}
