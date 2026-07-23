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

## Round 2: Enhancements (from on-device testing feedback)

Pre-researched facts specific to this round (verified by reading the installed `dioxus-cli-0.7.9` source directly — trust these over the Dioxus docs, which are ahead of what 0.7.9 actually does):

- **Android launcher icon is NOT wired to `[bundle] icon`.** `dioxus-cli`'s bundler only reads `bundle.icon` for macOS/Linux (`src/bundler/macos.rs`, `src/bundler/linux.rs`). For Android, `build_android_app_dir` (`src/build/android.rs`) always writes its own static default `ic_launcher*` files bundled inside the CLI itself — our `assets/icon.png` is silently ignored for the launcher icon. This is why the launcher icon looked "missing" (it's dx's generic default, not ours).
- Since `min_sdk = 29` (> 26), every target device uses the **adaptive icon** path exclusively: `mipmap-anydpi-v26/ic_launcher.xml` → `drawable/ic_launcher_background.xml` + `drawable-v24/ic_launcher_foreground.xml` (both vector XML). The legacy per-density `mipmap-*/ic_launcher.webp` files are dead weight for us — replacing the two vector XML files is sufficient and avoids needing raster image regeneration.
- **Release signing must go in the *deprecated* `[bundle.android]` section, not the documented `[android.signing]`.** The doc comment in `dioxus-cli-0.7.9/src/config/manifest.rs` claims `[android.signing]` "replaces" `[bundle.android]`, but the actual Handlebars data (`hbs_data.android_bundle = self.config.bundle.android.clone()`) and the gradle template (`assets/android/gen/app/build.gradle.kts.hbs`, `signingConfigs { create("release") { storeFile = file(...{{ android_bundle.jks_file }}...) } }`) only ever read `[bundle.android]`. `[android.signing]` is unused dead config in this version. Also: `dx build --release` only runs Gradle's `assembleRelease` at all when `self.config.bundle.android.is_some()` (`src/build/android.rs:385`) — release builds silently stay on `assembleDebug` without this section present.
- **`versionCode` is hardcoded to `1`** in the generated `build.gradle.kts` (`versionCode = 1`, not templated) — every release build has the same versionCode. Android/Obtainium treat same-or-lower versionCode as "not an update," so this must be patched per release or Obtainium will never see new versions as available.
- No per-locale Android string resources are generated by dx (only a single `res/values/strings.xml` with `app_name` from `bundled_app_name()`, which is the crate name PascalCased — "Magnifier" — good enough for English as-is). Adding our own `res/values-uk/strings.xml` alongside it is untouched by dx and picked up natively by Android's resource resolution.
- None of the above are exposed as CLI hooks, so all three (launcher icon, per-locale app name, versionCode) are fixed by editing files inside the Gradle project **after** `dx build`/`dx bundle` generates it, then re-invoking `./gradlew assemble<Debug|Release>` directly — Gradle repackages incrementally, it's fast, and the resulting APK lands at the exact same path `dx` would have produced.
- `rust-i18n` (crate, v4.x) does compile-time YAML loading via the `i18n!("locales", fallback = "en")` macro invoked once at crate root, and a runtime `t!("key")` / `t!("key", locale = "uk")` macro. No assets need to ship at runtime — the YAML is baked in at compile time. This is a drop-in replacement for the hand-rolled `match` in `src/i18n.rs`.
- `EventHandler<T>` and `Signal<T>` are `Copy` in Dioxus 0.7 (confirmed by existing code in `src/ui/controls.rs`, e.g. `on_freeze_toggle` used directly inside multiple closures with no `.clone()`) — new closures below can capture them by copy, same pattern.

---

### Task 12: Keep screen on while the live preview is active

Testing feedback: unlike a normal camera app, the screen was turning off from idle timeout while using the magnifier — even the Quick Settings flashlight toggle keeps the screen active, ours didn't.

**Files:**
- Modify: `src/camera/android/jni_glue.rs`
- Modify: `src/camera/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add `keep_screen_on` to `jni_glue.rs`** — append:

```rust
const FLAG_KEEP_SCREEN_ON: i32 = 0x00000080;

pub fn keep_screen_on() {
    let _ = with_jni(|env, activity| {
        let window = env
            .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
            .l()?;
        env.call_method(&window, "addFlags", "(I)V", &[JValue::Int(FLAG_KEEP_SCREEN_ON)])?;
        Ok(())
    });
}
```

- [ ] **Step 2: Expose it from `camera/mod.rs`** — add next to the existing `app_files_dir` re-export:

```rust
#[cfg(target_os = "android")]
pub fn keep_screen_on() {
    android::jni_glue::keep_screen_on();
}
```

- [ ] **Step 3: Call it once at launch in `main.rs`** — inside the existing `#[cfg(target_os = "android")]` block in `app()` (the one with `use_wry_event_handler`), add before it:

```rust
#[cfg(target_os = "android")]
use_hook(camera::keep_screen_on);
```

- [ ] **Step 4: Verify (USER RUNS THIS)** — `dx serve --android`, open the app, leave the phone untouched for longer than its normal screen-timeout setting (Settings → Display → Screen timeout). Screen must stay on while the app is foregrounded, and behave normally (can still turn off) once backgrounded.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: keep screen on while camera preview is active"`

---

### Task 13: Modern icons — in-app SVG icons + real launcher icon

Testing feedback: emoji buttons look dated; the launcher icon appears to be missing entirely (see Round 2 facts above — dx was silently substituting its own default icon).

**Files:**
- Modify: `src/ui/controls.rs`
- Create: `android-res/drawable/ic_launcher_background.xml`
- Create: `android-res/drawable-v24/ic_launcher_foreground.xml`
- Create: `scripts/android-postprocess.sh`
- Modify: `.gitignore` (not yet, that's Task 18)

- [ ] **Step 1: Add an `Icon` component and Material Symbols path constants to `controls.rs`** — add near the top, after the `use` statements:

```rust
#[component]
fn Icon(path: &'static str, size: u32) -> Element {
    rsx! {
        svg {
            view_box: "0 0 24 24",
            width: "{size}",
            height: "{size}",
            fill: "currentColor",
            path { d: "{path}" }
        }
    }
}

const ICON_FLASH: &str = "M7 2v11h3v9l7-12h-4l4-8z";
const ICON_SETTINGS: &str = "M19.14,12.94c0.04,-0.3 0.06,-0.61 0.06,-0.94c0,-0.32 -0.02,-0.64 -0.07,-0.94l2.03,-1.58c0.18,-0.14 0.23,-0.41 0.12,-0.61l-1.92,-3.32c-0.12,-0.22 -0.37,-0.29 -0.59,-0.22l-2.39,0.96c-0.5,-0.38 -1.03,-0.7 -1.62,-0.94L14.4,2.81c-0.04,-0.24 -0.24,-0.41 -0.48,-0.41h-3.84c-0.24,0 -0.43,0.17 -0.47,0.41L9.25,5.35C8.66,5.59 8.12,5.92 7.63,6.29L5.24,5.33c-0.22,-0.08 -0.47,0 -0.59,0.22L2.74,8.87C2.62,9.08 2.66,9.34 2.86,9.48l2.03,1.58C4.84,11.36 4.8,11.69 4.8,12s0.02,0.64 0.07,0.94l-2.03,1.58c-0.18,0.14 -0.23,0.41 -0.12,0.61l1.92,3.32c0.12,0.22 0.37,0.29 0.59,0.22l2.39,-0.96c0.5,0.38 1.03,0.7 1.62,0.94l0.36,2.54c0.05,0.24 0.24,0.41 0.48,0.41h3.84c0.24,0 0.44,-0.17 0.47,-0.41l0.36,-2.54c0.59,-0.24 1.13,-0.56 1.62,-0.94l2.39,0.96c0.22,0.08 0.47,0 0.59,-0.22l1.92,-3.32c0.12,-0.22 0.07,-0.47 -0.12,-0.61L19.14,12.94zM12,15.6c-1.98,0 -3.6,-1.62 -3.6,-3.6s1.62,-3.6 3.6,-3.6s3.6,1.62 3.6,3.6S13.98,15.6 12,15.6z";
const ICON_PAUSE: &str = "M6 19h4V5H6v14zm8-14v14h4V5h-4z";
const ICON_PLAY: &str = "M8 5v14l11-7z";
const ICON_CLOSE: &str = "M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";
```

(Standard Material Design icon paths, Apache-2.0-licensed, 24×24 viewBox — safe to embed verbatim.)

- [ ] **Step 2: Replace emoji in the torch and settings buttons** — in `Overlay`, replace:

```rust
                        aria_label: i18n::t("torch"),
                        "🔦"
```
with
```rust
                        aria_label: i18n::t("torch"),
                        Icon { path: ICON_FLASH, size: 32 }
```
and
```rust
                    aria_label: i18n::t("settings"),
                    "⚙️"
```
with
```rust
                    aria_label: i18n::t("settings"),
                    Icon { path: ICON_SETTINGS, size: 32 }
```

- [ ] **Step 3: Replace the freeze button's text with icon + keep text as `aria_label`** — replace:

```rust
                    if frozen { {i18n::t("unfreeze")} } else { {i18n::t("freeze")} }
```
with
```rust
                    aria_label: if frozen { i18n::t("unfreeze") } else { i18n::t("freeze") },
                    Icon { path: if frozen { ICON_PLAY } else { ICON_PAUSE }, size: 32 }
```

(Note: `button` already has a `class` attribute in this block — `aria_label` is an additional attribute on the same `button`, add it alongside `class`.)

- [ ] **Step 4: Same for the settings-sheet Close button** — replace its `{i18n::t("close")}` body with `Icon { path: ICON_CLOSE, size: 28 }` and add `aria_label: i18n::t("close"),`.

- [ ] **Step 5: Create the launcher icon vector drawables** — `android-res/drawable/ic_launcher_background.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <path android:fillColor="#1a1a1a" android:pathData="M0,0h108v108h-108z" />
</vector>
```

`android-res/drawable-v24/ic_launcher_foreground.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <path
        android:strokeColor="#ffd400"
        android:strokeWidth="8"
        android:pathData="M44,34 m-18,0 a18,18 0 1,0 36,0 a18,18 0 1,0 -36,0" />
    <path
        android:strokeColor="#ffd400"
        android:strokeWidth="8"
        android:strokeLineCap="round"
        android:pathData="M58,48 L74,64" />
</vector>
```

(Magnifying-glass ring + handle, matching `assets/icon.png`'s motif, centered in the 66dp adaptive-icon safe zone.)

- [ ] **Step 6: Create `scripts/android-postprocess.sh`** (icon patch only for now — extended in Tasks 17 and 18):

```bash
#!/usr/bin/env bash
# Patches the dx-generated Android Gradle project with things dx itself doesn't
# support (see docs/superpowers/plans/2026-07-23-magnifier.md, "Round 2" facts),
# then re-invokes Gradle directly so the patched resources make it into the APK.
#
# Usage: scripts/android-postprocess.sh <debug|release>
# Run AFTER `dx build --android` (debug) or `dx bundle --platform android --release` (release).
set -euo pipefail

PROFILE="${1:?usage: android-postprocess.sh <debug|release>}"
ROOT="target/dx/magnifier/${PROFILE}/android/app"
RES="${ROOT}/app/src/main/res"

if [ ! -d "$ROOT" ]; then
    echo "error: ${ROOT} not found — run dx build/bundle for ${PROFILE} first" >&2
    exit 1
fi

cp android-res/drawable/ic_launcher_background.xml "${RES}/drawable/ic_launcher_background.xml"
cp android-res/drawable-v24/ic_launcher_foreground.xml "${RES}/drawable-v24/ic_launcher_foreground.xml"

GRADLE_TASK="assembleDebug"
if [ "$PROFILE" = "release" ]; then
    GRADLE_TASK="assembleRelease"
fi

( cd "$ROOT" && ./gradlew "$GRADLE_TASK" )

echo "Patched and rebuilt: ${ROOT}/app/build/outputs/apk/${PROFILE}/app-${PROFILE}.apk"
```

Make it executable: `chmod +x scripts/android-postprocess.sh`.

- [ ] **Step 7: Verify (USER RUNS THIS)** — `dx build --android`, then `./scripts/android-postprocess.sh debug`, then `adb install -r target/dx/magnifier/debug/android/app/app/build/outputs/apk/debug/app-debug.apk`. Check the app drawer / home screen — launcher icon must show the yellow magnifying-glass ring, not dx's default icon. In-app torch/settings/freeze buttons must show icons, not emoji.

- [ ] **Step 8: Commit** — `git add -A && git commit -m "feat: material icons in-app, real adaptive launcher icon"`

---

### Task 14: Hold-to-freeze gesture

Testing feedback: holding a finger on the screen should also freeze the frame (in addition to the dedicated button).

**Files:**
- Modify: `src/ui/controls.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `tokio` as an explicit dependency** — add to `[dependencies]` in `Cargo.toml`:

```toml
tokio = { version = "1", default-features = false, features = ["time"] }
```

(Dioxus's desktop/mobile renderer already runs a tokio runtime under the hood; this just makes the crate nameable for `tokio::time::sleep`.)

- [ ] **Step 2: Add a dedicated hit-zone + hold-timer signal in `Overlay`** — restructure so there's a middle zone between the top and bottom bars that owns the 1-finger hold gesture, independent of the existing 2-finger pinch handlers on the outer `#overlay` div. Add after `let mut pinch_start = use_signal(...)`:

```rust
    let mut hold_gen = use_signal(|| 0u64);
```

Add this new middle div right after the closing of the `ontouchend` pinch handler's div block and before `div { id: "top-bar", ...}` (i.e. as the first child of `#overlay`, before `top-bar`):

```rust
            div {
                id: "preview-zone",
                ontouchstart: move |e| {
                    let n = e.touches().len();
                    hold_gen.with_mut(|g| *g += 1);
                    if n == 1 {
                        let my_gen = hold_gen();
                        spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(450)).await;
                            if hold_gen() == my_gen {
                                on_freeze_toggle.call(());
                            }
                        });
                    }
                },
                ontouchmove: move |_| hold_gen.with_mut(|g| *g += 1),
                ontouchend: move |_| hold_gen.with_mut(|g| *g += 1),
            }
```

Every touchstart/move/end bumps the generation counter, invalidating any in-flight timer; a timer only actually fires the freeze toggle if the generation is unchanged 450ms after a single-finger touchstart (i.e., the finger held still, alone, for 450ms). `on_freeze_toggle` is `Copy` (existing pattern in this file), so it can be used directly inside the `spawn`ed closure with no `.clone()`.

- [ ] **Step 3: CSS for the new zone** — add to `assets/main.css`:

```css
#preview-zone { flex: 1; touch-action: none; }
```

- [ ] **Step 4: Verify (USER RUNS THIS)** — `dx serve --android`. Tap-and-release quickly on the live preview → nothing happens (not a hold). Press and hold on the live preview for ~half a second → frame freezes. Release and hold again → unfreezes. Pinching with two fingers must still zoom normally and must NOT also trigger a freeze.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: hold-to-freeze gesture on the live preview"`

---

### Task 15: Bottom-anchored controls, single-line torch-on-launch row, less oversized buttons

Testing feedback: controls would read better anchored to the bottom (freeze between torch and settings); the torch-on-launch checkbox row wraps awkwardly; buttons are a bit larger than they need to be (though generous size is intentional for low-vision users, per the design spec).

**Files:**
- Modify: `src/ui/controls.rs`
- Modify: `assets/main.css`

- [ ] **Step 1: Restructure `Overlay` into a single bottom action row** — replace the `div { id: "top-bar", ... }` and `div { id: "bottom-bar", ... }` blocks entirely with:

```rust
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
                div { id: "action-row",
                    if caps.has_torch {
                        button {
                            class: if torch() { "big-btn active" } else { "big-btn" },
                            onclick: move |_| torch.toggle(),
                            aria_label: i18n::t("torch"),
                            Icon { path: ICON_FLASH, size: 32 }
                        }
                    }
                    button {
                        class: if frozen { "big-btn freeze active" } else { "big-btn freeze" },
                        onclick: move |_| on_freeze_toggle.call(()),
                        aria_label: if frozen { i18n::t("unfreeze") } else { i18n::t("freeze") },
                        Icon { path: if frozen { ICON_PLAY } else { ICON_PAUSE }, size: 32 }
                    }
                    button {
                        class: "big-btn",
                        onclick: move |_| show_settings.set(true),
                        aria_label: i18n::t("settings"),
                        Icon { path: ICON_SETTINGS, size: 32 }
                    }
                }
            }
```

(This already incorporates the icon swap from Task 13's Steps 2–3 — if doing Tasks 13 and 15 in order, this replaces those intermediate edits rather than duplicating them.)

- [ ] **Step 2: Update CSS** — in `assets/main.css`, remove the `#top-bar` rule and replace the `#bottom-bar` / `.big-btn.freeze` rules with:

```css
#bottom-bar {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    padding-bottom: 32px;
}
#action-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
.big-btn.freeze { flex: 1; max-width: 120px; }
```

Reduce button/slider sizing slightly — replace the existing `.big-btn` and `#zoom-slider` rules:

```css
.big-btn {
    min-width: 64px;
    min-height: 64px;
    font-size: 28px;
    border-radius: 16px;
    border: 3px solid #fff;
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
}
.big-btn.active { background: #ffd400; color: #000; border-color: #ffd400; }

#zoom-slider { width: 100%; height: 40px; accent-color: #ffd400; }
```

- [ ] **Step 3: Single-line torch-on-launch row** — in `SettingsSheet`, replace the `label { {i18n::t("torch_on_launch")} input {...} }` block's structure — it already puts text then input as siblings; the fix is CSS-only. In `assets/main.css`, replace:

```css
#settings-sheet label { display: flex; flex-direction: column; gap: 8px; }
```
with
```css
#settings-sheet label { display: flex; flex-direction: row; justify-content: space-between; align-items: center; gap: 12px; }
```

(The default-zoom slider label keeps working fine as a row too — text left, slider needs full width below it, so give that one specific label its own rule. Add a class to distinguish them: in `controls.rs`, change the default-zoom `label { ... }` to `label { class: "stacked", ... }`, and in `main.css` add `#settings-sheet label.stacked { flex-direction: column; align-items: stretch; }` after the row rule above.)

- [ ] **Step 4: Verify (USER RUNS THIS)** — `dx serve --android`. Bottom row shows torch / freeze / settings icons together, zoom slider above it. Open settings — torch-on-launch label and checkbox sit on one line; default-zoom label and its slider still stack. Buttons look slightly less oversized but still clearly tappable.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: bottom-anchored controls, single-line torch setting, tighter sizing"`

---

### Task 16: Proper i18n via `rust-i18n`

Testing feedback: replace the hand-rolled `match` in `src/i18n.rs` with a real i18n mechanism.

**Files:**
- Modify: `Cargo.toml`
- Create: `locales/en.yml`
- Create: `locales/uk.yml`
- Modify: `src/main.rs`
- Modify: `src/i18n.rs`

- [ ] **Step 1: Add the dependency** — add to `Cargo.toml` `[dependencies]`:

```toml
rust-i18n = "4"
```

- [ ] **Step 2: Create locale files** — `locales/en.yml`:

```yaml
torch: Torch
freeze: Freeze
unfreeze: Resume
settings: Settings
default_zoom: Default zoom
torch_on_launch: Torch on at launch
close: Close
need_camera: Camera permission required
grant: Grant permission
open_settings_hint: "Permission denied. Enable the camera in app settings."
camera_error: Camera error
retry: Retry
loading: "Loading…"
app_name: Magnifier
```

`locales/uk.yml`:

```yaml
torch: Ліхтарик
freeze: Стоп-кадр
unfreeze: Продовжити
settings: Налаштування
default_zoom: Початкове збільшення
torch_on_launch: Ліхтарик при запуску
close: Закрити
need_camera: Потрібен дозвіл на камеру
grant: Надати дозвіл
open_settings_hint: "Дозвіл заборонено. Увімкніть камеру в налаштуваннях застосунку."
camera_error: Помилка камери
retry: Повторити
loading: "Завантаження…"
app_name: Лупа
```

- [ ] **Step 3: Invoke the `i18n!` macro once at crate root** — in `src/main.rs`, right after `use dioxus::prelude::*;`:

```rust
rust_i18n::i18n!("locales", fallback = "en");
```

- [ ] **Step 4: Rewrite `src/i18n.rs`**:

```rust
use std::sync::OnceLock;

static IS_UK: OnceLock<bool> = OnceLock::new();

fn is_uk() -> bool {
    *IS_UK.get_or_init(|| {
        sys_locale::get_locale()
            .map(|l| l.to_lowercase().starts_with("uk"))
            .unwrap_or(false)
    })
}

/// Must be called once before any `t()` call (locale doesn't change at runtime).
pub fn init() {
    rust_i18n::set_locale(if is_uk() { "uk" } else { "en" });
}

pub fn t(key: &str) -> String {
    rust_i18n::t!(key).to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn both_locales_have_every_key() {
        for key in [
            "torch", "freeze", "unfreeze", "settings", "default_zoom",
            "torch_on_launch", "close", "need_camera", "grant",
            "open_settings_hint", "camera_error", "retry", "loading", "app_name",
        ] {
            assert_ne!(rust_i18n::t!(key, locale = "en"), key, "missing en key: {key}");
            assert_ne!(rust_i18n::t!(key, locale = "uk"), key, "missing uk key: {key}");
        }
    }
}
```

(`rust_i18n::t!` falls back to returning the key itself if a translation is missing — the test catches any key present in one locale file but not the other.)

- [ ] **Step 5: Call `i18n::init()` once before launch** — in `src/main.rs` `fn main()`, add right before `LaunchBuilder::new()`:

```rust
    i18n::init();
```

- [ ] **Step 6: Run tests** — `cargo test` → `both_locales_have_every_key` passes, plus all prior tests.

- [ ] **Step 7: Commit** — `git add -A && git commit -m "feat: replace hand-rolled i18n with rust-i18n + locale files"`

---

### Task 17: Translated Android app name (launcher label)

Testing feedback: the app name itself should be translated, not just in-app strings — this means the Android launcher label, which dx does not localize (see Round 2 facts).

**Files:**
- Create: `android-res/values-uk/strings.xml`
- Modify: `scripts/android-postprocess.sh`

- [ ] **Step 1: Create the Ukrainian app-name resource** — `android-res/values-uk/strings.xml`:

```xml
<resources>
    <string name="app_name">Лупа</string>
</resources>
```

(The default `res/values/strings.xml` dx generates already reads "Magnifier" — the PascalCased crate name — for English, so no override file is needed there.)

- [ ] **Step 2: Extend `scripts/android-postprocess.sh`** to also install this resource — add before the `GRADLE_TASK=` line:

```bash
mkdir -p "${RES}/values-uk"
cp android-res/values-uk/strings.xml "${RES}/values-uk/strings.xml"
```

- [ ] **Step 3: Verify (USER RUNS THIS)** — `dx build --android`, then `./scripts/android-postprocess.sh debug`, install. With the phone's system language set to Ukrainian, the launcher shows "Лупа"; set back to English (or any other language), it shows "Magnifier".

- [ ] **Step 4: Commit** — `git add -A && git commit -m "feat: translated Android launcher app name"`

---

### Task 18: Release signing (keystore + Dioxus.toml wiring)

Prerequisite for Task 19 (Obtainium needs a stably-signed APK so updates install over the previous version instead of requiring uninstall/reinstall).

**Files:**
- Create: `scripts/generate-release-keystore.sh`
- Create: `scripts/release-apk.sh`
- Modify: `Dioxus.toml`
- Modify: `.gitignore`
- Modify: `scripts/android-postprocess.sh`

- [ ] **Step 1: Add `[bundle.android]` to `Dioxus.toml`** with placeholder passwords (append after the existing `[bundle]` block, before `[android]`):

```toml
[bundle.android]
jks_file = "release.jks"
jks_password = "REPLACED_AT_RELEASE_TIME_STOREPASS"
key_alias = "magnifier-release"
key_password = "REPLACED_AT_RELEASE_TIME_KEYPASS"
```

(This is the section dx's Gradle template actually reads for signing — `[android.signing]` is documented but unused in 0.7.9, see Round 2 facts. The placeholders are never meant to be replaced with real passwords in a committed file — `scripts/release-apk.sh`, Step 3 below, patches them in a throwaway copy at release time only.)

- [ ] **Step 2: Gitignore the keystore and backup file** — add to `.gitignore`:

```
/release.jks
/Dioxus.toml.bak
```

- [ ] **Step 3: Create `scripts/generate-release-keystore.sh`** (run once, ever):

```bash
#!/usr/bin/env bash
set -euo pipefail

KEYSTORE=release.jks
ALIAS=magnifier-release

if [ -f "$KEYSTORE" ]; then
    echo "error: ${KEYSTORE} already exists — refusing to overwrite" >&2
    exit 1
fi

read -srp "New keystore password: " STOREPASS; echo
read -srp "New key password (press enter to reuse the keystore password): " KEYPASS; echo
KEYPASS="${KEYPASS:-$STOREPASS}"

keytool -genkeypair -v \
    -keystore "$KEYSTORE" \
    -alias "$ALIAS" \
    -keyalg RSA -keysize 2048 -validity 10000 \
    -storepass "$STOREPASS" -keypass "$KEYPASS" \
    -dname "CN=Magnifier, OU=, O=, L=, S=, C=UA"

echo
echo "Keystore created at ${KEYSTORE}."
echo "Store the passwords somewhere safe (a password manager) now — they are"
echo "not saved anywhere in this repo, and losing them means you can never"
echo "sign an update to this app again under the same identity."
```

Make it executable: `chmod +x scripts/generate-release-keystore.sh`.

- [ ] **Step 4: Create `scripts/release-apk.sh`** (run every release):

```bash
#!/usr/bin/env bash
# Builds a signed release APK: patches the real keystore passwords into a
# throwaway copy of Dioxus.toml, builds, patches launcher icon / uk app name /
# versionCode via android-postprocess.sh, then restores Dioxus.toml no matter
# what (trap) so real passwords never land in git.
set -euo pipefail

if [ ! -f release.jks ]; then
    echo "error: release.jks not found — run scripts/generate-release-keystore.sh first" >&2
    exit 1
fi

read -srp "Keystore password: " STOREPASS; echo
read -srp "Key password: " KEYPASS; echo

cp Dioxus.toml Dioxus.toml.bak
trap 'mv Dioxus.toml.bak Dioxus.toml' EXIT

python3 - "$STOREPASS" "$KEYPASS" <<'PY'
import sys, pathlib
storepass, keypass = sys.argv[1], sys.argv[2]
p = pathlib.Path("Dioxus.toml")
s = p.read_text()
s = s.replace("REPLACED_AT_RELEASE_TIME_STOREPASS", storepass)
s = s.replace("REPLACED_AT_RELEASE_TIME_KEYPASS", keypass)
p.write_text(s)
PY

dx bundle --platform android --release --package-types apk
./scripts/android-postprocess.sh release

echo "Signed APK: target/dx/magnifier/release/android/app/app/build/outputs/apk/release/app-release.apk"
```

Make it executable: `chmod +x scripts/release-apk.sh`.

- [ ] **Step 5: Add the versionCode bump to `scripts/android-postprocess.sh`** — dx hardcodes `versionCode = 1` in the generated `build.gradle.kts`, so every release needs it patched to something monotonically increasing before Gradle re-assembles. Add, right before the `GRADLE_TASK=` line:

```bash
if [ "$PROFILE" = "release" ]; then
    VERSION_CODE=$(git rev-list --count HEAD)
    sed -i '' "s/versionCode = 1/versionCode = ${VERSION_CODE}/" "${ROOT}/app/build.gradle.kts"
fi
```

(Using the git commit count as versionCode means it's automatically higher than any previous release, with zero manual bookkeeping, as long as at least one commit happened since the last release.)

- [ ] **Step 6: Verify (USER RUNS THIS)** — run `scripts/generate-release-keystore.sh` once, then `scripts/release-apk.sh`. Confirm: the build succeeds, `git diff Dioxus.toml` shows no changes afterward (passwords were reverted), and `git status` shows `release.jks` is NOT tracked. Install the resulting APK on the test device with `adb install -r <path>`, confirm the app runs.

- [ ] **Step 7: Commit** — `git add -A && git commit -m "feat: release signing (keystore, Dioxus.toml wiring, versionCode bump)"`

(Do not commit `release.jks` itself — it's gitignored per Step 2.)

---

### Task 19: Obtainium-trackable releases

Testing feedback: distribute through Obtainium (tracks GitHub Releases directly, no Play Store / F-Droid needed).

**Files:**
- No new source files — this is a process, documented fully in Task 20's README. This task just proves the process end-to-end once.

- [ ] **Step 1: Decide on a version, bump it** — edit `Cargo.toml`'s `version` field (e.g. `0.1.0` → `0.2.0`) to reflect this batch of enhancements. This becomes the APK's `versionName` (Task 18's `versionCode` bump, from commit count, handles the machine-readable ordering Obtainium/Android actually check).

- [ ] **Step 2: Commit the version bump** — `git add Cargo.toml Cargo.lock && git commit -m "chore: bump version to 0.2.0"`.

- [ ] **Step 3: Build the signed release APK (USER RUNS THIS)** — `./scripts/release-apk.sh` (from Task 18).

- [ ] **Step 4: Tag and push (USER RUNS THIS — confirm before pushing tags)** — `git tag v0.2.0 && git push origin v0.2.0` (after confirming with the user; pushing tags is a shared/visible action).

- [ ] **Step 5: Create a GitHub Release with the APK attached (USER RUNS THIS)** — `gh release create v0.2.0 target/dx/magnifier/release/android/app/app/build/outputs/apk/release/app-release.apk --title "v0.2.0" --notes "<what changed>"`.

- [ ] **Step 6: Add to Obtainium (USER RUNS THIS, on their phone)** — in Obtainium: "Add App" → paste the GitHub repo URL (`https://github.com/wight554/magnifier-dioxus`) → source type "GitHub" → it should auto-detect the release APK asset. Confirm Obtainium shows the installed app and reports "up to date" against the tag just created.

- [ ] **Step 7: No code commit for this task** (process-only) — but note the release workflow in the plan's completion so Task 20's README reflects the actual, verified steps rather than a guess.

---

### Task 20: Two-language README with install docs

Testing feedback: add install docs; README should exist in both Ukrainian and English (matching the app's own UI language rule).

**Files:**
- Modify: `README.md` (English, keep at repo root — GitHub convention)
- Create: `README.uk.md` (Ukrainian)

- [ ] **Step 1: Add an install section and language-switch link to the top of `README.md`**, above the existing `# Magnifier` line:

```markdown
[Українською](README.uk.md)

```

Then add a new `## Install` section after the existing intro paragraph and before `## Requirements`, covering the Obtainium flow end-to-end for a non-technical end user:

```markdown
## Install

The easiest way to install and keep this app updated is [Obtainium](https://github.com/ImranR98/Obtainium):

1. Install Obtainium from its [releases page](https://github.com/ImranR98/Obtainium/releases) or F-Droid.
2. In Obtainium, tap "Add App" and paste: `https://github.com/wight554/magnifier-dioxus`
3. Obtainium finds the latest signed release APK automatically and installs it.
4. Future updates show up in Obtainium like any other tracked app.

Alternatively, download the APK directly from the [Releases page](https://github.com/wight554/magnifier-dioxus/releases) and install it manually (you'll need to allow installs from this source in Android's settings).
```

- [ ] **Step 2: Create `README.uk.md`** — full Ukrainian translation of the same structure (intro, Install, Requirements, Desktop dev loop, Android, Testing), with a `[English](README.md)` link at the top mirroring `README.md`'s link back. Use the same section headers translated:

```markdown
[English](README.md)

# Лупа

Android-застосунок для читання дрібного тексту. Тап по іконці — одразу отримуєте
збільшене живе зображення з задньої камери; вмикайте ліхтарик; заморожуйте кадр,
щоб зручно тримати його нерухомо під час читання. Жодних меню на критичному шляху —
усе керується дотиком по самому накладенню.

Створено на [Dioxus](https://dioxuslabs.com) 0.7: прозорий webview рендерить
накладення керування поверх нативного `SurfaceView`, який напряму живиться NDK
Camera2 API (`ndk-sys`). Жодного Kotlin/Java, жодних Gradle-залежностей — увесь
конвеєр камери написаний на Rust.

## Встановлення

Найпростіший спосіб встановити застосунок і отримувати оновлення —
[Obtainium](https://github.com/ImranR98/Obtainium):

1. Встановіть Obtainium з його [сторінки релізів](https://github.com/ImranR98/Obtainium/releases) або F-Droid.
2. В Obtainium натисніть "Add App" і вставте: `https://github.com/wight554/magnifier-dioxus`
3. Obtainium автоматично знайде останній підписаний реліз APK і встановить його.
4. Майбутні оновлення з'являться в Obtainium як для будь-якого іншого застосунку.

Альтернативно, завантажте APK напряму зі [сторінки релізів](https://github.com/wight554/magnifier-dioxus/releases)
і встановіть вручну (потрібно дозволити встановлення з цього джерела в налаштуваннях Android).

## Вимоги

- Android 10 (API 29) або новіше, телефон із задньою камерою.

## Розробка (десктоп)

Лише робота над UI — камера тут заглушка (сіра рамка, фіксовані можливості), тому
ітерація не потребує пристрою:

```sh
dx serve --desktop
```

## Android

Збірка для налагодження + встановлення на підключений пристрій:

```sh
dx serve --android
```

## Тестування

```sh
cargo test
```
```

- [ ] **Step 3: Commit** — `git add -A && git commit -m "docs: add install instructions and Ukrainian README"`

---

## Round 2 verification summary

| Task | Gate |
|---|---|
| 12 | Screen stays on during active preview, on real device |
| 13 | Real launcher icon shows; in-app icons are SVG, not emoji |
| 14 | Press-and-hold on preview freezes/unfreezes; pinch still works |
| 15 | Bottom-anchored single row (torch/freeze/settings); one-line torch checkbox |
| 16 | `cargo test` passes incl. new i18n key-parity test |
| 17 | Launcher label reads "Лупа" under Ukrainian system locale |
| 18 | Signed release APK builds; no secrets land in git |
| 19 | Obtainium successfully tracks and reports the app as up to date |
| 20 | `README.md` and `README.uk.md` both present and cross-linked |

---

## Verification summary

| Milestone | Task | Gate |
|---|---|---|
| M1 | 6–7 | green surface under transparent overlay on device — STOP on failure |
| M2 | 8–9 | live preview + zoom/torch/freeze on device |
| M3 | 10–11 | lifecycle, permission UX, release APK |
