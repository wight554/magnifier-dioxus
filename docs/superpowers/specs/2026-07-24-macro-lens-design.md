# Macro Lens Support — Design

**Goal:** When the device has a dedicated macro camera lens (common on mid-range/budget phones as its own top-level Camera2 ID, distinct from the main wide lens), let the user opt into it from Settings for closer, sharper close-up reading than the main lens's digital zoom can achieve. Persisted like the other defaults; invisible entirely on phones without one.

**Non-goal:** Phones where the macro lens only exists as a "physical camera" hidden behind a single logical multi-camera back ID (increasingly common on flagships) are out of scope — reaching a specific physical sub-lens from the raw NDK Camera2 API is a materially harder, less-supported path than opening a distinct top-level camera ID, and not worth the complexity for this app.

## Architecture

Today, `Cam2::open_back_camera()` picks the *first* back-facing (`ACAMERA_LENS_FACING_BACK`) camera ID it finds and opens it — there's no concept of "which back camera."

This changes to:

1. Enumerate **all** camera IDs, keep every back-facing one (not just the first).
2. For each back-facing ID, read two characteristics:
   - `ACAMERA_LENS_INFO_AVAILABLE_FOCAL_LENGTHS` — a macro lens has a distinctly short focal length vs. the main lens on the same phone.
   - `ACAMERA_LENS_INFO_MINIMUM_FOCUS_DISTANCE` — expressed in diopters (1/meters); a macro lens focuses much closer than a main lens, so this value is much higher.
3. Heuristic: a back camera is flagged `is_macro` if its shortest available focal length is **≤ 4mm** *and* its minimum focus distance is **≥ 20 diopters** (≤ 5cm focus distance). Both conditions together, not either alone — a short focal length alone is common on ultra-wide lenses too, which aren't macro.
4. Log every back camera ID's raw focal length(s) and minimum focus distance at startup (`log::info!`), regardless of whether anything matched — this is the tool for tuning the two thresholds above against real hardware, since there's no official "this is a macro lens" flag in Camera2 and OEMs vary.
5. `open_back_camera(want_macro: bool) -> Result<Cam2>`: if `want_macro` and an `is_macro` camera was found, open that one; otherwise open the first non-macro back camera (today's behavior). If `want_macro` is true but no macro camera exists on this device, this silently falls back to the main camera rather than erroring — a synced/stale settings file requesting macro on a phone that doesn't have one should just work as if it didn't ask.

`CamInfo`/`CamCaps` gains `has_macro: bool` (device has one, exposed to Settings so the toggle can be conditionally rendered), separate from `use_macro` (the user's current on/off choice).

## Persisted state

`Settings` gains `use_macro: bool` (default `false`), alongside the existing `default_zoom`/`torch_on_launch`. Same load/save path, same `settings.json`.

## Switching flow

There's no live "hot swap" between lenses — switching means closing the current `Cam2` (session/device) and opening a new one, the same cost as the existing background-resume restart. So toggling `use_macro` in the Settings sheet:

1. Saves the setting (as the existing Close-button save already does).
2. Calls `cam.stop()`.
3. Sets `AppState::Loading` (reusing the exact same "Loading…" screen already shown on first launch and permission grant — no new UI state).
4. Calls `start_camera(...)` again, which spins up a fresh camera thread; that thread's `Cam2::open_back_camera(use_macro)` picks the newly-requested lens.
5. Current zoom ratio carries over, re-clamped against the *new* lens's own `max_zoom` (same clamp logic already used everywhere zoom is applied). Torch state carries over too — if the macro lens has no flash, this is a no-op exactly like `has_torch: false` already is.
6. If the reopen fails for any reason, it flows into the existing `AppState::Error` + Retry screen — no new error handling needed.

The Settings sheet closes as a natural side effect of `Overlay` (its parent) unmounting when state leaves `Active`/`Frozen` for `Loading`.

## UI

In `SettingsSheet`, a `use_macro` checkbox row, styled identically to the existing single-line `torch_on_launch` row. Rendered only when `caps.has_macro` is true — matches the app's existing "hide when unavailable" convention (the torch button already does this for `has_torch`). No visible indicator of which lens is active elsewhere in the UI (no badge, no extra chrome) — the setting itself is the only place this is surfaced, consistent with the app's minimal-UI philosophy.

## Testing

- Pure unit test for the detection heuristic (`is_macro(focal_length_mm: f32, min_focus_distance_diopters: f32) -> bool`) with a handful of representative real-world lens specs as fixtures (a typical main lens, a typical ultra-wide, a typical dedicated macro) — same style as the existing `zoom.rs` tests.
- Settings roundtrip test extended to cover `use_macro`, matching the existing `default_zoom`/`torch_on_launch` coverage.
- Everything else (enumeration, actual lens switching, whether the heuristic correctly flags this specific phone's macro camera) is JNI/NDK and device-specific — verified manually on-device, same as the rest of the camera code. The startup log line is the primary tool for that verification and for tuning the two thresholds if they misfire on a given phone.

## Open risk

The two heuristic thresholds (4mm focal length, 20 diopters minimum focus distance) are reasonable defaults inferred from typical macro lens specs across common phones, but not validated against this project's actual test device yet — first on-device run may require adjusting them based on the logged real values.
