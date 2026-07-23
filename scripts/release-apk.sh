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

if [ -z "${STOREPASS:-}" ]; then
    read -srp "Keystore password: " STOREPASS; echo
fi
if [ -z "${KEYPASS:-}" ]; then
    read -srp "Key password: " KEYPASS; echo
fi

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
