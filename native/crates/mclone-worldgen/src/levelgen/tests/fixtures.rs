use super::*;

pub(super) fn fixtures() -> Vec<NoiseSamplerFixture> {
    [
        include_str!("../../../../../../test/fixtures/noise/noise-sampler-overworld-seed-0.json"),
        include_str!("../../../../../../test/fixtures/noise/noise-sampler-overworld-seed-1.json"),
        include_str!("../../../../../../test/fixtures/noise/noise-sampler-overworld-seed-12345.json"),
        include_str!(
            "../../../../../../test/fixtures/noise/noise-sampler-overworld-seed-2151901553968352745.json"
        ),
    ]
    .into_iter()
    .map(|json| serde_json::from_str(json).expect("valid NoiseSampler fixture"))
    .collect()
}

pub(super) fn terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0-terrain-only.json"
    ))
    .expect("valid terrain chunk oracle fixture")
}

pub(super) fn surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0-surface-only.json"
    ))
    .expect("valid surface chunk oracle fixture")
}

pub(super) fn mountains_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-33-chunks-0-0-terrain-only.json"
    ))
    .expect("valid mountains terrain chunk oracle fixture")
}

pub(super) fn mountains_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-33-chunks-0-0-surface-only.json"
    ))
    .expect("valid mountains surface chunk oracle fixture")
}

pub(super) fn mountains_relief_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-33-chunks--12-9-terrain-only.json"
    ))
    .expect("valid mountains relief terrain chunk oracle fixture")
}

pub(super) fn mountains_relief_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-33-chunks--12-9-surface-only.json"
    ))
    .expect("valid mountains relief surface chunk oracle fixture")
}

pub(super) fn gravelly_mountains_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-250-chunks-0-0-terrain-only.json"
    ))
    .expect("valid gravelly mountains terrain chunk oracle fixture")
}

pub(super) fn gravelly_mountains_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-250-chunks-0-0-surface-only.json"
    ))
    .expect("valid gravelly mountains surface chunk oracle fixture")
}

pub(super) fn gravelly_mountains_relief_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-250-chunks--3-13-terrain-only.json"
    ))
    .expect("valid gravelly mountains relief terrain chunk oracle fixture")
}

pub(super) fn gravelly_mountains_relief_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-250-chunks--3-13-surface-only.json"
    ))
    .expect("valid gravelly mountains relief surface chunk oracle fixture")
}

pub(super) fn snowy_mountains_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-326-chunks-0-0-terrain-only.json"
    ))
    .expect("valid snowy mountains terrain chunk oracle fixture")
}

pub(super) fn snowy_mountains_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-326-chunks-0-0-surface-only.json"
    ))
    .expect("valid snowy mountains surface chunk oracle fixture")
}

pub(super) fn snowy_mountains_relief_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-326-chunks-4--21-terrain-only.json"
    ))
    .expect("valid snowy mountains relief terrain chunk oracle fixture")
}

pub(super) fn snowy_mountains_relief_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-326-chunks-4--21-surface-only.json"
    ))
    .expect("valid snowy mountains relief surface chunk oracle fixture")
}

pub(super) fn frozen_ocean_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-12345-chunks--247--247-surface-only.json"
    ))
    .expect("valid frozen ocean surface chunk oracle fixture")
}

pub(super) fn badlands_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-12345-chunks--320-99-surface-only.json"
    ))
    .expect("valid badlands surface chunk oracle fixture")
}

pub(super) fn eroded_badlands_pillar_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-868-chunks-8--6-surface-only.json"
    ))
    .expect("valid eroded badlands pillar surface chunk oracle fixture")
}

pub(super) fn stone_shore_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-74739-chunks-0-0-terrain-only.json"
    ))
    .expect("valid stone shore terrain chunk oracle fixture")
}

pub(super) fn stone_shore_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-74739-chunks-0-0-surface-only.json"
    ))
    .expect("valid stone shore surface chunk oracle fixture")
}

pub(super) fn stone_shore_edge_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-74739-chunks-6-8-terrain-only.json"
    ))
    .expect("valid stone shore edge terrain chunk oracle fixture")
}

pub(super) fn stone_shore_edge_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-74739-chunks-6-8-surface-only.json"
    ))
    .expect("valid stone shore edge surface chunk oracle fixture")
}

pub(super) fn shattered_savanna_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-68-chunks--6-0-terrain-only.json"
    ))
    .expect("valid shattered savanna terrain chunk oracle fixture")
}

pub(super) fn shattered_savanna_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-68-chunks--6-0-surface-only.json"
    ))
    .expect("valid shattered savanna surface chunk oracle fixture")
}

pub(super) fn shattered_savanna_plateau_terrain_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-153-chunks--8--2-terrain-only.json"
    ))
    .expect("valid shattered savanna plateau terrain chunk oracle fixture")
}

pub(super) fn shattered_savanna_plateau_surface_fixture() -> TerrainChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-153-chunks--8--2-surface-only.json"
    ))
    .expect("valid shattered savanna plateau surface chunk oracle fixture")
}

pub(super) fn full_chunk_fixture() -> FullChunkOracleFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/integration/overworld-seed-12345-chunks-0-0.json"
    ))
    .expect("valid full chunk oracle fixture")
}

pub(super) fn scheduler_trace_fixture() -> SchedulerTraceFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/scheduler/vanilla-scheduler-trace-seed-12345-chunk-0-0-spawn-bootstrap.json"
    ))
    .expect("valid vanilla scheduler trace fixture")
}

pub(super) fn scheduler_features_snapshot_fixture() -> SchedulerTraceFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-12345-chunk-0-0.json"
    ))
    .expect("valid vanilla scheduler features snapshot fixture")
}

pub(super) fn plains_scheduler_features_snapshot_fixture() -> SchedulerTraceFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-16-chunk-0-0-plains.json"
    ))
    .expect("valid vanilla plains scheduler features snapshot fixture")
}

pub(super) fn inland_plains_scheduler_features_snapshot_fixture() -> SchedulerTraceFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-17-chunk-0-0-plains-inland.json"
    ))
    .expect("valid vanilla inland plains scheduler features snapshot fixture")
}

pub(super) fn forest_seed_23823_scheduler_features_snapshot_fixture() -> SchedulerTraceFixture {
    serde_json::from_str(include_str!(
        "../../../../../../test/fixtures/scheduler/vanilla-scheduler-features-snapshot-seed-23823-chunk--13-8-forest.json"
    ))
    .expect("valid vanilla forest scheduler features snapshot fixture")
}

pub(super) fn create_noise_settings(fixture: &NoiseSamplerFixture) -> NoiseSettings {
    let settings = &fixture.noise_settings;
    NoiseSettings::create(
        settings.min_y,
        settings.height,
        NoiseSamplingSettings::new(
            settings.sampling.xz_scale,
            settings.sampling.y_scale,
            settings.sampling.xz_factor,
            settings.sampling.y_factor,
        ),
        NoiseSlideSettings::new(
            settings.top_slide.target,
            settings.top_slide.size,
            settings.top_slide.offset,
        ),
        NoiseSlideSettings::new(
            settings.bottom_slide.target,
            settings.bottom_slide.size,
            settings.bottom_slide.offset,
        ),
        settings.noise_size_horizontal,
        settings.noise_size_vertical,
        settings.density_factor,
        settings.density_offset,
        settings.use_simplex_surface_noise,
        settings.random_density_offset,
        settings.island_noise_override,
        settings.is_amplified,
    )
}

pub(super) fn create_noise_sampler(
    fixture: &NoiseSamplerFixture,
    pattern: &NoiseSamplerBiomePatternFixture,
) -> NoiseSampler<RepeatingPatternBiomeSource> {
    let settings = create_noise_settings(fixture);
    let seed = fixture.seed.parse::<i64>().expect("i64 fixture seed");
    let mut random = WorldgenRandom::new(seed);
    let blended_noise = BlendedNoise::new(&mut random);
    random.consume_count(2620);
    let depth_noise = PerlinNoise::from_octaves(&mut random, &DEPTH_NOISE_OCTAVES);

    NoiseSampler::new(
        RepeatingPatternBiomeSource::new(pattern),
        fixture.cell_width,
        fixture.cell_height,
        fixture.cell_count_y,
        settings,
        blended_noise,
        None,
        depth_noise,
        NoiseModifier::Passthrough,
    )
}

pub(super) fn assert_panic_message(work: impl FnOnce() + panic::UnwindSafe, expected: &str) {
    let panic = panic::catch_unwind(work).expect_err("expected panic");
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .expect("panic message");
    assert_eq!(message, expected);
}
