use crate::camera::zoom::{ratio_to_slider, slider_to_ratio};
use crate::camera::CamCaps;
use crate::{i18n, settings};
use dioxus::prelude::*;

#[component]
pub fn Overlay(
    frozen: bool,
    caps: CamCaps,
    zoom: Signal<f32>,
    torch: Signal<bool>,
    show_settings: Signal<bool>,
    cfg: Signal<settings::Settings>,
    on_freeze_toggle: EventHandler<()>,
) -> Element {
    let mut pinch_start = use_signal(|| None::<(f64, f32)>);

    rsx! {
        div {
            id: "overlay",
            ontouchstart: move |e| {
                let t = e.touches();
                if t.len() == 2 {
                    let d = touch_dist(&t);
                    pinch_start.set(Some((d, zoom())));
                }
            },
            ontouchmove: move |e| {
                let t = e.touches();
                if let (2, Some((d0, z0))) = (t.len(), pinch_start()) {
                    let scale = (touch_dist(&t) / d0) as f32;
                    zoom.set((z0 * scale).clamp(1.0, caps.max_zoom));
                }
            },
            ontouchend: move |_| pinch_start.set(None),

            div { id: "top-bar",
                if caps.has_torch {
                    button {
                        class: if torch() { "big-btn active" } else { "big-btn" },
                        onclick: move |_| torch.toggle(),
                        aria_label: i18n::t("torch"),
                        "🔦"
                    }
                }
                button {
                    class: "big-btn",
                    onclick: move |_| show_settings.set(true),
                    aria_label: i18n::t("settings"),
                    "⚙️"
                }
            }

            div { id: "bottom-bar",
                input {
                    id: "zoom-slider",
                    r#type: "range",
                    min: "0",
                    max: "1000",
                    value: "{(ratio_to_slider(zoom(), caps.max_zoom) * 1000.0) as i32}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<f32>() {
                            zoom.set(slider_to_ratio(v / 1000.0, caps.max_zoom));
                        }
                    },
                }
                button {
                    class: if frozen { "big-btn freeze active" } else { "big-btn freeze" },
                    onclick: move |_| on_freeze_toggle.call(()),
                    if frozen { {i18n::t("unfreeze")} } else { {i18n::t("freeze")} }
                }
            }

            if show_settings() {
                SettingsSheet { cfg, show_settings, caps }
            }
        }
    }
}

fn touch_dist(touches: &[dioxus::html::TouchPoint]) -> f64 {
    let a = touches[0].client_coordinates();
    let b = touches[1].client_coordinates();
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

#[component]
fn SettingsSheet(
    cfg: Signal<settings::Settings>,
    show_settings: Signal<bool>,
    caps: CamCaps,
) -> Element {
    rsx! {
        div { id: "settings-sheet",
            h2 { {i18n::t("settings")} }
            label {
                {i18n::t("default_zoom")}
                input {
                    r#type: "range",
                    min: "0",
                    max: "1000",
                    value: "{(ratio_to_slider(cfg().default_zoom, caps.max_zoom) * 1000.0) as i32}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<f32>() {
                            let mut c = cfg();
                            c.default_zoom = slider_to_ratio(v / 1000.0, caps.max_zoom);
                            cfg.set(c);
                        }
                    },
                }
            }
            label {
                {i18n::t("torch_on_launch")}
                input {
                    r#type: "checkbox",
                    checked: cfg().torch_on_launch,
                    onchange: move |e| {
                        let mut c = cfg();
                        c.torch_on_launch = e.checked();
                        cfg.set(c);
                    },
                }
            }
            button {
                class: "big-btn",
                onclick: move |_| {
                    let _ = settings::save(&settings::settings_path(), &cfg());
                    show_settings.set(false);
                },
                {i18n::t("close")}
            }
        }
    }
}
