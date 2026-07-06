use super::*;

#[test]
fn matches_java_oracle_across_sampled_cell_columns() {
    for fixture in fixtures() {
        let settings = create_noise_settings(&fixture);

        for (sample_set_name, sample_set) in &fixture.sample_sets {
            let sampler = create_noise_sampler(&fixture, &sample_set.biome_pattern);
            let mut column = vec![0.0; fixture.column_value_count];
            let mut index = 0;

            for cell_x in &sample_set.columns.x {
                for cell_z in &sample_set.columns.z {
                    sampler.fill_noise_column(
                        &mut column,
                        *cell_x,
                        *cell_z,
                        &settings,
                        fixture.biome_y,
                        fixture.min_cell_y,
                        fixture.cell_count_y,
                    );

                    for (y_index, actual) in column.iter().enumerate() {
                        let expected = sample_set.columns.values[index];
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "noise-sampler(seed={}, sampleSet={sample_set_name}) mismatch at flattened index {index} for cell ({cell_x}, {cell_z}) and column index {y_index}: expected {expected}, got {actual}",
                            fixture.seed
                        );
                        index += 1;
                    }
                }
            }
        }
    }
}

#[test]
fn noise_settings_create_enforces_java_min_y_height_guard() {
    let sampling = NoiseSamplingSettings::new(1.0, 1.0, 80.0, 160.0);
    let top_slide = NoiseSlideSettings::new(-10, 3, 0);
    let bottom_slide = NoiseSlideSettings::new(15, 3, 0);

    assert_panic_message(
        || {
            NoiseSettings::create(
                0,
                255,
                sampling,
                top_slide,
                bottom_slide,
                1,
                2,
                1.0,
                -0.46875,
                true,
                true,
                false,
                false,
            );
        },
        "height has to be a multiple of 16",
    );
    assert_panic_message(
        || {
            NoiseSettings::create(
                1,
                256,
                sampling,
                top_slide,
                bottom_slide,
                1,
                2,
                1.0,
                -0.46875,
                true,
                true,
                false,
                false,
            );
        },
        "min_y has to be a multiple of 16",
    );
    assert_panic_message(
        || {
            NoiseSettings::create(
                0,
                2048 + 16,
                sampling,
                top_slide,
                bottom_slide,
                1,
                2,
                1.0,
                -0.46875,
                true,
                true,
                false,
                false,
            );
        },
        "min_y + height cannot be higher than: 2032",
    );
}

#[test]
fn island_noise_override_is_rejected_in_this_overworld_slice() {
    let fixture = fixtures()
        .into_iter()
        .find(|fixture| fixture.seed == "12345")
        .expect("seed 12345 fixture");
    let sample_set = fixture
        .sample_sets
        .get("constantPlains")
        .expect("constantPlains sample set");
    let settings = create_noise_settings(&fixture);
    let mut random = WorldgenRandom::new(12_345);
    let blended_noise = BlendedNoise::new(&mut random);
    random.consume_count(2620);
    let depth_noise = PerlinNoise::from_octaves(&mut random, &DEPTH_NOISE_OCTAVES);
    let island_noise = SimplexNoise::new(&mut WorldgenRandom::new(12_345));
    let sampler = NoiseSampler::new(
        RepeatingPatternBiomeSource::new(&sample_set.biome_pattern),
        fixture.cell_width,
        fixture.cell_height,
        fixture.cell_count_y,
        settings.clone(),
        blended_noise,
        Some(island_noise),
        depth_noise,
        NoiseModifier::Passthrough,
    );

    assert_panic_message(
        || {
            let mut column = vec![0.0; fixture.column_value_count];
            sampler.fill_noise_column(
                &mut column,
                0,
                0,
                &settings,
                fixture.biome_y,
                fixture.min_cell_y,
                fixture.cell_count_y,
            );
        },
        "NoiseSampler island noise override is out of scope for the 1.17.1 overworld target",
    );
}
