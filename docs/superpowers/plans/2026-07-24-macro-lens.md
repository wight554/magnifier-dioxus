# Macro Lens Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect a dedicated macro camera lens (where the device exposes one as its own top-level Camera2 ID) and let the user opt into it from Settings, persisted, for closer/sharper close-up reading than the main lens's digital zoom can reach.

**Architecture:** `Cam2::open_back_camera` enumerates every back-facing camera ID (not just the first), flags each as macro-or-not via a pure heuristic (short focal length + very close minimum focus distance), and opens whichever the caller asked for (falling back to the main lens if the requested kind isn't present). Switching lenses live means closing and reopening the camera — reuses the exact restart path already used for the background-resume flow. Full design: `docs/superpowers/specs/2026-07-24-macro-lens-design.md`.

**Tech Stack:** Same as the rest of the project — Rust, `ndk-sys` 0.6 Camera2 NDK bindings, Dioxus 0.7 signals for UI wiring.

## Global Constraints

(Same project-wide rules as the original plan's Global Constraints — restated where they bind this work specifically:)

- NO Kotlin/Java source files; NO gradle dependency injection — everything in cargo/Rust.
- UI strings: Ukrainian when system locale starts with `uk`, English otherwise, via the existing `rust-i18n` locale files. Never use Russian anywhere (UI, code, comments, commits, docs).
- Large high-contrast touch targets, icons over text where the app already uses icons — the new checkbox row matches the existing `torch_on_launch` row's style exactly (text + checkbox, not icon-only, since Settings already mixes both).
- Conventional Commits; commit at the end of every task.
- On-device verification requires a physical phone with a macro lens to fully confirm the detection heuristic; the two thresholds (4mm focal length, 20 diopters minimum focus distance) are informed defaults, not device-validated yet — the executing agent must ask the user to run `dx build --android` + install and report the logged per-camera characteristics, and must not assume the heuristic is correct without that log.

## Known facts (pre-researched — trust these over memory)

- `ACAMERA_LENS_INFO_AVAILABLE_FOCAL_LENGTHS` (float array, mm) and `ACAMERA_LENS_INFO_MINIMUM_FOCUS_DISTANCE` (single float, diopters = 1/meters) are both present in the installed `ndk-sys` 0.6.0 bindings (verified: `grep -rn "LENS_INFO_AVAILABLE_FOCAL_LENGTHS\|LENS_INFO_MINIMUM_FOCUS_DISTANCE" ndk-sys-0.6.0+11769913/src/` finds them in every arch's generated bindings file). Access pattern matches the existing `ACAMERA_SCALER_AVAILABLE_MAX_DIGITAL_ZOOM` handling already in `cam2.rs`: float array entries via `entry.data.f` as a slice of `entry.count`, single float entries via `*entry.data.f`.
- A minimum focus distance of `0.0` means "fixed focus at infinity" per the Camera2 docs — correctly falls below the 20-diopter macro threshold, no special-casing needed.
- Dioxus signal-effect re-fire semantics on "set to an equal value" are not something this plan relies on being true — the macro-switch UI wiring below calls `cam.set_zoom(zoom())`/`cam.set_torch(torch())` directly after restarting, rather than depending on `use_effect` re-triggering, to avoid that uncertainty entirely. This mirrors how the mount-time `use_effect` already gets the first zoom/torch value to a freshly-started camera thread (queued into the command channel before the thread's receive loop starts, consumed once it does).
- `SettingsSheet` is conditionally rendered (`if show_settings() { SettingsSheet {...} }`), so Dioxus creates a fresh component instance — and fresh hook state — every time it's shown. A `use_signal` initialized from `cfg.peek().use_macro` inside `SettingsSheet` therefore naturally captures "the value when the sheet was opened" with no extra bookkeeping.

---

### Task 1: Macro-lens detection heuristic

**Files:**
- Create: `src/camera/macro_lens.rs`
- Modify: `src/camera/mod.rs`

**Interfaces:**
- Produces: `pub fn is_macro(focal_length_mm: f32, min_focus_distance_diopters: f32) -> bool`, used by Task 3.

- [ ] **Step 1: Write the module with its tests**

```rust
// src/camera/macro_lens.rs

/// Heuristic to flag a back-facing camera lens as a dedicated macro lens, based on
/// two Camera2 characteristics: `ACAMERA_LENS_INFO_AVAILABLE_FOCAL_LENGTHS` (mm) and
/// `ACAMERA_LENS_INFO_MINIMUM_FOCUS_DISTANCE` (diopters, i.e. 1/meters). A macro lens
/// has both a short focal length AND a very close minimum focus distance - a short
/// focal length alone is also true of ultra-wide lenses, which aren't macro.
pub fn is_macro(focal_length_mm: f32, min_focus_distance_diopters: f32) -> bool {
    focal_length_mm <= 4.0 && min_focus_distance_diopters >= 20.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typical_main_lens_not_macro() {
        assert!(!is_macro(4.75, 8.3));
    }

    #[test]
    fn typical_ultrawide_not_macro() {
        assert!(!is_macro(1.8, 5.0));
    }

    #[test]
    fn typical_dedicated_macro_lens() {
        assert!(is_macro(3.4, 25.0));
    }

    #[test]
    fn borderline_focal_length_but_far_focus_not_macro() {
        assert!(!is_macro(2.0, 10.0));
    }

    #[test]
    fn fixed_focus_infinity_not_macro() {
        assert!(!is_macro(4.0, 0.0));
    }
}
```

- [ ] **Step 2: Register the module** — in `src/camera/mod.rs`, add near the existing `pub mod zoom;`:

```rust
pub mod macro_lens;
```

- [ ] **Step 3: Run tests** — `cargo test macro_lens` → 5 pass.

- [ ] **Step 4: Commit** — `git add -A && git commit -m "feat: macro lens detection heuristic"`

---

### Task 2: `use_macro` setting

**Files:**
- Modify: `src/settings.rs`

**Interfaces:**
- Produces: `Settings.use_macro: bool` (default `false`), read by Task 4, written by Task 5.

- [ ] **Step 1: Add the field**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub default_zoom: f32,
    pub torch_on_launch: bool,
    #[serde(default)]
    pub use_macro: bool,
}
```

`#[serde(default)]` matters here: it makes existing `settings.json` files written by earlier app versions (which have no `use_macro` key) deserialize successfully with `use_macro: false`, instead of failing to parse and silently falling back to `Settings::default()` for everything (losing the user's saved `default_zoom`/`torch_on_launch` too).

- [ ] **Step 2: Update `Default`**

```rust
impl Default for Settings {
    fn default() -> Self {
        Self {
            default_zoom: 2.0,
            torch_on_launch: false,
            use_macro: false,
        }
    }
}
```

- [ ] **Step 3: Update the `roundtrip` test** — in the existing `#[cfg(test)] mod tests`, change:

```rust
        let s = Settings {
            default_zoom: 3.5,
            torch_on_launch: true,
        };
```
to
```rust
        let s = Settings {
            default_zoom: 3.5,
            torch_on_launch: true,
            use_macro: true,
        };
```

- [ ] **Step 4: Add a forward-compatibility test** — confirms Step 1's `#[serde(default)]` reasoning actually holds:

```rust
    #[test]
    fn missing_use_macro_key_defaults_false() {
        let dir = std::env::temp_dir().join("magnifier-test-old-format");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, r#"{"default_zoom":4.0,"torch_on_launch":true}"#).unwrap();
        let loaded = load(&p);
        assert_eq!(loaded.default_zoom, 4.0);
        assert!(loaded.torch_on_launch);
        assert!(!loaded.use_macro);
    }
```

- [ ] **Step 5: Run tests** — `cargo test` → 15 pass (9 pre-existing + Task 1's 5 `macro_lens` tests + this task's 1 new `missing_use_macro_key_defaults_false` test; the `roundtrip` test's count doesn't change, it's modified in place, not added to).

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: add use_macro setting"`

---

### Task 3: Multi-camera enumeration and selection in `Cam2`

**Files:**
- Modify: `src/camera/android/cam2.rs`

**Interfaces:**
- Consumes: `crate::camera::macro_lens::is_macro(f32, f32) -> bool` (Task 1).
- Produces: `CamInfo.has_macro: bool` (device capability, not "is the currently-open lens macro"); `Cam2::open_back_camera(want_macro: bool) -> anyhow::Result<Cam2>` (was `open_back_camera()`, no args).

- [ ] **Step 1: Add `has_macro` to `CamInfo`**

```rust
#[derive(Debug, Clone, Copy)]
pub struct CamInfo {
    pub max_zoom: f32,
    pub has_torch: bool,
    pub has_macro: bool,
    pub active_w: i32,
    pub active_h: i32,
    pub preview_w: i32,
    pub preview_h: i32,
}
```

- [ ] **Step 2: Add the per-lens macro-detection helper** — add as a new method on `Cam2`, near `read_characteristics`:

```rust
    unsafe fn is_macro_lens(metadata: *const ACameraMetadata) -> bool {
        unsafe {
            let mut focal_entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
            let focal_status = ACameraMetadata_getConstEntry(
                metadata,
                acamera_metadata_tag::ACAMERA_LENS_INFO_AVAILABLE_FOCAL_LENGTHS.0,
                &mut focal_entry,
            );
            let min_focal = if focal_status == camera_status_t::ACAMERA_OK && focal_entry.count > 0 {
                std::slice::from_raw_parts(focal_entry.data.f, focal_entry.count as usize)
                    .iter()
                    .cloned()
                    .fold(f32::MAX, f32::min)
            } else {
                f32::MAX
            };

            let mut dist_entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
            let dist_status = ACameraMetadata_getConstEntry(
                metadata,
                acamera_metadata_tag::ACAMERA_LENS_INFO_MINIMUM_FOCUS_DISTANCE.0,
                &mut dist_entry,
            );
            let min_focus_distance = if dist_status == camera_status_t::ACAMERA_OK {
                *dist_entry.data.f
            } else {
                0.0
            };

            log::info!(
                "magnifier: lens characteristics focal_length={min_focal}mm min_focus_distance={min_focus_distance}diopters"
            );

            crate::camera::macro_lens::is_macro(min_focal, min_focus_distance)
        }
    }
```

- [ ] **Step 3: Replace `open_back_camera`'s enumeration loop** — replace the whole function body from the `let mut chosen_id...` line through `let metadata = chosen_metadata;` (i.e. everything between `let ids = &*id_list;` and `let info = Self::read_characteristics(metadata)?;`) with:

```rust
            let mut candidates: Vec<(CString, *mut ACameraMetadata, bool)> = Vec::new();

            for i in 0..ids.numCameras {
                let id_ptr = *ids.cameraIds.offset(i as isize);
                let mut metadata: *mut ACameraMetadata = std::ptr::null_mut();
                ck!(
                    "ACameraManager_getCameraCharacteristics",
                    ACameraManager_getCameraCharacteristics(manager, id_ptr, &mut metadata)
                );

                let mut entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
                ck!(
                    "ACameraMetadata_getConstEntry(LENS_FACING)",
                    ACameraMetadata_getConstEntry(
                        metadata,
                        acamera_metadata_tag::ACAMERA_LENS_FACING.0,
                        &mut entry
                    )
                );
                let facing = *entry.data.u8_;

                if facing == acamera_metadata_enum_acamera_lens_facing::ACAMERA_LENS_FACING_BACK.0 as u8 {
                    let is_macro = Self::is_macro_lens(metadata);
                    let id = CStr::from_ptr(id_ptr).to_owned();
                    log::info!("magnifier: back camera {id:?} is_macro={is_macro}");
                    candidates.push((id, metadata, is_macro));
                } else {
                    ACameraMetadata_free(metadata);
                }
            }

            ACameraManager_deleteCameraIdList(id_list);

            anyhow::ensure!(!candidates.is_empty(), "no back-facing camera found");

            let has_macro = candidates.iter().any(|(_, _, is_macro)| *is_macro);
            let chosen_index = candidates
                .iter()
                .position(|(_, _, is_macro)| *is_macro == want_macro)
                .unwrap_or(0);

            let (id, metadata, _) = candidates.remove(chosen_index);
            for (_, leftover_metadata, _) in candidates {
                ACameraMetadata_free(leftover_metadata);
            }
```

This is a straight 1:1 replacement — the new block still calls `ACameraManager_deleteCameraIdList(id_list);` exactly once, right after the loop, in the same place the original code did; nothing is duplicated.

- [ ] **Step 4: Update the function signature and `read_characteristics` call** — change:

```rust
    pub fn open_back_camera() -> anyhow::Result<Cam2> {
```
to
```rust
    pub fn open_back_camera(want_macro: bool) -> anyhow::Result<Cam2> {
```

and change:
```rust
            let info = Self::read_characteristics(metadata)?;
```
to
```rust
            let mut info = Self::read_characteristics(metadata)?;
            info.has_macro = has_macro;
```

- [ ] **Step 5: Update `read_characteristics`'s return** — it constructs `CamInfo { max_zoom, has_torch, active_w, active_h, preview_w, preview_h }` — add `has_macro: false,` to that struct literal (Step 4 overwrites it right after the call with the real value; this just satisfies the compiler since `CamInfo` now has one more field and `read_characteristics` doesn't have `has_macro` in scope).

- [ ] **Step 6: Verify it compiles** — `dx build --android --target aarch64-linux-android` (requires the Android toolchain env vars already documented in this project's README) → succeeds with no new errors.

- [ ] **Step 7: Commit** — `git add -A && git commit -m "feat: enumerate and select among multiple back cameras for macro lens support"`

---

### Task 4: Thread `want_macro` through `AndroidCamera::start`

**Files:**
- Modify: `src/camera/android/mod.rs`

**Interfaces:**
- Consumes: `Cam2::open_back_camera(bool)` (Task 3), `Settings.use_macro` (Task 2).
- Produces: `CameraEvent::Ready`'s `CamCaps` now carries real `has_macro` (was previously not present in `CamCaps` at all — see Task 5 for the `CamCaps` struct change, which lives in `camera/mod.rs`).

- [ ] **Step 1: Move the settings load earlier and pass `use_macro` to `open_back_camera`** — currently `let settings = crate::settings::load(...)` happens well after camera open (used only for the initial zoom/torch apply). Move it to right after the permission check succeeds (`log::info!("magnifier: camera permission granted");`), before `Cam2::open_back_camera()` is called, and pass its `use_macro` field through. Change:

```rust
            log::info!("magnifier: camera permission granted");

            let mut cam = match Cam2::open_back_camera() {
```
to
```rust
            log::info!("magnifier: camera permission granted");

            let settings = crate::settings::load(&crate::settings::settings_path());

            let mut cam = match Cam2::open_back_camera(settings.use_macro) {
```

- [ ] **Step 2: Remove the now-duplicate later settings load** — delete this line (it's redundant now that Step 1 loads it earlier and the `settings` variable is already in scope for the rest of the function):

```rust
            let settings = crate::settings::load(&crate::settings::settings_path());
```

(This is the line right before `let mut zoom_ratio = settings.default_zoom.clamp(1.0, max_zoom);` — only that one `let settings = ...` line is deleted; `zoom_ratio`/`torch_on`/`frozen` below it stay exactly as they are, now referencing the `settings` bound in Step 1.)

- [ ] **Step 3: Pass `has_macro` into the `Ready` event** — change:

```rust
            log::info!("magnifier: sending Ready event, max_zoom={max_zoom} has_torch={}", info.has_torch);
            let _ = events.unbounded_send(CameraEvent::Ready(CamCaps {
                max_zoom,
                has_torch: info.has_torch,
            }));
```
to
```rust
            log::info!(
                "magnifier: sending Ready event, max_zoom={max_zoom} has_torch={} has_macro={}",
                info.has_torch, info.has_macro
            );
            let _ = events.unbounded_send(CameraEvent::Ready(CamCaps {
                max_zoom,
                has_torch: info.has_torch,
                has_macro: info.has_macro,
            }));
```

(This won't compile until Task 5 adds `has_macro` to the `CamCaps` struct — that's fine, Task 5 does it immediately after.)

- [ ] **Step 4: Commit** — `git add -A && git commit -m "feat: apply the saved macro-lens preference when starting the camera"`

(Verification deferred to Task 5's Step, since this task alone doesn't compile standalone — `CamCaps` gains `has_macro` there.)

---

### Task 5: `CamCaps.has_macro` + macro toggle switching flow

**Files:**
- Modify: `src/camera/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/ui/controls.rs`

**Interfaces:**
- Consumes: `CameraEvent::Ready(CamCaps)` now carrying `has_macro` (Task 4).
- Produces: `Overlay`'s new `on_macro_changed: EventHandler<()>` prop, forwarded into `SettingsSheet`.

- [ ] **Step 1: Add `has_macro` to `CamCaps`** — in `src/camera/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CamCaps {
    pub max_zoom: f32,
    pub has_torch: bool,
    pub has_macro: bool,
}
```

- [ ] **Step 2: Update the desktop stub** — `src/camera/stub.rs` constructs a `CamCaps` for its immediate `Ready` event (per this project's existing desktop-dev-loop stub). Find that struct literal and add `has_macro: false,` to it (the stub never has a macro lens — desktop dev is UI-only iteration, no real camera).

- [ ] **Step 3: Update `main.rs`'s placeholder `caps` default** — change:

```rust
    let caps = use_signal(|| CamCaps {
        max_zoom: 8.0,
        has_torch: false,
    });
```
to
```rust
    let caps = use_signal(|| CamCaps {
        max_zoom: 8.0,
        has_torch: false,
        has_macro: false,
    });
```

- [ ] **Step 4: Add the `on_macro_changed` prop to `Overlay` and wire it in `main.rs`** — in `src/ui/controls.rs`, add a new prop to the `Overlay` component's signature, alongside the existing `on_freeze_toggle: EventHandler<()>`:

```rust
    on_macro_changed: EventHandler<()>,
```

Forward it into `SettingsSheet`'s invocation (inside `Overlay`'s rsx, where it currently does `SettingsSheet { cfg, show_settings, caps }`):

```rust
                SettingsSheet { cfg, show_settings, caps, on_macro_changed }
```

In `src/main.rs`, pass a new handler when constructing `Overlay` (alongside the existing `on_freeze_toggle`):

```rust
                        on_macro_changed: {
                            let cam = cam.clone();
                            move |_| {
                                log::info!("magnifier: macro toggle changed, restarting camera");
                                cam.stop();
                                state.set(AppState::Loading);
                                start_camera(cam.clone(), state, caps);
                                cam.set_zoom(zoom());
                                cam.set_torch(torch());
                            }
                        },
```

The `cam.set_zoom(zoom())`/`cam.set_torch(torch())` calls immediately after `start_camera` queue the current live zoom/torch into the new camera thread's command channel (same mechanism the mount-time `use_effect` already relies on) so they carry over to the newly-selected lens instead of it starting from the saved `default_zoom`/`torch_on_launch`.

- [ ] **Step 5: Add the checkbox to `SettingsSheet`** — add `on_macro_changed: EventHandler<()>` to `SettingsSheet`'s prop signature, alongside its existing `cfg`, `show_settings`, `caps` props. Add a signal capturing the value at mount time, right after the component's opening brace (before its `rsx!`):

```rust
    let initial_use_macro = use_signal(|| cfg.peek().use_macro);
```

Add the checkbox row itself, only when `caps.has_macro` is true — insert it right after the existing `torch_on_launch` `label { ... }` block and before the Close `button`:

```rust
            if caps.has_macro {
                label {
                    {i18n::t("use_macro")}
                    input {
                        r#type: "checkbox",
                        checked: cfg().use_macro,
                        onchange: move |e| {
                            let mut c = cfg();
                            c.use_macro = e.checked();
                            cfg.set(c);
                        },
                    }
                }
            }
```

(No `class: "stacked"` — this follows the `torch_on_launch` row's plain single-line style, matching Task 15's existing row-layout CSS default.)

- [ ] **Step 6: Call `on_macro_changed` from the Close button, only if it actually changed** — the Close button's `onclick` currently does:

```rust
                onclick: move |_| {
                    let _ = settings::save(&settings::settings_path(), &cfg());
                    show_settings.set(false);
                },
```
change to:
```rust
                onclick: move |_| {
                    let _ = settings::save(&settings::settings_path(), &cfg());
                    if cfg().use_macro != initial_use_macro() {
                        on_macro_changed.call(());
                    }
                    show_settings.set(false);
                },
```

- [ ] **Step 7: Add the i18n key** — add to both `locales/en.yml` and `locales/uk.yml`:

`locales/en.yml`: `use_macro: Macro lens`
`locales/uk.yml`: `use_macro: Макрооб'єктив`

- [ ] **Step 8: Run the full test suite** — `cargo test` → all pass (existing count + Task 1's 5 + Task 2's new one).

- [ ] **Step 9: Verify Android build compiles** — `dx build --android --target aarch64-linux-android` → succeeds.

- [ ] **Step 10: Verify on device (USER RUNS THIS)** — `dx serve --android` (or `dx build --android` + install via the project's usual `adb install -r` flow). Check `adb logcat | grep "lens characteristics\|is_macro"` for the logged focal length / minimum focus distance of every back camera on the test phone — confirm whether the heuristic correctly identifies (or fails to identify) a macro lens if the phone has one, and report back so the two thresholds in `src/camera/macro_lens.rs` can be tuned if needed. If a macro lens is detected: open Settings, confirm the "Macro lens" row appears, toggle it, confirm the preview visibly switches lenses (brief Loading flash) and the zoom/torch level you had before the toggle is still applied afterward. If no macro lens exists on the test phone: confirm the row does NOT appear (rather than appearing and non-functionally doing nothing).

- [ ] **Step 11: Commit** — `git add -A && git commit -m "feat: macro lens toggle in settings, live lens switching"`

---

## Verification summary

| Task | Gate |
|---|---|
| 1 | `cargo test macro_lens` — 5 pass |
| 2 | `cargo test settings` — all pass, including the new forward-compat test |
| 3 | `dx build --android` compiles with the new multi-camera enumeration |
| 4 | (compiles together with Task 5) |
| 5 | Full `cargo test` passes; on-device: heuristic logged and (if applicable) toggle visibly switches lenses with zoom/torch carried over |
