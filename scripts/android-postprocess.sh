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
