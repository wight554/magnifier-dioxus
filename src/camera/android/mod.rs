mod cam2;
mod jni_glue;
mod surface;

use super::*;
use cam2::Cam2;
use std::sync::mpsc as std_mpsc;
use std::sync::Mutex;

enum Cmd {
    SetZoom(f32),
    SetTorch(bool),
    Freeze,
    Unfreeze,
    Stop,
}

pub struct AndroidCamera {
    tx: Mutex<Option<std_mpsc::Sender<Cmd>>>,
}

impl AndroidCamera {
    pub fn new() -> Self {
        Self {
            tx: Mutex::new(None),
        }
    }

    fn send(&self, cmd: Cmd) {
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(cmd);
        }
    }
}

impl CameraController for AndroidCamera {
    fn start(&self, events: EventSender) {
        let (cmd_tx, cmd_rx) = std_mpsc::channel::<Cmd>();
        *self.tx.lock().unwrap() = Some(cmd_tx);

        std::thread::spawn(move || {
            log::info!("magnifier: camera thread started");
            if !jni_glue::has_camera_permission() {
                log::info!("magnifier: requesting camera permission");
                jni_glue::request_camera_permission();
                for _ in 0..120 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    if jni_glue::has_camera_permission() {
                        break;
                    }
                }
            }
            if !jni_glue::has_camera_permission() {
                log::error!("magnifier: camera permission not granted");
                let _ = events.unbounded_send(CameraEvent::Error("no permission".into()));
                return;
            }
            log::info!("magnifier: camera permission granted");

            let mut cam = match Cam2::open_back_camera() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("magnifier: open_back_camera failed: {e:?}");
                    let _ = events.unbounded_send(CameraEvent::Error(format!("open camera: {e}")));
                    return;
                }
            };
            let info = cam.characteristics();
            let max_zoom = info.max_zoom.max(1.0);
            log::info!("magnifier: camera opened, info={info:?}");

            // Inserting our SurfaceView races wry's own dispatch that installs the webview
            // via setContentView on first launch; if ours lands first, setContentView wipes
            // it out and the surface never becomes valid. Retry a few times so a lost race
            // self-corrects instead of leaving the user on a permanent error screen.
            let mut handle = None;
            let mut last_err = None;
            for attempt in 0..3 {
                match surface::create_surface_view(info.preview_w, info.preview_h) {
                    Ok(h) => {
                        handle = Some(h);
                        break;
                    }
                    Err(e) => {
                        log::error!("magnifier: create_surface_view attempt {attempt} failed: {e:?}");
                        last_err = Some(e);
                    }
                }
            }
            let handle = match handle {
                Some(h) => h,
                None => {
                    let e = last_err.unwrap();
                    let _ = events.unbounded_send(CameraEvent::Error(format!("surface: {e}")));
                    return;
                }
            };
            log::info!("magnifier: surface view created");

            if let Err(e) = cam.start_preview(handle.native_window()) {
                log::error!("magnifier: start_preview failed: {e:?}");
                let _ = events.unbounded_send(CameraEvent::Error(format!("start_preview: {e}")));
                return;
            }
            log::info!("magnifier: preview started");

            let settings = crate::settings::load(&crate::settings::settings_path());
            let mut zoom_ratio = settings.default_zoom.clamp(1.0, max_zoom);
            let mut torch_on = settings.torch_on_launch && info.has_torch;
            let mut frozen = false;

            let crop = zoom::crop_region(info.active_w, info.active_h, zoom_ratio);
            if let Err(e) = cam.apply(crop, torch_on) {
                log::error!("magnifier: initial apply failed: {e:?}");
                let _ = events.unbounded_send(CameraEvent::Error(format!("apply: {e}")));
                return;
            }

            log::info!("magnifier: sending Ready event, max_zoom={max_zoom} has_torch={}", info.has_torch);
            let _ = events.unbounded_send(CameraEvent::Ready(CamCaps {
                max_zoom,
                has_torch: info.has_torch,
            }));

            loop {
                match cmd_rx.recv_timeout(std::time::Duration::from_millis(200)) {
                    Ok(Cmd::Freeze) => {
                        log::info!("magnifier: received Freeze, frozen={frozen}");
                        if !frozen {
                            let crop = zoom::crop_region(info.active_w, info.active_h, zoom_ratio);
                            let r = cam.apply(crop, false);
                            log::info!("magnifier: freeze apply(torch=false) -> {r:?}");
                            cam.stop_repeating();
                            log::info!("magnifier: stop_repeating called");
                            frozen = true;
                        }
                    }
                    Ok(Cmd::Unfreeze) => {
                        log::info!("magnifier: received Unfreeze, frozen={frozen}");
                        if frozen {
                            frozen = false;
                            let r = cam.resume_repeating();
                            log::info!("magnifier: resume_repeating -> {r:?}");
                            if r.is_ok() {
                                let crop =
                                    zoom::crop_region(info.active_w, info.active_h, zoom_ratio);
                                let _ = cam.apply(crop, torch_on);
                            }
                        }
                    }
                    Ok(Cmd::SetZoom(r)) => {
                        zoom_ratio = r.clamp(1.0, max_zoom);
                        if !frozen {
                            let crop = zoom::crop_region(info.active_w, info.active_h, zoom_ratio);
                            let _ = cam.apply(crop, torch_on);
                        }
                    }
                    Ok(Cmd::SetTorch(on)) => {
                        torch_on = on && info.has_torch;
                        if !frozen {
                            let crop = zoom::crop_region(info.active_w, info.active_h, zoom_ratio);
                            let _ = cam.apply(crop, torch_on);
                        }
                    }
                    Ok(Cmd::Stop) => break,
                    Err(std_mpsc::RecvTimeoutError::Timeout) => {
                        if cam.is_disconnected() {
                            let _ = events.unbounded_send(CameraEvent::Disconnected);
                            break;
                        }
                    }
                    Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            // `cam` and `handle` drop here: session/device/manager close, native window released.
        });
    }

    fn stop(&self) {
        if let Some(tx) = self.tx.lock().unwrap().take() {
            let _ = tx.send(Cmd::Stop);
        }
    }
    fn set_zoom(&self, ratio: f32) {
        self.send(Cmd::SetZoom(ratio));
    }
    fn set_torch(&self, on: bool) {
        self.send(Cmd::SetTorch(on));
    }
    fn freeze(&self) {
        self.send(Cmd::Freeze);
    }
    fn unfreeze(&self) {
        self.send(Cmd::Unfreeze);
    }
}
