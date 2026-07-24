use mclone_worldgen::{
    levelgen::McloneOverworldSamplingTopology,
    terrain_preview::{
        TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS, TERRAIN_PREVIEW_MAX_SAMPLE_SPACING,
        TERRAIN_PREVIEW_MIN_SAMPLE_SPACING, TerrainPreviewRequest,
    },
};

pub const TERRAIN_VIEWPORT_MIN_BLOCKS_ACROSS: u32 = 64;
pub const TERRAIN_VIEWPORT_MAX_BLOCKS_ACROSS: u32 = 131_072;
pub const TERRAIN_VIEWPORT_MAX_VISIBLE_TILES_PER_AXIS: u32 = 8;
pub const TERRAIN_VIEWPORT_MAX_DIAGNOSTIC_TILES_PER_AXIS: u32 = 16;
pub const TERRAIN_VIEWPORT_PRELOAD_MARGIN_TILES: i32 = 1;
pub const TERRAIN_VIEWPORT_AUTO_PIXELS_PER_CELL: f64 = 2.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerrainViewportDetail {
    Auto,
    Manual(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainViewportRequest {
    pub seed: i64,
    pub center_x: i32,
    pub center_z: i32,
    pub blocks_across: u32,
    pub panel_width_css: u32,
    pub panel_height_css: u32,
    pub detail: TerrainViewportDetail,
    pub max_visible_tiles_per_axis: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TerrainViewportTileId {
    pub seed: i64,
    pub tile_x: i32,
    pub tile_z: i32,
    pub sample_spacing: u32,
}

impl TerrainViewportTileId {
    pub fn footprint_blocks(self) -> u32 {
        TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS * self.sample_spacing
    }

    pub fn min_x(self) -> i32 {
        tile_origin(self.tile_x, self.footprint_blocks())
            .expect("validated terrain viewport tile X")
    }

    pub fn min_z(self) -> i32 {
        tile_origin(self.tile_z, self.footprint_blocks())
            .expect("validated terrain viewport tile Z")
    }

    pub fn preview_request(self) -> TerrainPreviewRequest {
        let half = i32::try_from(self.footprint_blocks() / 2)
            .expect("terrain viewport tile half footprint fits i32");
        TerrainPreviewRequest {
            seed: self.seed,
            center_x: self
                .min_x()
                .checked_add(half)
                .expect("validated terrain viewport tile center X"),
            center_z: self
                .min_z()
                .checked_add(half)
                .expect("validated terrain viewport tile center Z"),
            sample_spacing: self.sample_spacing,
            cells_per_axis: TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS,
            topology: McloneOverworldSamplingTopology::Unbounded,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainViewportLevel {
    pub sample_spacing: u32,
    pub visible_tiles: Vec<TerrainViewportTileId>,
    pub preload_tiles: Vec<TerrainViewportTileId>,
}

impl TerrainViewportLevel {
    pub fn visible_tile_count(&self) -> usize {
        self.visible_tiles.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerrainViewportPlan {
    pub request: TerrainViewportRequest,
    pub view_width_blocks: u32,
    pub view_height_blocks: u32,
    pub requested_spacing: u32,
    pub effective_spacing: u32,
    pub budget_limited: bool,
    pub levels: Vec<TerrainViewportLevel>,
}

impl TerrainViewportPlan {
    pub fn target_level(&self) -> &TerrainViewportLevel {
        self.levels
            .last()
            .expect("terrain viewport plans always contain one level")
    }

    pub fn coarsest_level(&self) -> &TerrainViewportLevel {
        self.levels
            .first()
            .expect("terrain viewport plans always contain one level")
    }
}

pub fn plan_terrain_viewport(
    request: TerrainViewportRequest,
) -> Result<TerrainViewportPlan, String> {
    if !(TERRAIN_VIEWPORT_MIN_BLOCKS_ACROSS..=TERRAIN_VIEWPORT_MAX_BLOCKS_ACROSS)
        .contains(&request.blocks_across)
    {
        return Err(format!(
            "terrain viewport width must be {} through {} blocks, got {}",
            TERRAIN_VIEWPORT_MIN_BLOCKS_ACROSS,
            TERRAIN_VIEWPORT_MAX_BLOCKS_ACROSS,
            request.blocks_across
        ));
    }
    if request.panel_width_css == 0 || request.panel_height_css == 0 {
        return Err("terrain viewport panel dimensions must be non-zero".to_owned());
    }
    if !(1..=TERRAIN_VIEWPORT_MAX_DIAGNOSTIC_TILES_PER_AXIS)
        .contains(&request.max_visible_tiles_per_axis)
    {
        return Err(format!(
            "terrain viewport tile-axis budget must be 1 through {}, got {}",
            TERRAIN_VIEWPORT_MAX_DIAGNOSTIC_TILES_PER_AXIS, request.max_visible_tiles_per_axis
        ));
    }

    let view_height_blocks = u32::try_from(
        u64::from(request.blocks_across)
            .checked_mul(u64::from(request.panel_height_css))
            .ok_or("terrain viewport height multiplication overflow")?
            .div_ceil(u64::from(request.panel_width_css)),
    )
    .map_err(|_| "terrain viewport height exceeds u32")?
    .max(1);
    validate_view_bounds(
        request.center_x,
        request.center_z,
        request.blocks_across,
        view_height_blocks,
    )?;

    let requested_spacing = match request.detail {
        TerrainViewportDetail::Auto => auto_spacing(request, view_height_blocks),
        TerrainViewportDetail::Manual(spacing) => {
            validate_spacing(spacing)?;
            spacing
        }
    };
    let mut effective_spacing = requested_spacing;
    loop {
        let (tiles_x, tiles_z) = level_tile_axis_counts(
            request.center_x,
            request.center_z,
            request.blocks_across,
            view_height_blocks,
            effective_spacing,
        )?;
        if tiles_x.max(tiles_z) <= request.max_visible_tiles_per_axis {
            break;
        }
        if effective_spacing == TERRAIN_PREVIEW_MAX_SAMPLE_SPACING {
            return Err(format!(
                "terrain viewport cannot fit within the {}-tile axis budget at spacing {}",
                request.max_visible_tiles_per_axis, effective_spacing
            ));
        }
        effective_spacing *= 2;
    }

    let mut coarsest_spacing = effective_spacing;
    loop {
        let (tiles_x, tiles_z) = level_tile_axis_counts(
            request.center_x,
            request.center_z,
            request.blocks_across,
            view_height_blocks,
            coarsest_spacing,
        )?;
        if tiles_x.max(tiles_z) <= 2 || coarsest_spacing == TERRAIN_PREVIEW_MAX_SAMPLE_SPACING {
            break;
        }
        coarsest_spacing *= 2;
    }

    let mut levels = Vec::new();
    let mut spacing = coarsest_spacing;
    loop {
        levels.push(plan_level(
            request.seed,
            request.center_x,
            request.center_z,
            request.blocks_across,
            view_height_blocks,
            spacing,
        )?);
        if spacing == effective_spacing {
            break;
        }
        spacing /= 2;
    }

    Ok(TerrainViewportPlan {
        request,
        view_width_blocks: request.blocks_across,
        view_height_blocks,
        requested_spacing,
        effective_spacing,
        budget_limited: effective_spacing != requested_spacing,
        levels,
    })
}

fn auto_spacing(request: TerrainViewportRequest, view_height_blocks: u32) -> u32 {
    let blocks_per_css_pixel = (f64::from(request.blocks_across)
        / f64::from(request.panel_width_css))
    .max(f64::from(view_height_blocks) / f64::from(request.panel_height_css));
    let target = blocks_per_css_pixel * TERRAIN_VIEWPORT_AUTO_PIXELS_PER_CELL;
    let mut spacing = TERRAIN_PREVIEW_MIN_SAMPLE_SPACING;
    while f64::from(spacing) < target && spacing < TERRAIN_PREVIEW_MAX_SAMPLE_SPACING {
        spacing *= 2;
    }
    spacing
}

fn validate_spacing(spacing: u32) -> Result<(), String> {
    if spacing.is_power_of_two()
        && (TERRAIN_PREVIEW_MIN_SAMPLE_SPACING..=TERRAIN_PREVIEW_MAX_SAMPLE_SPACING)
            .contains(&spacing)
    {
        return Ok(());
    }
    Err(format!(
        "terrain viewport spacing must be a power of two from {} through {}, got {}",
        TERRAIN_PREVIEW_MIN_SAMPLE_SPACING, TERRAIN_PREVIEW_MAX_SAMPLE_SPACING, spacing
    ))
}

fn validate_view_bounds(
    center_x: i32,
    center_z: i32,
    width_blocks: u32,
    height_blocks: u32,
) -> Result<(), String> {
    for (axis, center, extent) in [
        ("X", center_x, width_blocks),
        ("Z", center_z, height_blocks),
    ] {
        let center_twice = i64::from(center) * 2;
        let min_twice = center_twice - i64::from(extent);
        let max_twice = center_twice + i64::from(extent);
        if min_twice < i64::from(i32::MIN) * 2 || max_twice > i64::from(i32::MAX) * 2 {
            return Err(format!(
                "terrain viewport {axis} extent exceeds signed 32-bit world coordinates"
            ));
        }
    }
    Ok(())
}

fn plan_level(
    seed: i64,
    center_x: i32,
    center_z: i32,
    width_blocks: u32,
    height_blocks: u32,
    sample_spacing: u32,
) -> Result<TerrainViewportLevel, String> {
    let footprint = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS
        .checked_mul(sample_spacing)
        .ok_or("terrain viewport tile footprint overflow")?;
    let (min_tile_x, max_tile_x) = tile_range(center_x, width_blocks, footprint)?;
    let (min_tile_z, max_tile_z) = tile_range(center_z, height_blocks, footprint)?;
    let center_tile_x = center_x.div_euclid(
        i32::try_from(footprint).map_err(|_| "terrain viewport tile footprint exceeds i32")?,
    );
    let center_tile_z = center_z.div_euclid(
        i32::try_from(footprint).map_err(|_| "terrain viewport tile footprint exceeds i32")?,
    );

    let visible_tiles = ordered_tiles(
        seed,
        min_tile_x,
        max_tile_x,
        min_tile_z,
        max_tile_z,
        sample_spacing,
        center_tile_x,
        center_tile_z,
    )?;
    let preload_tiles = ordered_tiles(
        seed,
        min_tile_x.saturating_sub(TERRAIN_VIEWPORT_PRELOAD_MARGIN_TILES),
        max_tile_x.saturating_add(TERRAIN_VIEWPORT_PRELOAD_MARGIN_TILES),
        min_tile_z.saturating_sub(TERRAIN_VIEWPORT_PRELOAD_MARGIN_TILES),
        max_tile_z.saturating_add(TERRAIN_VIEWPORT_PRELOAD_MARGIN_TILES),
        sample_spacing,
        center_tile_x,
        center_tile_z,
    )?;

    Ok(TerrainViewportLevel {
        sample_spacing,
        visible_tiles,
        preload_tiles,
    })
}

fn level_tile_axis_counts(
    center_x: i32,
    center_z: i32,
    width_blocks: u32,
    height_blocks: u32,
    sample_spacing: u32,
) -> Result<(u32, u32), String> {
    let footprint = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS
        .checked_mul(sample_spacing)
        .ok_or("terrain viewport tile footprint overflow")?;
    let (min_x, max_x) = tile_range(center_x, width_blocks, footprint)?;
    let (min_z, max_z) = tile_range(center_z, height_blocks, footprint)?;
    Ok((
        u32::try_from(i64::from(max_x) - i64::from(min_x) + 1)
            .map_err(|_| "terrain viewport X tile count overflow")?,
        u32::try_from(i64::from(max_z) - i64::from(min_z) + 1)
            .map_err(|_| "terrain viewport Z tile count overflow")?,
    ))
}

fn tile_range(center: i32, extent: u32, footprint: u32) -> Result<(i32, i32), String> {
    let center_twice = i64::from(center) * 2;
    let denominator = i64::from(footprint) * 2;
    let min_tile = (center_twice - i64::from(extent)).div_euclid(denominator);
    let max_tile = -((-(center_twice + i64::from(extent))).div_euclid(denominator)) - 1;
    Ok((
        i32::try_from(min_tile).map_err(|_| "terrain viewport minimum tile exceeds i32")?,
        i32::try_from(max_tile).map_err(|_| "terrain viewport maximum tile exceeds i32")?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn ordered_tiles(
    seed: i64,
    min_tile_x: i32,
    max_tile_x: i32,
    min_tile_z: i32,
    max_tile_z: i32,
    sample_spacing: u32,
    center_tile_x: i32,
    center_tile_z: i32,
) -> Result<Vec<TerrainViewportTileId>, String> {
    let footprint = TERRAIN_PREVIEW_DEFAULT_CELLS_PER_AXIS
        .checked_mul(sample_spacing)
        .ok_or("terrain viewport tile footprint overflow")?;
    let mut tiles = Vec::new();
    for tile_z in min_tile_z..=max_tile_z {
        for tile_x in min_tile_x..=max_tile_x {
            if tile_origin(tile_x, footprint).is_none() || tile_origin(tile_z, footprint).is_none()
            {
                continue;
            }
            tiles.push(TerrainViewportTileId {
                seed,
                tile_x,
                tile_z,
                sample_spacing,
            });
        }
    }
    tiles.sort_by_key(|tile| {
        (
            i64::from(tile.tile_x).abs_diff(i64::from(center_tile_x))
                + i64::from(tile.tile_z).abs_diff(i64::from(center_tile_z)),
            tile.tile_z,
            tile.tile_x,
        )
    });
    Ok(tiles)
}

fn tile_origin(tile: i32, footprint: u32) -> Option<i32> {
    let origin = i64::from(tile).checked_mul(i64::from(footprint))?;
    let end = origin.checked_add(i64::from(footprint))?;
    if origin < i64::from(i32::MIN) || end > i64::from(i32::MAX) {
        return None;
    }
    i32::try_from(origin).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(detail: TerrainViewportDetail) -> TerrainViewportRequest {
        TerrainViewportRequest {
            seed: -98_765,
            center_x: 0,
            center_z: 0,
            blocks_across: 2_048,
            panel_width_css: 800,
            panel_height_css: 600,
            detail,
            max_visible_tiles_per_axis: TERRAIN_VIEWPORT_MAX_VISIBLE_TILES_PER_AXIS,
        }
    }

    #[test]
    fn auto_detail_tracks_css_pixel_density() {
        let plan = plan_terrain_viewport(request(TerrainViewportDetail::Auto)).unwrap();
        assert_eq!(plan.view_width_blocks, 2_048);
        assert_eq!(plan.view_height_blocks, 1_536);
        assert_eq!(plan.requested_spacing, 8);
        assert_eq!(plan.effective_spacing, 8);
        assert!(!plan.budget_limited);
        assert_eq!(plan.coarsest_level().sample_spacing, 16);
        assert_eq!(plan.target_level().sample_spacing, 8);
    }

    #[test]
    fn manual_detail_is_budgeted_and_refines_in_nested_levels() {
        let plan = plan_terrain_viewport(request(TerrainViewportDetail::Manual(1))).unwrap();
        assert_eq!(plan.requested_spacing, 1);
        assert_eq!(plan.effective_spacing, 4);
        assert!(plan.budget_limited);
        assert_eq!(
            plan.levels
                .iter()
                .map(|level| level.sample_spacing)
                .collect::<Vec<_>>(),
            vec![16, 8, 4]
        );
        assert!(
            plan.target_level().visible_tile_count()
                <= usize::try_from(TERRAIN_VIEWPORT_MAX_VISIBLE_TILES_PER_AXIS.pow(2)).unwrap()
        );
    }

    #[test]
    fn diagnostic_axis_budget_admits_more_cold_stress_work() {
        let normal = plan_terrain_viewport(request(TerrainViewportDetail::Manual(1))).unwrap();
        let mut stress = request(TerrainViewportDetail::Manual(1));
        stress.max_visible_tiles_per_axis = TERRAIN_VIEWPORT_MAX_DIAGNOSTIC_TILES_PER_AXIS;
        let stress = plan_terrain_viewport(stress).unwrap();
        assert!(stress.effective_spacing < normal.effective_spacing);
        assert!(
            stress.target_level().visible_tile_count() > normal.target_level().visible_tile_count()
        );
    }

    #[test]
    fn aligned_tile_identity_survives_small_pans() {
        let first = plan_terrain_viewport(request(TerrainViewportDetail::Manual(8))).unwrap();
        let mut moved = request(TerrainViewportDetail::Manual(8));
        moved.center_x = 40;
        moved.center_z = -20;
        let second = plan_terrain_viewport(moved).unwrap();
        assert!(
            first
                .target_level()
                .visible_tiles
                .iter()
                .any(|tile| second.target_level().visible_tiles.contains(tile))
        );
    }

    #[test]
    fn negative_tiles_produce_exact_preview_origins() {
        let tile = TerrainViewportTileId {
            seed: 7,
            tile_x: -2,
            tile_z: -1,
            sample_spacing: 16,
        };
        assert_eq!(tile.min_x(), -2_048);
        assert_eq!(tile.min_z(), -1_024);
        let request = tile.preview_request().validate().unwrap();
        assert_eq!(request.min_x(), tile.min_x());
        assert_eq!(request.min_z(), tile.min_z());
    }

    #[test]
    fn rejects_invalid_manual_detail_and_coordinate_overflow() {
        assert!(plan_terrain_viewport(request(TerrainViewportDetail::Manual(3))).is_err());
        let mut edge = request(TerrainViewportDetail::Auto);
        edge.center_x = i32::MAX;
        assert!(plan_terrain_viewport(edge).is_err());
    }
}
