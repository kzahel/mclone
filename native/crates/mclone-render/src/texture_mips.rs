pub(crate) struct RgbaMipLevel {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

const ALPHA_CUTOUT_CUTOFF: u8 = 96;

pub(crate) fn generate_rgba_mip_chain(
    width: u32,
    height: u32,
    rgba: &[u8],
    max_level_count: u32,
) -> Vec<RgbaMipLevel> {
    assert!(width > 0 && height > 0);
    assert_eq!(rgba.len(), width as usize * height as usize * 4);

    let max_level_count = max_level_count.max(1);
    let mut levels = Vec::with_capacity(
        rgba_mip_level_count(width, height)
            .min(max_level_count)
            .max(1) as usize,
    );
    levels.push(RgbaMipLevel {
        width,
        height,
        rgba: rgba.to_vec(),
    });
    let transparent = rgba.chunks_exact(4).any(|pixel| pixel[3] == 0);

    while levels.len() < max_level_count as usize
        && levels
            .last()
            .is_some_and(|level| level.width > 1 || level.height > 1)
    {
        let previous = levels.last().expect("base mip level exists");
        levels.push(downsample_rgba_mip(previous, transparent));
    }

    levels
}

fn rgba_mip_level_count(width: u32, height: u32) -> u32 {
    assert!(width > 0 && height > 0);
    u32::BITS - width.max(height).leading_zeros()
}

fn downsample_rgba_mip(previous: &RgbaMipLevel, transparent: bool) -> RgbaMipLevel {
    let width = (previous.width / 2).max(1);
    let height = (previous.height / 2).max(1);
    let mut rgba = vec![0_u8; width as usize * height as usize * 4];

    for y in 0..height {
        let src_y0 = y * previous.height / height;
        let src_y1 = ((y + 1) * previous.height / height).max(src_y0 + 1);
        for x in 0..width {
            let src_x0 = x * previous.width / width;
            let src_x1 = ((x + 1) * previous.width / width).max(src_x0 + 1);
            let mut sum = [0.0_f32; 4];
            let mut count = 0_u32;
            for src_y in src_y0..src_y1.min(previous.height) {
                for src_x in src_x0..src_x1.min(previous.width) {
                    let src_index = ((src_y * previous.width + src_x) * 4) as usize;
                    let pixel = &previous.rgba[src_index..src_index + 4];
                    if !transparent || pixel[3] != 0 {
                        for channel in 0..4 {
                            sum[channel] += srgb_to_linear(pixel[channel]);
                        }
                    }
                    count += 1;
                }
            }
            let dst_index = ((y * width + x) * 4) as usize;
            rgba[dst_index..dst_index + 4].copy_from_slice(&alpha_blend_rgba(
                sum,
                count,
                transparent,
            ));
        }
    }

    RgbaMipLevel {
        width,
        height,
        rgba,
    }
}

fn alpha_blend_rgba(sum: [f32; 4], count: u32, transparent: bool) -> [u8; 4] {
    let divisor = count as f32;
    let mut result = [0_u8; 4];
    for channel in 0..4 {
        result[channel] = linear_to_srgb(sum[channel] / divisor);
    }
    if transparent && result[3] < ALPHA_CUTOUT_CUTOFF {
        result[3] = 0;
    }
    result
}

fn srgb_to_linear(value: u8) -> f32 {
    (f32::from(value) / 255.0).powf(2.2)
}

fn linear_to_srgb(value: f32) -> u8 {
    (value.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_mip_level_count_follows_wgpu_dimensions() {
        assert_eq!(rgba_mip_level_count(1, 1), 1);
        assert_eq!(rgba_mip_level_count(2, 1), 2);
        assert_eq!(rgba_mip_level_count(3, 3), 2);
        assert_eq!(rgba_mip_level_count(4, 2), 3);
        assert_eq!(rgba_mip_level_count(4096, 4096), 13);
    }

    #[test]
    fn generate_rgba_mip_chain_keeps_uniform_opaque_colors_stable() {
        let rgba = [
            255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ];

        let levels = generate_rgba_mip_chain(2, 2, &rgba, u32::MAX);

        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].width, 2);
        assert_eq!(levels[0].height, 2);
        assert_eq!(levels[1].width, 1);
        assert_eq!(levels[1].height, 1);
        assert_eq!(levels[1].rgba, vec![255, 0, 0, 255]);
    }

    #[test]
    fn generate_rgba_mip_chain_applies_java_alpha_cutout() {
        let rgba = [255, 255, 255, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        let levels = generate_rgba_mip_chain(2, 2, &rgba, u32::MAX);

        assert_eq!(levels[1].rgba[3], 0);
    }

    #[test]
    fn generate_rgba_mip_chain_honors_max_level_count() {
        let rgba = vec![255; 64 * 64 * 4];

        let levels = generate_rgba_mip_chain(64, 64, &rgba, 5);

        assert_eq!(levels.len(), 5);
        assert_eq!(levels[4].width, 4);
        assert_eq!(levels[4].height, 4);
    }
}
