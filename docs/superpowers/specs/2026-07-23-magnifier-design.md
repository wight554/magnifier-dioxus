# Magnifier App — Design

Date: 2026-07-23
Status: approved

## Purpose

Lightweight Android magnifier for a low-vision user: tap the icon, immediately see a zoomed
camera view, optionally toggle the flashlight, read the label. No menus in the critical path.
Faster and lighter than using the stock camera app.

## Requirements

- Opens directly into a live, zoomed rear-camera preview.
- Zoom control: large on-screen slider plus pinch gesture; opens at a configurable default zoom.
- Flashlight (torch) inline toggle; configurable "torch on at launch".
- Tap-to-freeze: freeze the current frame to read comfortably; unfreeze resumes live preview.
- Small settings sheet: default zoom, torch-on-launch. Nothing else.
- UI in Ukrainian and English (Ukrainian when system locale is `uk`, English otherwise;
  no in-app language picker), large high-contrast touch targets, icons over text.
- Minimum Android 10 (API 29).

## Approach decision

Considered:

- **A. Webview `getUserMedia`** — Dioxus UI drives a `<video>` element via JS glue; zoom and
  torch via `applyConstraints`. Simplest, but torch/zoom constraint support varies by device
  and System WebView version.
- **B. Native camera (chosen)** — camera rendered by a native `SurfaceView` under a
  transparent webview; full hardware control, torch and sensor-crop zoom guaranteed by
  Camera2 semantics.
- C. Hybrid (webview preview + JNI torch) — rejected: `CameraManager.setTorchMode` fails
  while the camera is held by another client; torch must come from the owning session.

Within B: **Rust + NDK Camera2** (`ndk`/`ndk-sys` + `jni` crates) chosen over a Kotlin
CameraX bridge. All logic stays in cargo; no Kotlin sources or gradle dependencies injected
into the dx-generated Android project. Cost: manual Camera2 state machine.

## Architecture

Single crate, two layers:

1. **UI layer** — Dioxus 0.7 in the wry webview with a transparent background, layered on
   top. Renders only the control overlay: zoom slider, torch toggle, freeze button, settings
   sheet. The screen center is see-through to the camera preview below.
2. **Camera layer** — Rust. Via JNI, a `SurfaceView` is created and inserted into the
   activity view hierarchy below the webview (view operations dispatched with
   `runOnUiThread`). `ACameraManager` opens the back camera and drives a repeating capture
   request into the surface. Runs on a dedicated camera thread; commands arrive over a
   channel.

Mechanics:

- **Zoom** — `SCALER_CROP_REGION` (API 29 floor), `CONTROL_ZOOM_RATIO` on API 30+. True
  sensor crop, not CSS scaling. Slider bounds come from
  `SCALER_AVAILABLE_MAX_DIGITAL_ZOOM` in camera characteristics.
- **Torch** — `FLASH_MODE_TORCH` on the repeating request (same session, no conflicts).
- **Freeze** — stop the repeating request; the `SurfaceView` keeps displaying the last
  frame. Unfreeze restarts the repeating request.
- **Settings** — JSON file in the app's internal data directory.
- **Permission** — `Activity.requestPermissions` via JNI (framework API, no androidx).
  Custom `AndroidManifest.xml` with `CAMERA` permission and flash/camera `uses-feature`.
- **Desktop stub** — the camera sits behind a trait; a fake desktop impl (gray box, fixed
  capabilities) enables UI iteration with hot reload. Android impl is
  `cfg(target_os = "android")`.

## Components

- `main.rs` — entry point, logging, `launch(App)`.
- `app.rs` — root component. App states: `Loading` → `NoPermission` → `Active` → `Frozen`
  / `Error`. Signals: zoom, torch, frozen, settings.
- `camera/mod.rs` — `CameraController` trait: `start`, `stop`, `set_zoom(f32)`,
  `set_torch(bool)`, `freeze`, `unfreeze`, `capabilities() -> CamCaps { zoom_range,
  has_torch }`. `CameraEvent { Ready, Error(String), Disconnected }` flows to the UI via a
  channel bridged into a signal.
- `camera/android.rs` — JNI surface creation and view insertion, NDK Camera2
  open/session/repeating-request state machine, characteristics query.
- `camera/stub.rs` — desktop fake.
- `settings.rs` — `Settings { default_zoom: f32, torch_on_launch: bool }`, serde JSON,
  load-or-default, save on change.
- `ui/controls.rs` — overlay controls: thick bottom zoom slider, torch button, large
  freeze button, gear opening the settings sheet. Large touch targets, high contrast.
  Pinch-to-zoom is handled here too: the webview covers the whole screen and receives all
  touches, so touch events on the transparent center area are interpreted as pinch and
  written to the zoom signal.

## Data flow

Startup: `main` → load settings → `launch` → root effect checks/requests CAMERA permission
→ camera thread starts → surface created (UI thread) → camera opened → default zoom and
torch applied → `Ready` → state `Active`.

Controls: widget → signal write → trait call → command channel → camera thread → updated
repeating request. Fire-and-forget; the UI never blocks on the camera.

## Error handling

- Permission denied → `NoPermission` screen with large text and a retry button;
  permanently denied → hint to open system app settings.
- Camera error/disconnect callback → `Error` state with a large retry button that fully
  restarts the camera thread.
- No flash unit (`FLASH_INFO_AVAILABLE` false) → torch button hidden.
- Freeze turns the torch off (stopping the repeating request drops torch on many devices);
  unfreeze restores the prior torch state.
- Activity pause/resume → release the camera on pause, reacquire on resume.

## Testing

Unit tests (desktop): settings serde round-trip and load-or-default on missing/corrupt
file; zoom math (slider position → crop rect / zoom ratio, clamped to hardware range).

Manual on-device milestones, riskiest first:

- **M1 spike** — transparent webview over a JNI-inserted `SurfaceView` with a raw preview.
  Proves the three open risks: webview transparency on Android, view layering inside wry's
  activity, custom AndroidManifest support in dx. Hard failure here reopens the approach
  discussion (Kotlin bridge or approach A).
- **M2** — zoom, torch, freeze wired to real controls.
- **M3** — settings sheet and persistence, pause/resume, permission flows, polish.

Desktop loop: `dx serve --platform desktop` with the stub camera for UI work.

No CI or emulator camera automation; emulator cameras fake poorly and the app is small.

## Open risks

1. Wry webview transparency on Android.
2. Inserting a `SurfaceView` below the webview inside wry's generated activity.
3. dx support for a custom `AndroidManifest.xml`.

All three are resolved (or the design is revisited) in M1.
