mod jni_glue;
mod surface;

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

            // M1 spike: prove the surface is visible under the transparent webview
            // by painting it a solid color, before any real camera code exists.
            let handle = match surface::create_surface_view(1920, 1080) {
                Ok(h) => h,
                Err(e) => {
                    let _ = events.unbounded_send(CameraEvent::Error(format!("surface: {e}")));
                    return;
                }
            };
            unsafe {
                let win = handle.native_window();
                ndk_sys::ANativeWindow_setBuffersGeometry(
                    win,
                    0,
                    0,
                    ndk_sys::AHardwareBuffer_Format::AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM.0
                        as i32,
                );
                let mut buf = std::mem::zeroed::<ndk_sys::ANativeWindow_Buffer>();
                if ndk_sys::ANativeWindow_lock(win, &mut buf, std::ptr::null_mut()) == 0 {
                    let pixels = buf.bits as *mut u32;
                    for i in 0..(buf.stride * buf.height) as isize {
                        *pixels.offset(i) = 0xff00_8800;
                    }
                    ndk_sys::ANativeWindow_unlockAndPost(win);
                }
            }
            let _ = events.unbounded_send(CameraEvent::Ready(CamCaps {
                max_zoom: 8.0,
                has_torch: false,
            }));
            // Keep the surface (and its SurfaceView) alive for the duration of the spike.
            std::mem::forget(handle);
        });
    }
    fn stop(&self) {}
    fn set_zoom(&self, _: f32) {}
    fn set_torch(&self, _: bool) {}
    fn freeze(&self) {}
    fn unfreeze(&self) {}
}
