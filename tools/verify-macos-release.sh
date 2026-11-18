#!/usr/bin/env bash
# Run on macOS after the signed Tauri build and before publishing any artifacts.
set -euo pipefail

bundle_dir=${1:?Usage: bash tools/verify-macos-release.sh PATH_TO_BUNDLE_DIR}
app_name=OmniScreenVeil.app
app="$bundle_dir/macos/$app_name"
archive="$app.tar.gz"
shopt -s nullglob
dmgs=("$bundle_dir"/dmg/*.dmg)
if [[ ! -d "$app" || ! -s "$archive" || ! -s "$archive.sig" || ${#dmgs[@]} != 1 ]]; then
    echo 'Expected a macOS app, signed updater archive and exactly one DMG.' >&2
    exit 1
fi
dmg=${dmgs[0]}

# Check what users get through updates, not just the app in the build tree.
xcrun stapler validate "$app"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
tar -xzf "$archive" -C "$stage"
codesign --verify --deep --strict --verbose=2 "$stage/$app_name"
spctl --assess --type execute --verbose=4 "$stage/$app_name"

# Tauri notarizes/staples the .app, but only signs the disk image.
codesign --verify --verbose=2 "$dmg"
xcrun notarytool submit "$dmg" --wait --timeout 30m \
    --apple-id "${APPLE_ID:?Set APPLE_ID}" \
    --password "${APPLE_PASSWORD:?Set APPLE_PASSWORD}" \
    --team-id "${APPLE_TEAM_ID:?Set APPLE_TEAM_ID}"
xcrun stapler staple "$dmg"
xcrun stapler validate "$dmg"
spctl --assess --type open --context context:primary-signature --verbose=4 "$dmg"
