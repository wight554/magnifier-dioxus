pub mod zoom;
pub mod macro_lens;

#[cfg(not(target_os = "android"))]
mod stub;
#[cfg(target_os = "android")]
mod android;

use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CamCaps {
    pub max_zoom: f32,
    pub has_torch: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraEvent {
    Ready(CamCaps),
    Error(String),
    Disconnected,
}

pub type EventSender = futures_channel::mpsc::UnboundedSender<CameraEvent>;

pub trait CameraController: Send + Sync {
    /// Start (or restart) the camera. Events flow through `events`.
    fn start(&self, events: EventSender);
    /// Release the camera (pause / shutdown).
    fn stop(&self);
    fn set_zoom(&self, ratio: f32);
    fn set_torch(&self, on: bool);
    fn freeze(&self);
    fn unfreeze(&self);
}

pub fn create() -> Arc<dyn CameraController> {
    #[cfg(not(target_os = "android"))]
    {
        Arc::new(stub::StubCamera::default())
    }
    #[cfg(target_os = "android")]
    {
        Arc::new(android::AndroidCamera::new())
    }
}

/// The app's private, writable internal storage directory.
///
/// `dirs::data_local_dir()` has no Android support - it falls into the generic Unix
/// branch, which relies on `$HOME` (unset for Android app processes), so it silently
/// resolves to an unwritable path. Use JNI's `Context.getFilesDir()` instead.
#[cfg(target_os = "android")]
pub fn app_files_dir() -> std::path::PathBuf {
    android::jni_glue::app_files_dir()
}

#[cfg(target_os = "android")]
pub fn keep_screen_on() {
    android::jni_glue::keep_screen_on();
}
