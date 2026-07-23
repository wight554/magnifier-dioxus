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
