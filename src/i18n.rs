use std::sync::OnceLock;

static IS_UK: OnceLock<bool> = OnceLock::new();

fn is_uk() -> bool {
    *IS_UK.get_or_init(|| {
        sys_locale::get_locale()
            .map(|l| l.to_lowercase().starts_with("uk"))
            .unwrap_or(false)
    })
}

/// Must be called once before any `t()` call (locale doesn't change at runtime).
pub fn init() {
    rust_i18n::set_locale(if is_uk() { "uk" } else { "en" });
}

pub fn t(key: &str) -> String {
    rust_i18n::t!(key).to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn both_locales_have_every_key() {
        for key in [
            "torch", "freeze", "unfreeze", "settings", "default_zoom",
            "torch_on_launch", "close", "need_camera", "grant",
            "open_settings_hint", "camera_error", "retry", "loading", "app_name",
        ] {
            assert_ne!(rust_i18n::t!(key, locale = "en"), key, "missing en key: {key}");
            assert_ne!(rust_i18n::t!(key, locale = "uk"), key, "missing uk key: {key}");
        }
    }
}
