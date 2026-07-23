# Magnifier App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Android magnifier app — tap icon, see zoomed rear-camera preview, toggle flashlight, freeze frame to read.

**Architecture:** Dioxus 0.7 UI in a transparent webview overlay on top of a native `SurfaceView` inserted below the webview via JNI. All camera logic is Rust using NDK Camera2 (`ndk-sys`) on a dedicated camera thread; UI talks to it through a `CameraController` trait (desktop stub for UI iteration). Spec: `docs/superpowers/specs/2026-07-23-magnifier-design.md`.

**Tech Stack:** Rust, Dioxus 0.7 (`dx` CLI), `jni` 0.21, `ndk-sys` 0.6 (`nativewindow` feature), `ndk-context`, `serde`/`serde_json`, `sys-locale`.

## Global Constraints

- Minimum Android API 29 (Android 10); phone-only, portrait.
- NO Kotlin/Java source files; NO gradle dependency injection. Everything lives in cargo.
- UI strings: Ukrainian when system locale starts with `uk`, English otherwise. No language picker. Never use Russian anywhere (UI, code, comments, commits, docs).
- Large high-contrast touch targets (min 64px), icons over text.
- Conventional Commits; commit at the end of every task.
- Fetch current Dioxus 0.7 docs before using unfamiliar APIs — mobile APIs changed across 0.5/0.6/0.7; do not code from memory.
- On-device verification requires a physical phone. The executing agent must ask the user to run `dx serve --platform android` and report results — never assume device access.

## Known API facts (pre-researched — trust these over memory)

- `ndk-sys` 0.6 ships Camera2 bindings (`ACameraManager_*`, `ACameraDevice_*`, `ACaptureRequest_*`, …) but has **no** link directive for them. The app's `build.rs` must emit `cargo:rustc-link-lib=camera2ndk` (and use the `nativewindow` feature for `ANativeWindow`).
- Android permissions go in `Dioxus.toml` under `[android.permissions]` (e.g. `CAMERA = true`).
- `ndk_context::android_context()` provides the `JavaVM` pointer and the Activity object (wry/tao initialize it).
- Pure JNI cannot implement Java interfaces (`Runnable`, `SurfaceHolder.Callback`) — no runtime class definition. Consequences:
  - UI-thread view operations must go through wry's Android UI-thread dispatch hook (exists for Tauri plugins; find current name in the wry version Dioxus 0.7 pins — historically `wry::prelude::dispatch` or similar in `wry::android`). If it is not reachable from the Dioxus app, STOP and report (fallback would be an embedded-dex Runnable, which is a design change).
  - Surface readiness and permission grants are detected by **polling** (`Surface.isValid()`, `checkSelfPermission`), not callbacks.

---

### Task 1: Scaffold project

**Files:**
- Create: `Cargo.toml`, `Dioxus.toml`, `src/main.rs`, `assets/main.css` (via `dx new` then edit)

**Interfaces:**
- Produces: running Dioxus 0.7 app skeleton, binary name `magnifier`, package `com.magnifier.app`.

- [ ] **Step 1: Scaffold**

Run: `dx new magnifier` (template: bare-bones, no router, no tailwind), then move contents into repo root (the repo root is the crate root — `Cargo.toml` sits next to `docs/`). Verify `dx --version` is 0.7.x first.

- [ ] **Step 2: Configure Dioxus.toml**

Merge into generated `Dioxus.toml` (keep generated keys not shown here):

```toml
[application]
name = "magnifier"

[bundle]
identifier = "com.magnifier.app"
publisher = "magnifier"

[android]

[android.permissions]
CAMERA = true
```

If `dx` 0.7 supports activity attributes in `[android]`, set portrait orientation and min SDK 29 here (check `dx` docs / generated manifest under `target/dx/magnifier/`). If not supported, note it in the task commit message and handle at Task 6 verification — portrait lock is required before release, min SDK 29 is required.

- [ ] **Step 3: Minimal main.rs**

```rust
use dioxus::prelude::*;

fn main() {
    dioxus::launch(app);
}

fn app() -> Element {
    rsx! {
        div { id: "root", "magnifier" }
    }
}
```

- [ ] **Step 4: Verify desktop run**

Run: `dx serve --platform desktop`
Expected: window opens showing "magnifier".

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: scaffold dioxus 0.7 project"
```

---

### Task 2: Settings module (TDD)

**Files:**
- Create: `src/settings.rs`
- Modify: `src/main.rs` (add `mod settings;`)

**Interfaces:**
- Produces:
  - `pub struct Settings { pub default_zoom: f32, pub torch_on_launch: bool }` (Clone, Copy, PartialEq, Serialize, Deserialize)
  - `impl Default for Settings` → `{ default_zoom: 2.0, torch_on_launch: false }`
  - `pub fn load(path: &std::path::Path) -> Settings` — returns default on missing/corrupt file
  - `pub fn save(path: &std::path::Path, s: &Settings) -> std::io::Result<()>`
  - `pub fn settings_path() -> std::path::PathBuf` — Android: `<internal-data>/settings.json` via `dirs::data_local_dir()` fallback to `.`; desktop: same call

Add deps: `cargo add serde --features derive`, `cargo add serde_json`, `cargo add dirs`.

- [ ] **Step 1: Write failing tests**

```rust
// bottom of src/settings.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join("magnifier-test-rt");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        let s = Settings { default_zoom: 3.5, torch_on_launch: true };
        save(&p, &s).unwrap();
        assert_eq!(load(&p), s);
    }

    #[test]
    fn missing_file_gives_default() {
        assert_eq!(load(std::path::Path::new("/nonexistent/x.json")), Settings::default());
    }

    #[test]
    fn corrupt_file_gives_default() {
        let dir = std::env::temp_dir().join("magnifier-test-corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, "{not json").unwrap();
        assert_eq!(load(&p), Settings::default());
    }
}
```

- [ ] **Step 2: Run tests, verify FAIL** — `cargo test settings` → compile error (types missing).

- [ ] **Step 3: Implement**

```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub default_zoom: f32,
    pub torch_on_launch: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { default_zoom: 2.0, torch_on_launch: false }
    }
}

pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(s)?)
}

pub fn settings_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("magnifier/settings.json")
}
```

- [ ] **Step 4: Run tests, verify PASS** — `cargo test settings` → 3 passed.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: settings persistence"`

---

### Task 3: Zoom math (TDD)

**Files:**
- Create: `src/camera/mod.rs` (just `pub mod zoom;` for now), `src/camera/zoom.rs`
- Modify: `src/main.rs` (add `mod camera;`)

**Interfaces:**
- Produces:
  - `pub fn slider_to_ratio(slider: f32, max_zoom: f32) -> f32` — exponential mapping `max_zoom.powf(slider)`, slider clamped to [0,1], result clamped to [1, max_zoom]
  - `pub fn ratio_to_slider(ratio: f32, max_zoom: f32) -> f32` — inverse
  - `pub fn crop_region(active_w: i32, active_h: i32, ratio: f32) -> (i32, i32, i32, i32)` — centered `(xmin, ymin, width, height)` rect for `SCALER_CROP_REGION` (NDK metadata rect layout is x/y/w/h, NOT the Java left/top/right/bottom)

- [ ] **Step 1: Write failing tests**

```rust
// bottom of src/camera/zoom.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_endpoints() {
        assert!((slider_to_ratio(0.0, 8.0) - 1.0).abs() < 1e-5);
        assert!((slider_to_ratio(1.0, 8.0) - 8.0).abs() < 1e-4);
    }

    #[test]
    fn slider_clamps() {
        assert!((slider_to_ratio(-1.0, 8.0) - 1.0).abs() < 1e-5);
        assert!((slider_to_ratio(2.0, 8.0) - 8.0).abs() < 1e-4);
    }

    #[test]
    fn slider_roundtrip() {
        let r = slider_to_ratio(0.37, 6.0);
        assert!((ratio_to_slider(r, 6.0) - 0.37).abs() < 1e-4);
    }

    #[test]
    fn crop_full_at_1x() {
        // (xmin, ymin, width, height) — NDK rect layout
        assert_eq!(crop_region(4000, 3000, 1.0), (0, 0, 4000, 3000));
    }

    #[test]
    fn crop_half_at_2x_centered() {
        assert_eq!(crop_region(4000, 3000, 2.0), (1000, 750, 2000, 1500));
    }
}
```

- [ ] **Step 2: Run, verify FAIL** — `cargo test zoom` → compile error.

- [ ] **Step 3: Implement**

```rust
pub fn slider_to_ratio(slider: f32, max_zoom: f32) -> f32 {
    let s = slider.clamp(0.0, 1.0);
    max_zoom.powf(s).clamp(1.0, max_zoom)
}

pub fn ratio_to_slider(ratio: f32, max_zoom: f32) -> f32 {
    if max_zoom <= 1.0 {
        return 0.0;
    }
    (ratio.clamp(1.0, max_zoom).ln() / max_zoom.ln()).clamp(0.0, 1.0)
}

pub fn crop_region(active_w: i32, active_h: i32, ratio: f32) -> (i32, i32, i32, i32) {
    let r = ratio.max(1.0);
    let w = (active_w as f32 / r) as i32;
    let h = (active_h as f32 / r) as i32;
    // NDK metadata rect layout: (xmin, ymin, width, height)
    ((active_w - w) / 2, (active_h - h) / 2, w, h)
}
```

- [ ] **Step 4: Run, verify PASS** — `cargo test zoom` → 5 passed.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: zoom math"`

---

### Task 4: CameraController trait, events, desktop stub

**Files:**
- Modify: `src/camera/mod.rs`
- Create: `src/camera/stub.rs`

**Interfaces:**
- Produces (used by every later task):

```rust
// src/camera/mod.rs
pub mod zoom;

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
    { Arc::new(stub::StubCamera::default()) }
    #[cfg(target_os = "android")]
    { Arc::new(android::AndroidCamera::new()) }
}
```

Add dep: `cargo add futures-channel`. (The `android` module lands in Task 6 — keep the `#[cfg(target_os = "android")]` lines commented out until then so desktop builds stay green: add them in Task 6.)

- [ ] **Step 1: Implement stub**

```rust
// src/camera/stub.rs
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
    fn stop(&self) { log::info!("stub camera: stop"); }
    fn set_zoom(&self, ratio: f32) { log::info!("stub camera: zoom {ratio}"); }
    fn set_torch(&self, on: bool) { log::info!("stub camera: torch {on}"); }
    fn freeze(&self) { log::info!("stub camera: freeze"); }
    fn unfreeze(&self) { log::info!("stub camera: unfreeze"); }
}
```

Add dep: `cargo add log` (and `cargo add env_logger` + init in `main` for desktop).

- [ ] **Step 2: Verify build** — `cargo check` → clean; `cargo test` → all prior tests pass.

- [ ] **Step 3: Commit** — `git add -A && git commit -m "feat: camera controller trait with desktop stub"`

---

### Task 5: UI overlay + i18n

**Files:**
- Create: `src/ui/mod.rs`, `src/ui/controls.rs`, `src/i18n.rs`
- Modify: `src/main.rs` → becomes app root, `assets/main.css`

**Interfaces:**
- Consumes: `camera::{create, CameraController, CameraEvent, CamCaps}`, `camera::zoom::{slider_to_ratio, ratio_to_slider}`, `settings::{Settings, load, save, settings_path}`
- Produces:
  - `AppState` enum: `Loading | NoPermission | Active | Frozen | Error(String)` (in `src/main.rs`)
  - `i18n::t(key: &str) -> &'static str` — static uk/en tables, locale from `sys_locale::get_locale()` starts-with `"uk"` check, cached in `OnceLock`
  - Components: `ZoomSlider`, `TorchButton`, `FreezeButton`, `SettingsSheet` in `ui/controls.rs`

Add dep: `cargo add sys-locale`.

- [ ] **Step 1: i18n module**

```rust
// src/i18n.rs
use std::sync::OnceLock;

static IS_UK: OnceLock<bool> = OnceLock::new();

fn is_uk() -> bool {
    *IS_UK.get_or_init(|| {
        sys_locale::get_locale()
            .map(|l| l.to_lowercase().starts_with("uk"))
            .unwrap_or(false)
    })
}

pub fn t(key: &str) -> &'static str {
    // (uk, en)
    let (uk, en) = match key {
        "torch" => ("Ліхтарик", "Torch"),
        "freeze" => ("Стоп-кадр", "Freeze"),
        "unfreeze" => ("Продовжити", "Resume"),
        "settings" => ("Налаштування", "Settings"),
        "default_zoom" => ("Початкове збільшення", "Default zoom"),
        "torch_on_launch" => ("Ліхтарик при запуску", "Torch on at launch"),
        "close" => ("Закрити", "Close"),
        "need_camera" => ("Потрібен дозвіл на камеру", "Camera permission required"),
        "grant" => ("Надати дозвіл", "Grant permission"),
        "open_settings_hint" => (
            "Дозвіл заборонено. Увімкніть камеру в налаштуваннях застосунку.",
            "Permission denied. Enable the camera in app settings.",
        ),
        "camera_error" => ("Помилка камери", "Camera error"),
        "retry" => ("Повторити", "Retry"),
        "loading" => ("Завантаження…", "Loading…"),
        _ => ("?", "?"),
    };
    if is_uk() { uk } else { en }
}
```

- [ ] **Step 2: App root with state machine**

```rust
// src/main.rs
mod camera;
mod i18n;
mod settings;
mod ui;

use camera::{CamCaps, CameraController, CameraEvent};
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    Loading,
    NoPermission,
    Active,
    Frozen,
    Error(String),
}

fn main() {
    #[cfg(not(target_os = "android"))]
    env_logger::init();
    dioxus::launch(app);
}

fn app() -> Element {
    let cam: Arc<dyn CameraController> = use_hook(camera::create);
    let mut state = use_signal(|| AppState::Loading);
    let mut caps = use_signal(|| CamCaps { max_zoom: 8.0, has_torch: false });
    let cfg = use_signal(|| settings::load(&settings::settings_path()));
    let mut zoom = use_signal(|| cfg.peek().default_zoom);
    let mut torch = use_signal(|| cfg.peek().torch_on_launch);
    let show_settings = use_signal(|| false);

    // camera event pump
    let cam_for_events = cam.clone();
    use_hook(move || {
        let (tx, mut rx) = futures_channel::mpsc::unbounded::<CameraEvent>();
        cam_for_events.start(tx);
        spawn(async move {
            while let Some(ev) = rx.next().await {
                match ev {
                    CameraEvent::Ready(c) => {
                        caps.set(c);
                        state.set(AppState::Active);
                    }
                    CameraEvent::Error(e) => state.set(AppState::Error(e)),
                    CameraEvent::Disconnected => {
                        state.set(AppState::Error("disconnected".into()))
                    }
                }
            }
        });
    });

    // push zoom/torch changes to camera
    use_effect({
        let cam = cam.clone();
        move || cam.set_zoom(zoom())
    });
    use_effect({
        let cam = cam.clone();
        move || cam.set_torch(torch())
    });

    rsx! {
        document::Stylesheet { href: asset!("/assets/main.css") }
        match state() {
            AppState::Loading => rsx! { div { class: "center-msg", {i18n::t("loading")} } },
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
                                let (tx, _rx) = futures_channel::mpsc::unbounded();
                                cam.start(tx); // real retry wiring refined in Task 9
                            }
                        },
                        {i18n::t("retry")}
                    }
                }
            },
            AppState::Active | AppState::Frozen => rsx! {
                ui::controls::Overlay {
                    frozen: state() == AppState::Frozen,
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
            },
        }
    }
}
```

Add dep: `cargo add futures-util`.

- [ ] **Step 3: Overlay component**

```rust
// src/ui/mod.rs
pub mod controls;
```

```rust
// src/ui/controls.rs
use crate::camera::zoom::{ratio_to_slider, slider_to_ratio};
use crate::camera::CamCaps;
use crate::{i18n, settings};
use dioxus::prelude::*;

#[component]
pub fn Overlay(
    frozen: bool,
    caps: CamCaps,
    zoom: Signal<f32>,
    torch: Signal<bool>,
    show_settings: Signal<bool>,
    cfg: Signal<settings::Settings>,
    on_freeze_toggle: EventHandler<()>,
) -> Element {
    // pinch state: distance between two touches at gesture start
    let mut pinch_start = use_signal(|| None::<(f64, f32)>);

    rsx! {
        div {
            id: "overlay",
            // pinch-to-zoom on the transparent center area
            ontouchstart: move |e| {
                let t = e.touches();
                if t.len() == 2 {
                    let d = dist(&t);
                    pinch_start.set(Some((d, zoom())));
                }
            },
            ontouchmove: move |e| {
                let t = e.touches();
                if let (2, Some((d0, z0))) = (t.len(), pinch_start()) {
                    let scale = (dist(&t) / d0) as f32;
                    zoom.set((z0 * scale).clamp(1.0, caps.max_zoom));
                }
            },
            ontouchend: move |_| pinch_start.set(None),

            div { id: "top-bar",
                if caps.has_torch {
                    button {
                        class: if torch() { "big-btn active" } else { "big-btn" },
                        onclick: move |_| torch.toggle(),
                        aria_label: i18n::t("torch"),
                        "🔦"
                    }
                }
                button {
                    class: "big-btn",
                    onclick: move |_| show_settings.set(true),
                    aria_label: i18n::t("settings"),
                    "⚙️"
                }
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
                button {
                    class: if frozen { "big-btn freeze active" } else { "big-btn freeze" },
                    onclick: move |_| on_freeze_toggle.call(()),
                    if frozen { {i18n::t("unfreeze")} } else { {i18n::t("freeze")} }
                }
            }

            if show_settings() {
                SettingsSheet { cfg, show_settings, caps }
            }
        }
    }
}

fn dist(touches: &[dioxus::html::geometry::ClientPoint]) -> f64 {
    // NOTE: verify exact TouchEvent API in Dioxus 0.7 (e.touches() point access);
    // adjust extraction accordingly.
    let dx = touches[0].x - touches[1].x;
    let dy = touches[0].y - touches[1].y;
    (dx * dx + dy * dy).sqrt()
}

#[component]
fn SettingsSheet(
    cfg: Signal<settings::Settings>,
    show_settings: Signal<bool>,
    caps: CamCaps,
) -> Element {
    rsx! {
        div { id: "settings-sheet",
            h2 { {i18n::t("settings")} }
            label {
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
            button {
                class: "big-btn",
                onclick: move |_| {
                    let _ = settings::save(&settings::settings_path(), &cfg());
                    show_settings.set(false);
                },
                {i18n::t("close")}
            }
        }
    }
}
```

- [ ] **Step 4: CSS — transparent background, big controls**

```css
/* assets/main.css */
html, body, #main {
    margin: 0;
    height: 100%;
    background: transparent; /* camera shows through on Android */
}

#overlay { position: fixed; inset: 0; display: flex; flex-direction: column;
           justify-content: space-between; touch-action: none; }
#top-bar { display: flex; justify-content: space-between; padding: 12px; }
#bottom-bar { display: flex; flex-direction: column; gap: 12px; padding: 16px;
              padding-bottom: 32px; }

.big-btn { min-width: 72px; min-height: 72px; font-size: 32px; border-radius: 16px;
           border: 3px solid #fff; background: rgba(0,0,0,0.55); color: #fff; }
.big-btn.active { background: #ffd400; color: #000; border-color: #ffd400; }
.big-btn.freeze { width: 100%; font-size: 28px; font-weight: 700; }

#zoom-slider { width: 100%; height: 48px; accent-color: #ffd400; }

.center-msg { position: fixed; inset: 0; display: flex; flex-direction: column;
              align-items: center; justify-content: center; gap: 16px;
              background: #000; color: #fff; font-size: 28px; text-align: center;
              padding: 24px; }
.center-msg .hint { font-size: 20px; opacity: 0.8; }

#settings-sheet { position: fixed; left: 0; right: 0; bottom: 0;
                  background: rgba(0,0,0,0.92); color: #fff; padding: 24px;
                  border-radius: 24px 24px 0 0; display: flex;
                  flex-direction: column; gap: 20px; font-size: 24px; }
#settings-sheet label { display: flex; flex-direction: column; gap: 8px; }
```

On desktop set body background to `#333` behind the stub (optional: `@media (hover: hover)` block) so controls are visible.

- [ ] **Step 5: Verify on desktop** — `dx serve --platform desktop`: stub reports Ready → overlay renders; slider moves; torch button toggles (log lines appear); freeze button flips label; settings sheet opens, saves, closes. Fix compile errors against real Dioxus 0.7 APIs (touch events, `document::Stylesheet`) using current docs — the shapes above are close but MUST be validated.

- [ ] **Step 6: Run all tests** — `cargo test` → pass.

- [ ] **Step 7: Commit** — `git add -A && git commit -m "feat: overlay ui with i18n and settings sheet"`

---

### Task 6: Android glue — build config, permission, M1 part A

**Files:**
- Create: `build.rs`, `src/camera/android/mod.rs`, `src/camera/android/jni_glue.rs`
- Modify: `Cargo.toml`, `src/camera/mod.rs` (enable the android cfg lines from Task 4)

**Interfaces:**
- Produces:
  - `jni_glue::with_jni(|env, activity| ...)` — attaches current thread to the VM via `ndk-context`, hands a `&mut JNIEnv` and Activity `JObject`
  - `jni_glue::has_camera_permission() -> bool`
  - `jni_glue::request_camera_permission()`
  - `AndroidCamera::new()` — implements `CameraController` (skeleton: emits `Error("not implemented")` from `start` for now)

- [ ] **Step 1: build.rs + deps**

```rust
// build.rs
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        // ndk-sys declares Camera2 symbols but does not link the library
        println!("cargo:rustc-link-lib=camera2ndk");
    }
}
```

```toml
# Cargo.toml additions
[target.'cfg(target_os = "android")'.dependencies]
jni = "0.21"
ndk-sys = { version = "0.6", features = ["nativewindow"] }
ndk-context = "0.1"
android_logger = "0.14"
```

Init `android_logger` in `main()` under `#[cfg(target_os = "android")]`.

- [ ] **Step 2: JNI glue**

```rust
// src/camera/android/jni_glue.rs
use jni::objects::{JObject, JValue};
use jni::JNIEnv;

pub fn with_jni<R>(f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> jni::errors::Result<R> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    let activity = unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) };
    f(&mut env, &activity)
}

const CAMERA_PERM: &str = "android.permission.CAMERA";
const PERMISSION_GRANTED: i32 = 0;

pub fn has_camera_permission() -> bool {
    with_jni(|env, activity| {
        let perm = env.new_string(CAMERA_PERM)?;
        let res = env
            .call_method(
                activity,
                "checkSelfPermission",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&perm)],
            )?
            .i()?;
        Ok(res == PERMISSION_GRANTED)
    })
    .unwrap_or(false)
}

pub fn request_camera_permission() {
    let _ = with_jni(|env, activity| {
        let perm = env.new_string(CAMERA_PERM)?;
        let arr = env.new_object_array(1, "java/lang/String", &perm)?;
        env.call_method(
            activity,
            "requestPermissions",
            "([Ljava/lang/String;I)V",
            &[JValue::Object(&arr), JValue::Int(1)],
        )?;
        Ok(())
    });
}
```

- [ ] **Step 3: AndroidCamera skeleton**

```rust
// src/camera/android/mod.rs
mod jni_glue;

use super::*;

pub struct AndroidCamera;

impl AndroidCamera {
    pub fn new() -> Self { Self }
}

impl CameraController for AndroidCamera {
    fn start(&self, events: EventSender) {
        std::thread::spawn(move || {
            if !jni_glue::has_camera_permission() {
                jni_glue::request_camera_permission();
                // poll until granted (dialog is async, no callback available)
                for _ in 0..120 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    if jni_glue::has_camera_permission() { break; }
                }
            }
            if !jni_glue::has_camera_permission() {
                let _ = events.unbounded_send(CameraEvent::Error("no permission".into()));
                return;
            }
            let _ = events.unbounded_send(CameraEvent::Error("camera not implemented yet".into()));
        });
    }
    fn stop(&self) {}
    fn set_zoom(&self, _: f32) {}
    fn set_torch(&self, _: bool) {}
    fn freeze(&self) {}
    fn unfreeze(&self) {}
}
```

- [ ] **Step 4: On-device verify (USER RUNS THIS)**

Ask the user to run `dx serve --platform android` with a phone connected and report:
1. App builds and launches.
2. System camera-permission dialog appears on first launch.
3. After granting, the UI shows the camera-error screen with "camera not implemented yet".
4. Check generated manifest at `target/dx/magnifier/**/AndroidManifest.xml` contains `android.permission.CAMERA`; report whether min SDK and portrait could be set from `Dioxus.toml` (see Task 1 Step 2).

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: android jni glue and camera permission flow"`

---

### Task 7: SurfaceView under transparent webview — M1 part B (GATING)

**Files:**
- Create: `src/camera/android/surface.rs`
- Modify: `src/camera/android/mod.rs`

**Interfaces:**
- Produces:
  - `surface::create_surface_view(width: i32, height: i32) -> anyhow::Result<SurfaceHandle>` — creates a `SurfaceView` on the UI thread, inserts it at index 0 of the activity content view (below the webview), sets fixed buffer size, waits (polls) until the surface is valid
  - `SurfaceHandle { pub fn native_window(&self) -> *mut ndk_sys::ANativeWindow }` (global refs held; `Drop` releases them and `ANativeWindow_release`)

Add `cargo add anyhow`.

- [ ] **Step 1: Find the wry UI-thread dispatch API**

Inspect the wry version in `Cargo.lock`. Look for the Android UI-thread dispatch used by Tauri plugins (historically `wry::prelude::dispatch(|env, activity, webview| ...)`). Confirm it is exported and callable from app code. **If no dispatch API is reachable: STOP. Report to the user** — the fallback (embedded dex Runnable) is a design change requiring approval.

- [ ] **Step 2: Implement surface creation (on UI thread via dispatch)**

```rust
// src/camera/android/surface.rs
// Shapes below assume wry's dispatch(|env, activity, _webview|). Adjust to the
// actual signature found in Step 1.
use jni::objects::{GlobalRef, JObject, JValue};
use std::sync::mpsc;

pub struct SurfaceHandle {
    surface_ref: GlobalRef,          // android.view.Surface
    view_ref: GlobalRef,             // android.view.SurfaceView (kept alive)
    native_window: *mut ndk_sys::ANativeWindow,
}
unsafe impl Send for SurfaceHandle {}

const ANDROID_R_ID_CONTENT: i32 = 0x0102_0002;

pub fn create_surface_view(width: i32, height: i32) -> anyhow::Result<SurfaceHandle> {
    let (tx, rx) = mpsc::channel();

    wry::prelude::dispatch(move |env, activity, _webview| {
        let result = (|| -> jni::errors::Result<(GlobalRef, GlobalRef)> {
            let sv = env.new_object(
                "android/view/SurfaceView",
                "(Landroid/content/Context;)V",
                &[JValue::Object(activity)],
            )?;
            let holder = env
                .call_method(&sv, "getHolder", "()Landroid/view/SurfaceHolder;", &[])?
                .l()?;
            env.call_method(
                &holder,
                "setFixedSize",
                "(II)V",
                &[JValue::Int(width), JValue::Int(height)],
            )?;
            let content = env
                .call_method(
                    activity,
                    "findViewById",
                    "(I)Landroid/view/View;",
                    &[JValue::Int(ANDROID_R_ID_CONTENT)],
                )?
                .l()?;
            // index 0 draws first → below the webview
            env.call_method(
                &content,
                "addView",
                "(Landroid/view/View;I)V",
                &[JValue::Object(&sv), JValue::Int(0)],
            )?;
            Ok((env.new_global_ref(&sv)?, env.new_global_ref(&holder)?))
        })();
        let _ = tx.send(result);
    });

    let (view_ref, holder_ref) = rx.recv_timeout(std::time::Duration::from_secs(5))??;

    // Poll from this (camera) thread until the surface is valid.
    // No SurfaceHolder.Callback possible from pure JNI.
    for _ in 0..100 {
        let valid = super::jni_glue::with_jni(|env, _| {
            let surface = env
                .call_method(holder_ref.as_obj(), "getSurface", "()Landroid/view/Surface;", &[])?
                .l()?;
            if surface.is_null() {
                return Ok(None);
            }
            let valid = env.call_method(&surface, "isValid", "()Z", &[])?.z()?;
            if valid {
                Ok(Some(env.new_global_ref(&surface)?))
            } else {
                Ok(None)
            }
        })?;
        if let Some(surface_ref) = valid {
            let native_window = super::jni_glue::with_jni(|env, _| unsafe {
                Ok(ndk_sys::ANativeWindow_fromSurface(
                    env.get_native_interface().cast(),
                    surface_ref.as_obj().as_raw().cast(),
                ))
            })?;
            anyhow::ensure!(!native_window.is_null(), "ANativeWindow_fromSurface returned null");
            return Ok(SurfaceHandle { surface_ref, view_ref, native_window });
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    anyhow::bail!("surface never became valid")
}

impl SurfaceHandle {
    pub fn native_window(&self) -> *mut ndk_sys::ANativeWindow {
        self.native_window
    }
}

impl Drop for SurfaceHandle {
    fn drop(&mut self) {
        unsafe { ndk_sys::ANativeWindow_release(self.native_window) };
        // GlobalRefs drop automatically; view removal happens on app teardown
    }
}
```

- [ ] **Step 3: Webview transparency**

Set the webview/window background transparent from the Dioxus side. Check Dioxus 0.7 mobile `Config` for `with_background_color((0, 0, 0, 0))` or a transparency flag, e.g.:

```rust
// main(): replace dioxus::launch(app) with a configured launch on mobile.
// Exact builder API: check dioxus 0.7 docs (LaunchBuilder + mobile Config).
```

If no config exists, set it via JNI inside the same dispatch: `webview.setBackgroundColor(0)` (`Color.TRANSPARENT`).

- [ ] **Step 4: Temporary M1 wiring**

In `AndroidCamera::start` (after the permission block), call `surface::create_surface_view(1920, 1080)` and paint the native window a solid color to prove visibility WITHOUT the camera:

```rust
// temporary M1 proof, removed in Task 9:
let handle = match surface::create_surface_view(1920, 1080) {
    Ok(h) => h,
    Err(e) => {
        let _ = events.unbounded_send(CameraEvent::Error(format!("surface: {e}")));
        return;
    }
};
unsafe {
    let win = handle.native_window();
    ndk_sys::ANativeWindow_setBuffersGeometry(win, 0, 0,
        ndk_sys::AHardwareBuffer_Format::AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM.0 as i32);
    let mut buf = std::mem::zeroed::<ndk_sys::ANativeWindow_Buffer>();
    if ndk_sys::ANativeWindow_lock(win, &mut buf, std::ptr::null_mut()) == 0 {
        let pixels = buf.bits as *mut u32;
        for i in 0..(buf.stride * buf.height) {
            *pixels.offset(i as isize) = 0xFF00_8800; // opaque green-ish
        }
        ndk_sys::ANativeWindow_unlockAndPost(win);
    }
}
let _ = events.unbounded_send(CameraEvent::Ready(CamCaps { max_zoom: 8.0, has_torch: false }));
std::mem::forget(handle); // keep view alive for the spike
```

- [ ] **Step 5: On-device verify — M1 GATE (USER RUNS THIS)**

`dx serve --platform android`. Expected: green background fills the screen with the overlay controls readable on top. This proves: view insertion below webview works, webview is transparent, UI-thread dispatch works. **If the screen is white/black with no green, or controls invisible — STOP, report findings, do not proceed to Task 8.**

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: native surface view under transparent webview (M1 spike)"`

---

### Task 8: NDK Camera2 wrapper

**Files:**
- Create: `src/camera/android/cam2.rs`
- Modify: `src/camera/android/mod.rs` (add `mod cam2;`)

**Interfaces:**
- Produces (all `pub(super)`, camera-thread only, NOT Send):
  - `Cam2::open_back_camera() -> anyhow::Result<Cam2>` — manager + back-facing id (LENS_FACING_BACK) + open device (state callbacks push onto an internal flag checked by `is_disconnected()`)
  - `Cam2::characteristics(&self) -> CamInfo` where `CamInfo { max_zoom: f32, has_torch: bool, active_w: i32, active_h: i32, preview_w: i32, preview_h: i32 }` — from `ACAMERA_SCALER_AVAILABLE_MAX_DIGITAL_ZOOM`, `ACAMERA_FLASH_INFO_AVAILABLE`, `ACAMERA_SENSOR_INFO_ACTIVE_ARRAY_SIZE`, and a preview size chosen from `ACAMERA_SCALER_AVAILABLE_STREAM_CONFIGURATIONS` (largest ≤ 1920×1080 for `AIMAGE_FORMAT_PRIVATE`/output)
  - `Cam2::start_preview(&mut self, window: *mut ANativeWindow) -> anyhow::Result<()>` — output container + target + session + repeating request (`TEMPLATE_PREVIEW`, `CONTROL_AF_MODE_CONTINUOUS_PICTURE`)
  - `Cam2::apply(&mut self, crop: (i32,i32,i32,i32), torch: bool) -> anyhow::Result<()>` — updates `ACAMERA_SCALER_CROP_REGION` + `ACAMERA_FLASH_MODE` (TORCH/OFF) on the request, re-issues `setRepeatingRequest`
  - `Cam2::stop_repeating(&mut self)` / `Cam2::resume_repeating(&mut self)` — freeze/unfreeze
  - `Cam2::close(self)` — session, device, manager teardown in order
- Consumes: `SurfaceHandle::native_window()` from Task 7.

This is unsafe-FFI-heavy; every `ACAMERA_OK` return code must be checked and turned into `anyhow::Error` with the numeric status. Camera state callbacks (`ACameraDevice_StateCallbacks`) are C function pointers — allowed (this is C ABI, not Java interfaces). Use a `static AtomicBool` for disconnect flagging.

- [ ] **Step 1: Implement `Cam2` open + characteristics.** Skeleton of the FFI call sequence (fill in error checking):

```rust
// open: ACameraManager_create → ACameraManager_getCameraIdList →
//   for each id: ACameraManager_getCameraCharacteristics →
//     ACameraMetadata_getConstEntry(ACAMERA_LENS_FACING) == ACAMERA_LENS_FACING_BACK →
//   ACameraManager_openCamera(id, &state_callbacks, &mut device)
// characteristics (from the chosen id's ACameraMetadata):
//   ACAMERA_SCALER_AVAILABLE_MAX_DIGITAL_ZOOM (f32, default 1.0 if absent)
//   ACAMERA_FLASH_INFO_AVAILABLE (u8 != 0)
//   ACAMERA_SENSOR_INFO_ACTIVE_ARRAY_SIZE (i32[4]: left, top, w, h — NOTE: layout is
//     (xmin, ymin, width, height) in NDK; crop_region() from Task 3 expects w/h)
//   preview size: iterate ACAMERA_SCALER_AVAILABLE_STREAM_CONFIGURATIONS entries
//     (i32[4]: format, width, height, isInput) — pick output entries with
//     format == AIMAGE_FORMAT_PRIVATE (0x22), choose largest w*h with w<=1920, h<=1080;
//     fall back to 1280x720 if none matched
```

- [ ] **Step 2: Implement `start_preview` + `apply` + freeze controls.**

```rust
// start_preview:
//   ACaptureSessionOutputContainer_create
//   ACaptureSessionOutput_create(window) → container add
//   ACameraDevice_createCaptureSession(device, container, &session_callbacks, &mut session)
//   ACameraDevice_createCaptureRequest(device, TEMPLATE_PREVIEW, &mut request)
//   ACameraOutputTarget_create(window) → ACaptureRequest_addTarget
//   set u8 ACAMERA_CONTROL_AF_MODE = ACAMERA_CONTROL_AF_MODE_CONTINUOUS_PICTURE
//   ACameraCaptureSession_setRepeatingRequest(session, null_callbacks, 1, &request, ...)
// apply(crop, torch):
//   ACaptureRequest_setEntry_i32(request, ACAMERA_SCALER_CROP_REGION, 4, crop.as_ptr())
//   ACaptureRequest_setEntry_u8(request, ACAMERA_FLASH_MODE,
//       if torch { ACAMERA_FLASH_MODE_TORCH } else { ACAMERA_FLASH_MODE_OFF })
//   ACameraCaptureSession_setRepeatingRequest(...) again
// stop_repeating: ACameraCaptureSession_stopRepeating(session)  // last frame stays on SurfaceView
// resume_repeating: setRepeatingRequest with the stored request
```

Write out the full Rust for both steps (roughly 250–350 lines) with a small `macro_rules! ck` that converts non-zero `camera_status_t` into `anyhow::bail!("ACamera call {} failed: {}", name, code)`.

- [ ] **Step 3: Compile check for android** — `cargo check --target aarch64-linux-android` (or `dx build --platform android`). Expected: clean (link errors here mean build.rs Step 1 of Task 6 regressed).

- [ ] **Step 4: Commit** — `git add -A && git commit -m "feat: ndk camera2 wrapper"`

---

### Task 9: Camera thread — wire it all (completes M1, delivers M2)

**Files:**
- Modify: `src/camera/android/mod.rs` (replace skeleton internals)

**Interfaces:**
- Consumes: everything produced by Tasks 6–8.
- Produces: fully functional `AndroidCamera` implementing the Task 4 trait. Internal command enum:

```rust
enum Cmd { SetZoom(f32), SetTorch(bool), Freeze, Unfreeze, Stop }
```

- [ ] **Step 1: Implement the camera thread**

`AndroidCamera` holds `Mutex<Option<std::sync::mpsc::Sender<Cmd>>>`. `start()` spawns the thread:

1. permission flow (Task 6; on failure send `Error("no permission")` — the UI maps this string to the `NoPermission` state)
2. `Cam2::open_back_camera()` → `characteristics()`
3. `create_surface_view(preview_w, preview_h)` (remove the green-fill spike code)
4. `start_preview(handle.native_window())`
5. send `CameraEvent::Ready(CamCaps { max_zoom, has_torch })`
6. loop on `rx.recv_timeout(200ms)`: apply `Cmd`s (zoom → `zoom::crop_region(active_w, active_h, ratio)` → `apply`; torch → `apply`; freeze → remember torch state, `apply(crop, false)` then `stop_repeating`; unfreeze → `resume_repeating` + reapply); on timeout check `is_disconnected()` → send `Disconnected` and exit
7. `Cmd::Stop` → `close()`, remove the SurfaceView? No — leave the view, just close the camera (reacquire reuses the surface if still valid, else recreates)

Trait methods forward into the channel; `set_zoom`/`set_torch` before `Ready` are dropped (channel not yet created) — acceptable because the thread applies `Settings` defaults itself at step 4 (pass a `Settings` copy into `start`; extend `AndroidCamera::new()` to take nothing, but read `settings::load` inside the thread).

- [ ] **Step 2: UI retry + NoPermission mapping**

In `src/main.rs`: map `CameraEvent::Error(e)` where `e == "no permission"` to `AppState::NoPermission`; real retry button re-invokes `cam.start(tx)` with a fresh channel wired to the same event pump (refactor the pump into a `start_camera(cam, state, caps)` helper function so retry reuses it).

- [ ] **Step 3: On-device verify — M2 (USER RUNS THIS)**

Checklist for the user, on `dx serve --platform android`:
1. Launch → live rear-camera preview at default zoom (2.0x), sharp close-up focus.
2. Slider changes zoom smoothly 1x→max; pinch works on center area.
3. Torch button lights the flash; button turns yellow; toggling off works.
4. Freeze: image stops, torch goes off; Resume: live again, torch returns.
5. No torch button visible on a device without flash (if available to test).

- [ ] **Step 4: Commit** — `git add -A && git commit -m "feat: live camera preview with zoom, torch, freeze"`

---

### Task 10: Lifecycle + permission edge cases

**Files:**
- Modify: `src/main.rs`, `src/camera/android/mod.rs`, `src/camera/android/jni_glue.rs`

**Interfaces:**
- Consumes: Task 9's `AndroidCamera`.
- Produces: pause/resume handling, permanently-denied UX.

- [ ] **Step 1: Pause/resume**

Find how Dioxus 0.7 mobile surfaces activity lifecycle (tao `Event::Suspended`/`Resumed`, or a dioxus mobile event hook — check current docs). On suspend: `cam.stop()` (camera must be released — holding it breaks other apps and the OS may kill us). On resume: restart via the `start_camera` helper. If no lifecycle hook is reachable from app code, document the limitation in README and rely on Android killing/restarting the camera session (Camera2 disconnect callback → our `Disconnected` → `Error` state with retry) — but exhaust the docs first.

- [ ] **Step 2: Permanently denied**

`jni_glue`: add `should_show_rationale() -> bool` (`Activity.shouldShowRequestPermissionRationale(String)Z`). After a failed permission poll: if rationale is false AND we already asked once (persist an `asked_before` flag inside settings dir as `perm_asked` marker file), show the `open_settings_hint` string (already in i18n). The `NoPermission` screen gains a Grant button that re-calls `request_camera_permission()`.

- [ ] **Step 3: On-device verify (USER RUNS THIS):** background the app → camera releases (screen dark in app switcher is fine); reopen → preview resumes. Deny permission twice → hint text appears.

- [ ] **Step 4: Commit** — `git add -A && git commit -m "feat: lifecycle and permission edge handling"`

---

### Task 11: Final polish

**Files:**
- Modify: `Dioxus.toml` (app display name "Magnifier", icon), `assets/` (icon), `README.md`

- [ ] **Step 1: App icon** — simple high-contrast magnifying-glass icon (generate a 512×512 PNG, e.g. dark background + white glass shape), wire via `[bundle] icon`. Verify `dx` picks it up for Android.

- [ ] **Step 2: README** — one page: what it is, `dx serve --platform desktop` for UI dev, `dx serve --platform android` for device, `dx bundle --platform android --release` for an installable APK, toolchain prerequisites (ANDROID_HOME/NDK_HOME, `rustup target add aarch64-linux-android`).

- [ ] **Step 3: Release build verify (USER RUNS THIS):** `dx bundle --platform android --release`, install APK, cold-start time check — icon tap → readable preview must feel instant (< ~2s).

- [ ] **Step 4: Run full test suite** — `cargo test` → all pass.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "chore: icon, readme, release polish"`

---

## Verification summary

| Milestone | Task | Gate |
|---|---|---|
| M1 | 6–7 | green surface under transparent overlay on device — STOP on failure |
| M2 | 8–9 | live preview + zoom/torch/freeze on device |
| M3 | 10–11 | lifecycle, permission UX, release APK |
