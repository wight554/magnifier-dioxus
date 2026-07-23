use std::sync::OnceLock;

static IS_UK: OnceLock<bool> = OnceLock::new();

fn is_uk() -> bool {
    *IS_UK.get_or_init(|| {
        sys_locale::get_locale()
            .map(|l| l.to_lowercase().starts_with("uk"))
            .unwrap_or(false)
    })
}

pub fn t(key: &str) -> &'static str {
    let (uk, en) = match key {
        "torch" => ("Ліхтарик", "Torch"),
        "freeze" => ("Стоп-кадр", "Freeze"),
        "unfreeze" => ("Продовжити", "Resume"),
        "settings" => ("Налаштування", "Settings"),
        "default_zoom" => ("Початкове збільшення", "Default zoom"),
        "torch_on_launch" => ("Ліхтарик при запуску", "Torch on at launch"),
        "close" => ("Закрити", "Close"),
        "need_camera" => ("Потрібен дозвіл на камеру", "Camera permission required"),
        "grant" => ("Надати дозвіл", "Grant permission"),
        "open_settings_hint" => (
            "Дозвіл заборонено. Увімкніть камеру в налаштуваннях застосунку.",
            "Permission denied. Enable the camera in app settings.",
        ),
        "camera_error" => ("Помилка камери", "Camera error"),
        "retry" => ("Повторити", "Retry"),
        "loading" => ("Завантаження…", "Loading…"),
        _ => ("?", "?"),
    };
    if is_uk() {
        uk
    } else {
        en
    }
}
