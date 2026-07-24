use crate::camera::zoom::{ratio_to_slider, slider_to_ratio};
use crate::camera::CamCaps;
use crate::{i18n, settings};
use dioxus::prelude::*;

#[component]
fn Icon(path: &'static str, size: u32) -> Element {
    rsx! {
        svg {
            view_box: "0 0 24 24",
            width: "{size}",
            height: "{size}",
            fill: "currentColor",
            path { d: "{path}" }
        }
    }
}

const ICON_FLASH: &str = "M7 2v11h3v9l7-12h-4l4-8z";
const ICON_SETTINGS: &str = "M19.14,12.94c0.04,-0.3 0.06,-0.61 0.06,-0.94c0,-0.32 -0.02,-0.64 -0.07,-0.94l2.03,-1.58c0.18,-0.14 0.23,-0.41 0.12,-0.61l-1.92,-3.32c-0.12,-0.22 -0.37,-0.29 -0.59,-0.22l-2.39,0.96c-0.5,-0.38 -1.03,-0.7 -1.62,-0.94L14.4,2.81c-0.04,-0.24 -0.24,-0.41 -0.48,-0.41h-3.84c-0.24,0 -0.43,0.17 -0.47,0.41L9.25,5.35C8.66,5.59 8.12,5.92 7.63,6.29L5.24,5.33c-0.22,-0.08 -0.47,0 -0.59,0.22L2.74,8.87C2.62,9.08 2.66,9.34 2.86,9.48l2.03,1.58C4.84,11.36 4.8,11.69 4.8,12s0.02,0.64 0.07,0.94l-2.03,1.58c-0.18,0.14 -0.23,0.41 -0.12,0.61l1.92,3.32c0.12,0.22 0.37,0.29 0.59,0.22l2.39,-0.96c0.5,0.38 1.03,0.7 1.62,0.94l0.36,2.54c0.05,0.24 0.24,0.41 0.48,0.41h3.84c0.24,0 0.44,-0.17 0.47,-0.41l0.36,-2.54c0.59,-0.24 1.13,-0.56 1.62,-0.94l2.39,0.96c0.22,0.08 0.47,0 0.59,-0.22l1.92,-3.32c0.12,-0.22 0.07,-0.47 -0.12,-0.61L19.14,12.94zM12,15.6c-1.98,0 -3.6,-1.62 -3.6,-3.6s1.62,-3.6 3.6,-3.6s3.6,1.62 3.6,3.6S13.98,15.6 12,15.6z";
const ICON_PAUSE: &str = "M6 19h4V5H6v14zm8-14v14h4V5h-4z";
const ICON_PLAY: &str = "M8 5v14l11-7z";
const ICON_CLOSE: &str = "M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";

#[component]
pub fn Overlay(
    frozen: bool,
    caps: CamCaps,
    zoom: Signal<f32>,
    torch: Signal<bool>,
    show_settings: Signal<bool>,
    cfg: Signal<settings::Settings>,
    on_freeze_toggle: EventHandler<()>,
    on_macro_changed: EventHandler<()>,
) -> Element {
    let mut pinch_start = use_signal(|| None::<(f64, f32)>);
    let mut hold_gen = use_signal(|| 0u64);
    let mut held_via_gesture = use_signal(|| false);

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

            div {
                id: "preview-zone",
                ontouchstart: move |e| {
                    let n = e.touches().len();
                    hold_gen.with_mut(|g| *g += 1);
                    if n == 1 {
                        let my_gen = hold_gen();
                        let was_frozen = frozen;
                        spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(450)).await;
                            if hold_gen() == my_gen && !was_frozen {
                                held_via_gesture.set(true);
                                on_freeze_toggle.call(());
                            }
                        });
                    }
                },
                ontouchmove: move |_| hold_gen.with_mut(|g| *g += 1),
                ontouchend: move |_| {
                    hold_gen.with_mut(|g| *g += 1);
                    if held_via_gesture() {
                        held_via_gesture.set(false);
                        on_freeze_toggle.call(());
                    }
                },
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
                div { id: "action-row",
                    if caps.has_torch {
                        button {
                            class: if torch() { "big-btn active" } else { "big-btn" },
                            onclick: move |_| torch.toggle(),
                            aria_label: i18n::t("torch"),
                            Icon { path: ICON_FLASH, size: 32 }
                        }
                    }
                    button {
                        class: if frozen { "big-btn freeze active" } else { "big-btn freeze" },
                        onclick: move |_| on_freeze_toggle.call(()),
                        aria_label: if frozen { i18n::t("unfreeze") } else { i18n::t("freeze") },
                        Icon { path: if frozen { ICON_PLAY } else { ICON_PAUSE }, size: 32 }
                    }
                    button {
                        class: "big-btn",
                        onclick: move |_| show_settings.set(true),
                        aria_label: i18n::t("settings"),
                        Icon { path: ICON_SETTINGS, size: 32 }
                    }
                }
            }

            if show_settings() {
                SettingsSheet { cfg, show_settings, caps, on_macro_changed }
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
    on_macro_changed: EventHandler<()>,
) -> Element {
    let initial_use_macro = use_signal(|| cfg.peek().use_macro);
    rsx! {
        div { id: "settings-sheet",
            h2 { {i18n::t("settings")} }
            label {
                class: "stacked",
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
            if caps.has_macro {
                label {
                    {i18n::t("use_macro")}
                    input {
                        r#type: "checkbox",
                        checked: cfg().use_macro,
                        onchange: move |e| {
                            let mut c = cfg();
                            c.use_macro = e.checked();
                            cfg.set(c);
                        },
                    }
                }
            }
            button {
                class: "big-btn",
                onclick: move |_| {
                    let _ = settings::save(&settings::settings_path(), &cfg());
                    if cfg().use_macro != initial_use_macro() {
                        on_macro_changed.call(());
                    }
                    show_settings.set(false);
                },
                aria_label: i18n::t("close"),
                Icon { path: ICON_CLOSE, size: 28 }
            }
        }
    }
}
