use serde::Serialize;

pub const DEFAULT_TERRAIN_ANALYSIS_LAGS: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
pub const DEFAULT_TERRAIN_PLANE_RADII: [usize; 5] = [2, 4, 8, 16, 32];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerrainHeightRaster {
    width: usize,
    depth: usize,
    heights: Vec<i32>,
    included: Vec<bool>,
}

impl TerrainHeightRaster {
    pub fn new(
        width: usize,
        depth: usize,
        heights: Vec<i32>,
        included: Vec<bool>,
    ) -> Result<Self, String> {
        let expected = width
            .checked_mul(depth)
            .ok_or("terrain height raster area overflow")?;
        if width == 0 || depth == 0 {
            return Err("terrain height raster dimensions must be nonzero".to_owned());
        }
        if heights.len() != expected || included.len() != expected {
            return Err(format!(
                "terrain height raster expected {expected} cells, got {} heights and {} mask cells",
                heights.len(),
                included.len()
            ));
        }
        Ok(Self {
            width,
            depth,
            heights,
            included,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn heights(&self) -> &[i32] {
        &self.heights
    }

    pub fn included(&self) -> &[bool] {
        &self.included
    }

    fn height(&self, x: usize, z: usize) -> f64 {
        f64::from(self.heights[z * self.width + x])
    }

    fn is_included(&self, x: usize, z: usize) -> bool {
        self.included[z * self.width + x]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainValueDistribution {
    pub sample_count: usize,
    pub mean: f64,
    pub rms: f64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub max: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainHeightDistribution {
    pub sample_count: usize,
    pub min: i32,
    pub max: i32,
    pub mean: f64,
    pub p05: i32,
    pub p50: i32,
    pub p95: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainLagCharacteristics {
    pub lag_blocks: usize,
    pub pair_count: usize,
    pub mean_abs_height_delta: f64,
    pub rms_height_delta: f64,
    pub p90_abs_height_delta: f64,
    pub mean_abs_grade: f64,
    pub rms_grade: f64,
    pub equal_height_share: f64,
    pub x_rms_height_delta: f64,
    pub z_rms_height_delta: f64,
    pub axis_anisotropy: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainPlaneFitCharacteristics {
    pub radius_blocks: usize,
    pub window_width_blocks: usize,
    pub window_count: usize,
    pub mean_rmse: f64,
    pub pooled_rmse: f64,
    pub p50_rmse: f64,
    pub p90_rmse: f64,
    pub mean_fitted_grade: f64,
    pub p90_fitted_grade: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainCharacteristics {
    pub width: usize,
    pub depth: usize,
    pub included_columns: usize,
    pub included_share: f64,
    pub height: TerrainHeightDistribution,
    pub lag_curve: Vec<TerrainLagCharacteristics>,
    pub curvature: TerrainValueDistribution,
    pub plane_fit_curve: Vec<TerrainPlaneFitCharacteristics>,
    pub roughness_exponent: Option<f64>,
    pub fine_detail_share_r4_of_r32: Option<f64>,
    pub fingerprint: u64,
}

pub fn analyze_terrain_height_raster(
    raster: &TerrainHeightRaster,
    lags: &[usize],
    plane_radii: &[usize],
) -> Result<TerrainCharacteristics, String> {
    validate_scales("terrain lag", lags)?;
    validate_scales("terrain plane radius", plane_radii)?;
    let included_heights = raster
        .heights
        .iter()
        .copied()
        .zip(raster.included.iter().copied())
        .filter_map(|(height, included)| included.then_some(height))
        .collect::<Vec<_>>();
    if included_heights.is_empty() {
        return Err("terrain height raster mask includes no columns".to_owned());
    }

    let height = height_distribution(included_heights.clone());
    let lag_curve = lags
        .iter()
        .copied()
        .filter(|lag| *lag < raster.width.max(raster.depth))
        .map(|lag| lag_characteristics(raster, lag))
        .filter(|band| band.pair_count > 0)
        .collect::<Vec<_>>();
    let curvature = curvature_characteristics(raster);
    let integrals = TerrainIntegrals::new(raster);
    let plane_fit_curve = plane_radii
        .iter()
        .copied()
        .filter(|radius| radius * 2 + 1 <= raster.width.min(raster.depth))
        .map(|radius| plane_fit_characteristics(raster, &integrals, radius))
        .filter(|scale| scale.window_count > 0)
        .collect::<Vec<_>>();
    let roughness_exponent = roughness_exponent(&lag_curve);
    let fine_detail_share_r4_of_r32 = detail_share(&plane_fit_curve, 4, 32);
    let included_columns = included_heights.len();

    Ok(TerrainCharacteristics {
        width: raster.width,
        depth: raster.depth,
        included_columns,
        included_share: included_columns as f64 / raster.heights.len() as f64,
        height,
        lag_curve,
        curvature,
        plane_fit_curve,
        roughness_exponent,
        fine_detail_share_r4_of_r32,
        fingerprint: raster_fingerprint(raster),
    })
}

fn validate_scales(label: &str, scales: &[usize]) -> Result<(), String> {
    if scales.is_empty() {
        return Err(format!("{label} list must not be empty"));
    }
    if scales.contains(&0) {
        return Err(format!("{label} values must be nonzero"));
    }
    if scales.windows(2).any(|window| window[0] >= window[1]) {
        return Err(format!("{label} values must be strictly increasing"));
    }
    Ok(())
}

fn height_distribution(mut values: Vec<i32>) -> TerrainHeightDistribution {
    values.sort_unstable();
    let mean = values.iter().map(|value| f64::from(*value)).sum::<f64>() / values.len() as f64;
    TerrainHeightDistribution {
        sample_count: values.len(),
        min: values[0],
        max: values[values.len() - 1],
        mean,
        p05: percentile_i32(&values, 5),
        p50: percentile_i32(&values, 50),
        p95: percentile_i32(&values, 95),
    }
}

fn value_distribution(mut values: Vec<f64>) -> TerrainValueDistribution {
    if values.is_empty() {
        return TerrainValueDistribution {
            sample_count: 0,
            mean: 0.0,
            rms: 0.0,
            p50: 0.0,
            p90: 0.0,
            p95: 0.0,
            max: 0.0,
        };
    }
    values.sort_by(f64::total_cmp);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let rms = (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt();
    TerrainValueDistribution {
        sample_count: values.len(),
        mean,
        rms,
        p50: percentile_f64(&values, 50),
        p90: percentile_f64(&values, 90),
        p95: percentile_f64(&values, 95),
        max: values[values.len() - 1],
    }
}

fn lag_characteristics(raster: &TerrainHeightRaster, lag: usize) -> TerrainLagCharacteristics {
    let mut deltas = Vec::new();
    let mut x_sum_squares = 0.0;
    let mut x_count = 0usize;
    let mut z_sum_squares = 0.0;
    let mut z_count = 0usize;

    if lag < raster.width {
        for z in 0..raster.depth {
            for x in 0..raster.width - lag {
                if !raster.is_included(x, z) || !raster.is_included(x + lag, z) {
                    continue;
                }
                let delta = (raster.height(x + lag, z) - raster.height(x, z)).abs();
                deltas.push(delta);
                x_sum_squares += delta * delta;
                x_count += 1;
            }
        }
    }
    if lag < raster.depth {
        for z in 0..raster.depth - lag {
            for x in 0..raster.width {
                if !raster.is_included(x, z) || !raster.is_included(x, z + lag) {
                    continue;
                }
                let delta = (raster.height(x, z + lag) - raster.height(x, z)).abs();
                deltas.push(delta);
                z_sum_squares += delta * delta;
                z_count += 1;
            }
        }
    }

    let distribution = value_distribution(deltas.clone());
    let x_rms = rms_from_sum(x_sum_squares, x_count);
    let z_rms = rms_from_sum(z_sum_squares, z_count);
    let min_axis = x_rms.min(z_rms);
    TerrainLagCharacteristics {
        lag_blocks: lag,
        pair_count: deltas.len(),
        mean_abs_height_delta: distribution.mean,
        rms_height_delta: distribution.rms,
        p90_abs_height_delta: distribution.p90,
        mean_abs_grade: distribution.mean / lag as f64,
        rms_grade: distribution.rms / lag as f64,
        equal_height_share: deltas.iter().filter(|delta| **delta == 0.0).count() as f64
            / deltas.len().max(1) as f64,
        x_rms_height_delta: x_rms,
        z_rms_height_delta: z_rms,
        axis_anisotropy: if min_axis > 0.0 {
            x_rms.max(z_rms) / min_axis
        } else {
            1.0
        },
    }
}

fn curvature_characteristics(raster: &TerrainHeightRaster) -> TerrainValueDistribution {
    let mut values = Vec::new();
    if raster.width < 3 || raster.depth < 3 {
        return value_distribution(values);
    }
    for z in 1..raster.depth - 1 {
        for x in 1..raster.width - 1 {
            if !raster.is_included(x, z)
                || !raster.is_included(x - 1, z)
                || !raster.is_included(x + 1, z)
                || !raster.is_included(x, z - 1)
                || !raster.is_included(x, z + 1)
            {
                continue;
            }
            let laplacian = raster.height(x - 1, z)
                + raster.height(x + 1, z)
                + raster.height(x, z - 1)
                + raster.height(x, z + 1)
                - 4.0 * raster.height(x, z);
            values.push(laplacian.abs());
        }
    }
    value_distribution(values)
}

fn plane_fit_characteristics(
    raster: &TerrainHeightRaster,
    integrals: &TerrainIntegrals,
    radius: usize,
) -> TerrainPlaneFitCharacteristics {
    let side = radius * 2 + 1;
    let sample_count = side * side;
    let centered_square_sum = (radius * (radius + 1) * (radius * 2 + 1) / 3) as f64;
    let gradient_denom = side as f64 * centered_square_sum;
    let mut rmse_values = Vec::new();
    let mut grade_values = Vec::new();

    for z in radius..raster.depth - radius {
        for x in radius..raster.width - radius {
            let min_x = x - radius;
            let min_z = z - radius;
            let max_x = x + radius + 1;
            let max_z = z + radius + 1;
            if integrals.mask.rect(min_x, min_z, max_x, max_z) as usize != sample_count {
                continue;
            }
            let sum_h = integrals.height.rect(min_x, min_z, max_x, max_z);
            let sum_h2 = integrals.height_squared.rect(min_x, min_z, max_x, max_z);
            let sum_xh = integrals.x_height.rect(min_x, min_z, max_x, max_z);
            let sum_zh = integrals.z_height.rect(min_x, min_z, max_x, max_z);
            let centered_xh = sum_xh - x as f64 * sum_h;
            let centered_zh = sum_zh - z as f64 * sum_h;
            let intercept_energy = sum_h * sum_h / sample_count as f64;
            let x_energy = centered_xh * centered_xh / gradient_denom;
            let z_energy = centered_zh * centered_zh / gradient_denom;
            let residual_sum_squares = (sum_h2 - intercept_energy - x_energy - z_energy).max(0.0);
            rmse_values.push((residual_sum_squares / sample_count as f64).sqrt());
            let grade_x = centered_xh / gradient_denom;
            let grade_z = centered_zh / gradient_denom;
            grade_values.push(grade_x.hypot(grade_z));
        }
    }

    let rmse = value_distribution(rmse_values);
    let grade = value_distribution(grade_values);
    TerrainPlaneFitCharacteristics {
        radius_blocks: radius,
        window_width_blocks: side,
        window_count: rmse.sample_count,
        mean_rmse: rmse.mean,
        pooled_rmse: rmse.rms,
        p50_rmse: rmse.p50,
        p90_rmse: rmse.p90,
        mean_fitted_grade: grade.mean,
        p90_fitted_grade: grade.p90,
    }
}

fn roughness_exponent(curve: &[TerrainLagCharacteristics]) -> Option<f64> {
    let points = curve
        .iter()
        .filter(|band| band.lag_blocks <= 32 && band.rms_height_delta > 0.0)
        .map(|band| ((band.lag_blocks as f64).ln(), band.rms_height_delta.ln()))
        .collect::<Vec<_>>();
    if points.len() < 2 {
        return None;
    }
    let mean_x = points.iter().map(|point| point.0).sum::<f64>() / points.len() as f64;
    let mean_y = points.iter().map(|point| point.1).sum::<f64>() / points.len() as f64;
    let numerator = points
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>();
    let denominator = points
        .iter()
        .map(|(x, _)| (x - mean_x) * (x - mean_x))
        .sum::<f64>();
    (denominator > 0.0).then_some(numerator / denominator)
}

fn detail_share(
    curve: &[TerrainPlaneFitCharacteristics],
    fine_radius: usize,
    broad_radius: usize,
) -> Option<f64> {
    let fine = curve
        .iter()
        .find(|scale| scale.radius_blocks == fine_radius)?
        .pooled_rmse;
    let broad = curve
        .iter()
        .find(|scale| scale.radius_blocks == broad_radius)?
        .pooled_rmse;
    (broad > 0.0).then_some((fine * fine / (broad * broad)).clamp(0.0, 1.0))
}

fn raster_fingerprint(raster: &TerrainHeightRaster) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for (height, included) in raster.heights.iter().zip(raster.included.iter()) {
        for byte in height
            .to_le_bytes()
            .into_iter()
            .chain([u8::from(*included)])
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn rms_from_sum(sum_squares: f64, count: usize) -> f64 {
    if count == 0 {
        0.0
    } else {
        (sum_squares / count as f64).sqrt()
    }
}

fn percentile_i32(sorted: &[i32], percent: usize) -> i32 {
    sorted[(sorted.len() - 1) * percent / 100]
}

fn percentile_f64(sorted: &[f64], percent: usize) -> f64 {
    sorted[(sorted.len() - 1) * percent / 100]
}

struct TerrainIntegrals {
    mask: IntegralU64,
    height: IntegralF64,
    height_squared: IntegralF64,
    x_height: IntegralF64,
    z_height: IntegralF64,
}

impl TerrainIntegrals {
    fn new(raster: &TerrainHeightRaster) -> Self {
        let mut mask = Vec::with_capacity(raster.heights.len());
        let mut height = Vec::with_capacity(raster.heights.len());
        let mut height_squared = Vec::with_capacity(raster.heights.len());
        let mut x_height = Vec::with_capacity(raster.heights.len());
        let mut z_height = Vec::with_capacity(raster.heights.len());
        for z in 0..raster.depth {
            for x in 0..raster.width {
                let included = raster.is_included(x, z);
                let value = if included { raster.height(x, z) } else { 0.0 };
                mask.push(u64::from(included));
                height.push(value);
                height_squared.push(value * value);
                x_height.push(x as f64 * value);
                z_height.push(z as f64 * value);
            }
        }
        Self {
            mask: IntegralU64::new(raster.width, raster.depth, &mask),
            height: IntegralF64::new(raster.width, raster.depth, &height),
            height_squared: IntegralF64::new(raster.width, raster.depth, &height_squared),
            x_height: IntegralF64::new(raster.width, raster.depth, &x_height),
            z_height: IntegralF64::new(raster.width, raster.depth, &z_height),
        }
    }
}

struct IntegralF64 {
    stride: usize,
    values: Vec<f64>,
}

impl IntegralF64 {
    fn new(width: usize, depth: usize, source: &[f64]) -> Self {
        let stride = width + 1;
        let mut values = vec![0.0; stride * (depth + 1)];
        for z in 0..depth {
            let mut row_sum = 0.0;
            for x in 0..width {
                row_sum += source[z * width + x];
                values[(z + 1) * stride + x + 1] = values[z * stride + x + 1] + row_sum;
            }
        }
        Self { stride, values }
    }

    fn rect(&self, min_x: usize, min_z: usize, max_x: usize, max_z: usize) -> f64 {
        self.values[max_z * self.stride + max_x]
            - self.values[min_z * self.stride + max_x]
            - self.values[max_z * self.stride + min_x]
            + self.values[min_z * self.stride + min_x]
    }
}

struct IntegralU64 {
    stride: usize,
    values: Vec<u64>,
}

impl IntegralU64 {
    fn new(width: usize, depth: usize, source: &[u64]) -> Self {
        let stride = width + 1;
        let mut values = vec![0; stride * (depth + 1)];
        for z in 0..depth {
            let mut row_sum = 0u64;
            for x in 0..width {
                row_sum += source[z * width + x];
                values[(z + 1) * stride + x + 1] = values[z * stride + x + 1] + row_sum;
            }
        }
        Self { stride, values }
    }

    fn rect(&self, min_x: usize, min_z: usize, max_x: usize, max_z: usize) -> u64 {
        self.values[max_z * self.stride + max_x] + self.values[min_z * self.stride + min_x]
            - self.values[min_z * self.stride + max_x]
            - self.values[max_z * self.stride + min_x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_surface_has_zero_detrended_roughness() {
        let width = 65;
        let depth = 65;
        let heights = (0..depth)
            .flat_map(|z| (0..width).map(move |x| 40 + x as i32 * 2 - z as i32))
            .collect::<Vec<_>>();
        let raster =
            TerrainHeightRaster::new(width, depth, heights, vec![true; width * depth]).unwrap();
        let analysis =
            analyze_terrain_height_raster(&raster, &[1, 2, 4, 8, 16, 32], &[2, 4, 8, 16, 32])
                .unwrap();

        assert!(analysis.curvature.max <= f64::EPSILON);
        assert!(
            analysis
                .plane_fit_curve
                .iter()
                .all(|scale| scale.pooled_rmse <= 1e-6)
        );
        assert!((analysis.roughness_exponent.unwrap() - 1.0).abs() <= 1e-9);
    }

    #[test]
    fn checkerboard_has_more_fine_detail_than_a_broad_step() {
        let width = 65;
        let depth = 65;
        let checker = (0..depth)
            .flat_map(|z| (0..width).map(move |x| if (x + z) % 2 == 0 { 70 } else { 74 }))
            .collect::<Vec<_>>();
        let broad = (0..depth)
            .flat_map(|_| (0..width).map(move |x| if x < width / 2 { 70 } else { 74 }))
            .collect::<Vec<_>>();
        let mask = vec![true; width * depth];
        let checker = analyze_terrain_height_raster(
            &TerrainHeightRaster::new(width, depth, checker, mask.clone()).unwrap(),
            &[1, 2, 4, 8, 16, 32],
            &[2, 4, 8, 16, 32],
        )
        .unwrap();
        let broad = analyze_terrain_height_raster(
            &TerrainHeightRaster::new(width, depth, broad, mask).unwrap(),
            &[1, 2, 4, 8, 16, 32],
            &[2, 4, 8, 16, 32],
        )
        .unwrap();

        assert!(checker.curvature.rms > broad.curvature.rms);
        assert!(
            checker.fine_detail_share_r4_of_r32.unwrap()
                > broad.fine_detail_share_r4_of_r32.unwrap()
        );
    }

    #[test]
    fn plane_windows_require_a_complete_included_neighborhood() {
        let width = 17;
        let depth = 17;
        let mut mask = vec![true; width * depth];
        mask[8 * width + 8] = false;
        let raster = TerrainHeightRaster::new(width, depth, vec![72; width * depth], mask).unwrap();
        let analysis = analyze_terrain_height_raster(&raster, &[1, 2, 4, 8], &[2, 4, 8]).unwrap();

        assert!(analysis.plane_fit_curve[0].window_count < (width - 4) * (depth - 4));
        assert!(
            analysis
                .plane_fit_curve
                .iter()
                .all(|scale| scale.radius_blocks != 8)
        );
    }
}
