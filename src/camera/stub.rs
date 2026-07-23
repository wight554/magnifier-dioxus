use super::*;

#[derive(Default)]
pub struct StubCamera;

impl CameraController for StubCamera {
    fn start(&self, events: EventSender) {
        log::info!("stub camera: start");
        let _ = events.unbounded_send(CameraEvent::Ready(CamCaps {
            max_zoom: 8.0,
            has_torch: true,
        }));
    }
    fn stop(&self) {
        log::info!("stub camera: stop");
    }
    fn set_zoom(&self, ratio: f32) {
        log::info!("stub camera: zoom {ratio}");
    }
    fn set_torch(&self, on: bool) {
        log::info!("stub camera: torch {on}");
    }
    fn freeze(&self) {
        log::info!("stub camera: freeze");
    }
    fn unfreeze(&self) {
        log::info!("stub camera: unfreeze");
    }
}
