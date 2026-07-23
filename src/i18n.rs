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
        let en: std::collections::BTreeMap<String, String> =
            serde_yaml::from_str(include_str!("../locales/en.yml")).unwrap();
        let uk: std::collections::BTreeMap<String, String> =
            serde_yaml::from_str(include_str!("../locales/uk.yml")).unwrap();
        let en_keys: std::collections::BTreeSet<_> = en.keys().collect();
        let uk_keys: std::collections::BTreeSet<_> = uk.keys().collect();
        assert_eq!(
            en_keys, uk_keys,
            "locales/en.yml and locales/uk.yml must have identical key sets"
        );
    }
}
