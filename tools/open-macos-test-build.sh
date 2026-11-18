#!/usr/bin/env bash
# Temporary workaround for a trusted, pre-notarization OmniScreenVeil build.
set -euo pipefail

if [[ $(uname -s) != Darwin ]]; then
    echo 'Run this script on the Mac where OmniScreenVeil is installed.' >&2
    exit 1
fi

app=${1:-/Applications/OmniScreenVeil.app}
app=${app%/}
[[ "$app" = /* ]] || app="$PWD/$app"
if [[ $# -gt 1 || ${app##*/} != OmniScreenVeil.app || ! -d "$app" || -L "$app" ]]; then
    echo 'Usage: bash tools/open-macos-test-build.sh [/path/to/OmniScreenVeil.app]' >&2
    echo 'Copy OmniScreenVeil from the DMG into Applications first.' >&2
    exit 1
fi

bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist")
if [[ "$bundle_id" != io.github.j1seki.omniscreenveil ]]; then
    echo 'Unexpected bundle identifier; nothing changed.' >&2
    exit 1
fi

# This catches broken signatures, but an ad-hoc signature does not prove origin.
if ! /usr/bin/codesign --verify --deep --strict "$app"; then
    echo 'The app failed signature verification; nothing changed. Get a fresh build.' >&2
    exit 1
fi

if /usr/bin/xattr -p com.apple.quarantine "$app" >/dev/null 2>&1; then
    printf 'Remove download quarantine from this copy only?\n  %s\n' "$app"
    printf 'Continue only if you built it yourself or verified it came from a trusted source. [y/N] '
    read -r answer || exit 1
    case "$answer" in
        y|Y|yes|YES) /usr/bin/xattr -d com.apple.quarantine "$app" ;;
        *) echo 'Cancelled; nothing changed.'; exit 1 ;;
    esac
fi

/usr/bin/open "$app"
