/// Heuristic to flag a back-facing camera lens as a dedicated macro lens, based on
/// two Camera2 characteristics: `ACAMERA_LENS_INFO_AVAILABLE_FOCAL_LENGTHS` (mm) and
/// `ACAMERA_LENS_INFO_MINIMUM_FOCUS_DISTANCE` (diopters, i.e. 1/meters). A macro lens
/// has both a short focal length AND a very close minimum focus distance - a short
/// focal length alone is also true of ultra-wide lenses, which aren't macro.
pub fn is_macro(focal_length_mm: f32, min_focus_distance_diopters: f32) -> bool {
    focal_length_mm <= 4.0 && min_focus_distance_diopters >= 20.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typical_main_lens_not_macro() {
        assert!(!is_macro(4.75, 8.3));
    }

    #[test]
    fn typical_ultrawide_not_macro() {
        assert!(!is_macro(1.8, 5.0));
    }

    #[test]
    fn typical_dedicated_macro_lens() {
        assert!(is_macro(3.4, 25.0));
    }

    #[test]
    fn borderline_focal_length_but_far_focus_not_macro() {
        assert!(!is_macro(2.0, 10.0));
    }

    #[test]
    fn fixed_focus_infinity_not_macro() {
        assert!(!is_macro(4.0, 0.0));
    }
}
