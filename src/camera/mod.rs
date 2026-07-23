pub mod zoom;

#[cfg(not(target_os = "android"))]
mod stub;
// #[cfg(target_os = "android")]
// mod android;
// enabled in Task 6 once src/camera/android/mod.rs exists

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
        compile_error!("android camera controller not wired yet (see Task 6)")
    }
}
