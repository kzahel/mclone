use crate::block::{RawBlockId, STONE, WATER};

const BITS_FOR_Y: i32 = 12;
const Y_SIZE: i32 = (1 << BITS_FOR_Y) - 32;
const MAX_Y: i32 = (Y_SIZE >> 1) - 1;
const INT_MIN: i32 = i32::MIN;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoiseSamplingSettings {
    xz_scale: f64,
    y_scale: f64,
    xz_factor: f64,
    y_factor: f64,
}

impl NoiseSamplingSettings {
    pub fn new(xz_scale: f64, y_scale: f64, xz_factor: f64, y_factor: f64) -> Self {
        Self {
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
        }
    }

    pub fn xz_scale(&self) -> f64 {
        self.xz_scale
    }

    pub fn y_scale(&self) -> f64 {
        self.y_scale
    }

    pub fn xz_factor(&self) -> f64 {
        self.xz_factor
    }

    pub fn y_factor(&self) -> f64 {
        self.y_factor
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoiseSlideSettings {
    target: i32,
    size: i32,
    offset: i32,
}

impl NoiseSlideSettings {
    pub fn new(target: i32, size: i32, offset: i32) -> Self {
        Self {
            target,
            size,
            offset,
        }
    }

    pub fn target(&self) -> i32 {
        self.target
    }

    pub fn size(&self) -> i32 {
        self.size
    }

    pub fn offset(&self) -> i32 {
        self.offset
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoiseSettings {
    min_y: i32,
    height: i32,
    noise_sampling_settings: NoiseSamplingSettings,
    top_slide_settings: NoiseSlideSettings,
    bottom_slide_settings: NoiseSlideSettings,
    noise_size_horizontal: i32,
    noise_size_vertical: i32,
    density_factor: f64,
    density_offset: f64,
    use_simplex_surface_noise: bool,
    random_density_offset: bool,
    island_noise_override: bool,
    is_amplified: bool,
}

impl NoiseSettings {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        min_y: i32,
        height: i32,
        noise_sampling_settings: NoiseSamplingSettings,
        top_slide_settings: NoiseSlideSettings,
        bottom_slide_settings: NoiseSlideSettings,
        noise_size_horizontal: i32,
        noise_size_vertical: i32,
        density_factor: f64,
        density_offset: f64,
        use_simplex_surface_noise: bool,
        random_density_offset: bool,
        island_noise_override: bool,
        is_amplified: bool,
    ) -> Self {
        let settings = Self {
            min_y,
            height,
            noise_sampling_settings,
            top_slide_settings,
            bottom_slide_settings,
            noise_size_horizontal,
            noise_size_vertical,
            density_factor,
            density_offset,
            use_simplex_surface_noise,
            random_density_offset,
            island_noise_override,
            is_amplified,
        };
        settings.guard_y();
        settings
    }

    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn noise_sampling_settings(&self) -> &NoiseSamplingSettings {
        &self.noise_sampling_settings
    }

    pub fn top_slide_settings(&self) -> &NoiseSlideSettings {
        &self.top_slide_settings
    }

    pub fn bottom_slide_settings(&self) -> &NoiseSlideSettings {
        &self.bottom_slide_settings
    }

    pub fn noise_size_horizontal(&self) -> i32 {
        self.noise_size_horizontal
    }

    pub fn noise_size_vertical(&self) -> i32 {
        self.noise_size_vertical
    }

    pub fn density_factor(&self) -> f64 {
        self.density_factor
    }

    pub fn density_offset(&self) -> f64 {
        self.density_offset
    }

    pub fn use_simplex_surface_noise(&self) -> bool {
        self.use_simplex_surface_noise
    }

    pub fn random_density_offset(&self) -> bool {
        self.random_density_offset
    }

    pub fn island_noise_override(&self) -> bool {
        self.island_noise_override
    }

    pub fn is_amplified(&self) -> bool {
        self.is_amplified
    }

    fn guard_y(&self) {
        if self.min_y() + self.height() > MAX_Y + 1 {
            panic!("min_y + height cannot be higher than: {}", MAX_Y + 1);
        }

        if self.height() % 16 != 0 {
            panic!("height has to be a multiple of 16");
        }

        if self.min_y() % 16 != 0 {
            panic!("min_y has to be a multiple of 16");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseModifier {
    Passthrough,
}

impl NoiseModifier {
    pub fn modify_noise(&self, noise: f64, _y: i32, _z: i32, _x: i32) -> f64 {
        match self {
            Self::Passthrough => noise,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoiseGeneratorSettings {
    noise_settings: NoiseSettings,
    default_block: RawBlockId,
    default_fluid: RawBlockId,
    bedrock_roof_position: i32,
    bedrock_floor_position: i32,
    sea_level: i32,
    min_surface_level: i32,
    disable_mob_generation: bool,
    aquifers_enabled: bool,
    noise_caves_enabled: bool,
    deepslate_enabled: bool,
    ore_veins_enabled: bool,
    noodle_caves_enabled: bool,
}

impl NoiseGeneratorSettings {
    pub fn overworld() -> Self {
        Self::overworld_with_amplified(false)
    }

    pub fn overworld_with_amplified(is_amplified: bool) -> Self {
        Self {
            noise_settings: NoiseSettings::create(
                0,
                256,
                NoiseSamplingSettings::new(0.9999999814507745, 0.9999999814507745, 80.0, 160.0),
                NoiseSlideSettings::new(-10, 3, 0),
                NoiseSlideSettings::new(15, 3, 0),
                1,
                2,
                1.0,
                -0.46875,
                true,
                true,
                false,
                is_amplified,
            ),
            default_block: STONE,
            default_fluid: WATER,
            bedrock_roof_position: INT_MIN,
            bedrock_floor_position: 0,
            sea_level: 63,
            min_surface_level: 0,
            disable_mob_generation: false,
            aquifers_enabled: false,
            noise_caves_enabled: false,
            deepslate_enabled: false,
            ore_veins_enabled: false,
            noodle_caves_enabled: false,
        }
    }

    pub fn noise_settings(&self) -> &NoiseSettings {
        &self.noise_settings
    }

    pub fn default_block(&self) -> RawBlockId {
        self.default_block
    }

    pub fn default_fluid(&self) -> RawBlockId {
        self.default_fluid
    }

    pub fn bedrock_roof_position(&self) -> i32 {
        self.bedrock_roof_position
    }

    pub fn bedrock_floor_position(&self) -> i32 {
        self.bedrock_floor_position
    }

    pub fn sea_level(&self) -> i32 {
        self.sea_level
    }

    pub fn min_surface_level(&self) -> i32 {
        self.min_surface_level
    }

    pub fn disable_mob_generation(&self) -> bool {
        self.disable_mob_generation
    }

    pub fn is_aquifers_enabled(&self) -> bool {
        self.aquifers_enabled
    }

    pub fn is_noise_caves_enabled(&self) -> bool {
        self.noise_caves_enabled
    }

    pub fn is_deepslate_enabled(&self) -> bool {
        self.deepslate_enabled
    }

    pub fn is_ore_veins_enabled(&self) -> bool {
        self.ore_veins_enabled
    }

    pub fn is_noodle_caves_enabled(&self) -> bool {
        self.noodle_caves_enabled
    }
}
