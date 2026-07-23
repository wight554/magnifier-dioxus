mod jni_glue;

use super::*;

pub struct AndroidCamera;

impl AndroidCamera {
    pub fn new() -> Self {
        Self
    }
}

impl CameraController for AndroidCamera {
    fn start(&self, events: EventSender) {
        std::thread::spawn(move || {
            if !jni_glue::has_camera_permission() {
                jni_glue::request_camera_permission();
                for _ in 0..120 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    if jni_glue::has_camera_permission() {
                        break;
                    }
                }
            }
            if !jni_glue::has_camera_permission() {
                let _ = events.unbounded_send(CameraEvent::Error("no permission".into()));
                return;
            }
            let _ = events.unbounded_send(CameraEvent::Error(
                "camera not implemented yet".into(),
            ));
        });
    }
    fn stop(&self) {}
    fn set_zoom(&self, _: f32) {}
    fn set_torch(&self, _: bool) {}
    fn freeze(&self) {}
    fn unfreeze(&self) {}
}
