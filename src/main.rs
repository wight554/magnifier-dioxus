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
    NoPermission { permanently_denied: bool },
    Active,
    Frozen,
    Error(String),
}

static MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    #[cfg(not(target_os = "android"))]
    env_logger::init();
    #[cfg(target_os = "android")]
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info));

    LaunchBuilder::new()
        .with_cfg(mobile! {
            dioxus::mobile::Config::new().with_background_color((0, 0, 0, 0))
        })
        .launch(app);
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
            log::info!("magnifier: ui received {ev:?}");
            match ev {
                CameraEvent::Ready(c) => {
                    caps.set(c);
                    state.set(AppState::Active);
                }
                CameraEvent::Error(e) => {
                    if e == "no permission" {
                        state.set(AppState::NoPermission {
                            permanently_denied: false,
                        });
                    } else if e == "no permission permanent" {
                        state.set(AppState::NoPermission {
                            permanently_denied: true,
                        });
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

    // tao's Suspended/Resumed map directly to raw Android onPause/onResume, which fire
    // for ANY focus loss - including our own permission dialog, not just real
    // backgrounding. Only stop while genuinely showing the live preview, and only
    // restart if we're the ones who stopped it, so an in-flight permission request
    // (state: Loading) is never interrupted by its own dialog appearing.
    #[cfg(target_os = "android")]
    {
        use_hook(camera::keep_screen_on);
        let cam = cam.clone();
        let mut stopped_for_background = use_signal(|| false);
        dioxus::mobile::use_wry_event_handler(move |event, _target| {
            use tao::event::Event;
            match event {
                Event::Suspended => {
                    if matches!(state(), AppState::Active | AppState::Frozen) {
                        log::info!("magnifier: app suspended while active, stopping camera");
                        cam.stop();
                        stopped_for_background.set(true);
                    }
                }
                Event::Resumed => {
                    if stopped_for_background() {
                        log::info!("magnifier: app resumed, restarting camera");
                        stopped_for_background.set(false);
                        state.set(AppState::Loading);
                        start_camera(cam.clone(), state, caps);
                    }
                }
                _ => {}
            }
        });
    }

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
            AppState::NoPermission { permanently_denied } => rsx! {
                div { class: "center-msg",
                    p { {i18n::t("need_camera")} }
                    if permanently_denied {
                        p { class: "hint", {i18n::t("open_settings_hint")} }
                    } else {
                        button {
                            class: "big-btn",
                            onclick: {
                                let cam = cam.clone();
                                move |_| {
                                    state.set(AppState::Loading);
                                    start_camera(cam.clone(), state, caps);
                                }
                            },
                            {i18n::t("grant")}
                        }
                    }
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
                                log::info!("magnifier: freeze toggle clicked, state={:?}", state());
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
