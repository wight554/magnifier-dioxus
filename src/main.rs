use dioxus::prelude::*;

mod camera;
mod i18n;
mod settings;
mod ui;

use camera::{CamCaps, CameraController, CameraEvent};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    Loading,
    NoPermission,
    Active,
    Frozen,
    Error(String),
}

static MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    #[cfg(not(target_os = "android"))]
    env_logger::init();
    dioxus::launch(app);
}

fn start_camera(
    cam: Arc<dyn CameraController>,
    mut state: Signal<AppState>,
    mut caps: Signal<CamCaps>,
) {
    let (tx, mut rx) = futures_channel::mpsc::unbounded::<CameraEvent>();
    cam.start(tx);
    spawn(async move {
        use futures_util::StreamExt;
        while let Some(ev) = rx.next().await {
            match ev {
                CameraEvent::Ready(c) => {
                    caps.set(c);
                    state.set(AppState::Active);
                }
                CameraEvent::Error(e) => {
                    if e == "no permission" {
                        state.set(AppState::NoPermission);
                    } else {
                        state.set(AppState::Error(e));
                    }
                }
                CameraEvent::Disconnected => {
                    state.set(AppState::Error("disconnected".into()));
                }
            }
        }
    });
}

fn app() -> Element {
    let cam: Arc<dyn CameraController> = use_hook(camera::create);
    let mut state = use_signal(|| AppState::Loading);
    let caps = use_signal(|| CamCaps {
        max_zoom: 8.0,
        has_torch: false,
    });
    let cfg = use_signal(|| settings::load(&settings::settings_path()));
    let zoom = use_signal(|| cfg.peek().default_zoom);
    let torch = use_signal(|| cfg.peek().torch_on_launch);
    let show_settings = use_signal(|| false);

    use_hook({
        let cam = cam.clone();
        move || start_camera(cam.clone(), state, caps)
    });

    use_effect({
        let cam = cam.clone();
        move || cam.set_zoom(zoom())
    });
    use_effect({
        let cam = cam.clone();
        move || cam.set_torch(torch())
    });

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        match state() {
            AppState::Loading => rsx! {
                div { class: "center-msg", {i18n::t("loading")} }
            },
            AppState::NoPermission => rsx! {
                div { class: "center-msg",
                    p { {i18n::t("need_camera")} }
                    p { class: "hint", {i18n::t("open_settings_hint")} }
                }
            },
            AppState::Error(e) => rsx! {
                div { class: "center-msg",
                    p { {i18n::t("camera_error")} }
                    p { class: "hint", "{e}" }
                    button {
                        class: "big-btn",
                        onclick: {
                            let cam = cam.clone();
                            move |_| {
                                state.set(AppState::Loading);
                                start_camera(cam.clone(), state, caps);
                            }
                        },
                        {i18n::t("retry")}
                    }
                }
            },
            AppState::Active | AppState::Frozen => {
                let is_frozen = state() == AppState::Frozen;
                rsx! {
                    ui::controls::Overlay {
                        frozen: is_frozen,
                        caps: caps(),
                        zoom,
                        torch,
                        show_settings,
                        cfg,
                        on_freeze_toggle: {
                            let cam = cam.clone();
                            move |_| {
                                if state() == AppState::Frozen {
                                    cam.unfreeze();
                                    cam.set_torch(torch());
                                    state.set(AppState::Active);
                                } else {
                                    cam.freeze();
                                    state.set(AppState::Frozen);
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}
