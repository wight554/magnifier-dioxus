[Українською](README.uk.md)

# Magnifier

Android magnifier for reading small print. Tap the icon, get an immediately zoomed,
live rear-camera view; toggle the flashlight; freeze the frame to hold it steady while
reading. No menus in the critical path — everything is a tap on the overlay itself.

Built with [Dioxus](https://dioxuslabs.com) 0.7: a transparent webview renders the
control overlay, layered on top of a native `SurfaceView` fed directly by the NDK
Camera2 API (`ndk-sys`). No Kotlin/Java source, no Gradle dependencies — the whole
camera pipeline is Rust.

Design and implementation notes: `docs/superpowers/specs/2026-07-23-magnifier-design.md`
and `docs/superpowers/plans/2026-07-23-magnifier.md`.

## Install

The easiest way to install and keep this app updated is [Obtainium](https://github.com/ImranR98/Obtainium):

1. Install Obtainium from its [releases page](https://github.com/ImranR98/Obtainium/releases) or F-Droid.
2. In Obtainium, tap "Add App" and paste: `https://github.com/wight554/magnifier-dioxus`
3. Obtainium finds the latest signed release APK automatically and installs it.
4. Future updates show up in Obtainium like any other tracked app.

Alternatively, download the APK directly from the [Releases page](https://github.com/wight554/magnifier-dioxus/releases) and install it manually (you'll need to allow installs from this source in Android's settings).

## Requirements

- Android 10 (API 29) or newer, phone with a rear camera.
- Rust, `dx` CLI (`cargo install dioxus-cli`), matching the `dioxus` version in
  `Cargo.toml` (mismatched versions print a warning but usually still work).
- For Android builds: Android SDK (platform 29+, platform-tools) and NDK 27+, plus a
  JDK. Set `ANDROID_HOME`/`NDK_HOME`/`JAVA_HOME` and add
  `$ANDROID_HOME/platform-tools` to `PATH`.
- `rustup target add aarch64-linux-android`.

## Desktop dev loop

UI work only — the camera is a stub (gray box, fixed capabilities) so iteration doesn't
need a device:

```sh
dx serve --desktop
```

## Android

Debug build + install on a connected device:

```sh
dx serve --android          # builds, installs, and streams logs
# or, to drive install/launch/logs yourself:
dx build --android
adb install -r target/dx/magnifier/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.magnifier.app/dev.dioxus.main.MainActivity
```

Release APK:

```sh
dx bundle --platform android --release
```

## Releasing

Releases are built and signed automatically by GitHub Actions: pushing a tag matching
`v*` triggers `.github/workflows/release.yml`, which builds a signed release APK on a
macOS runner and publishes it to the repo's [Releases page](https://github.com/wight554/magnifier-dioxus/releases)
— which is what Obtainium tracks.

One-time setup, before the first automated release:

1. Generate the release keystore locally (if you haven't already, from Task 18):
   `./scripts/generate-release-keystore.sh`.
2. Base64-encode it and copy to the clipboard: `base64 -i release.jks | pbcopy`.
3. In the GitHub repo, go to Settings → Secrets and variables → Actions, and add three
   repository secrets:
   - `ANDROID_KEYSTORE_BASE64` — paste the base64 output from step 2.
   - `ANDROID_KEYSTORE_PASSWORD` — the keystore password chosen when generating it.
   - `ANDROID_KEY_PASSWORD` — the key password chosen when generating it.

After that one-time setup, cutting a release is just:

```sh
# bump the version in Cargo.toml, then:
git add Cargo.toml && git commit -m "chore: bump version to vX.Y.Z"
git tag vX.Y.Z && git push origin vX.Y.Z
```

CI does the rest — build, sign, and publish the APK to the Releases page.

## Testing

```sh
cargo test          # settings serde + zoom math, desktop only
```

Camera/JNI code has no automated tests (emulator cameras don't behave like real
hardware) — it's verified manually on a physical device. `adb logcat | grep magnifier`
surfaces the app's own tracing through the camera thread and permission flow.
