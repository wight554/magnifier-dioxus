pub fn slider_to_ratio(slider: f32, max_zoom: f32) -> f32 {
    let s = slider.clamp(0.0, 1.0);
    max_zoom.powf(s).clamp(1.0, max_zoom)
}

pub fn ratio_to_slider(ratio: f32, max_zoom: f32) -> f32 {
    if max_zoom <= 1.0 {
        return 0.0;
    }
    (ratio.clamp(1.0, max_zoom).ln() / max_zoom.ln()).clamp(0.0, 1.0)
}

/// Centered crop rect in NDK metadata layout: (xmin, ymin, width, height).
pub fn crop_region(active_w: i32, active_h: i32, ratio: f32) -> (i32, i32, i32, i32) {
    let r = ratio.max(1.0);
    let w = (active_w as f32 / r) as i32;
    let h = (active_h as f32 / r) as i32;
    ((active_w - w) / 2, (active_h - h) / 2, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_endpoints() {
        assert!((slider_to_ratio(0.0, 8.0) - 1.0).abs() < 1e-5);
        assert!((slider_to_ratio(1.0, 8.0) - 8.0).abs() < 1e-4);
    }

    #[test]
    fn slider_clamps() {
        assert!((slider_to_ratio(-1.0, 8.0) - 1.0).abs() < 1e-5);
        assert!((slider_to_ratio(2.0, 8.0) - 8.0).abs() < 1e-4);
    }

    #[test]
    fn slider_roundtrip() {
        let r = slider_to_ratio(0.37, 6.0);
        assert!((ratio_to_slider(r, 6.0) - 0.37).abs() < 1e-4);
    }

    #[test]
    fn crop_full_at_1x() {
        assert_eq!(crop_region(4000, 3000, 1.0), (0, 0, 4000, 3000));
    }

    #[test]
    fn crop_half_at_2x_centered() {
        assert_eq!(crop_region(4000, 3000, 2.0), (1000, 750, 2000, 1500));
    }
}
