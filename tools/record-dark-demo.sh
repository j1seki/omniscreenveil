#!/usr/bin/env bash
# Capture real app screenshots on an isolated X11 desktop; never record the
# user's display. Run `pnpm --dir apps/desktop tauri build --no-bundle` first.
set -euo pipefail
root=$(cd -- "$(dirname -- "$0")/.." && pwd)
output=${1:-"$root/docs/images/app-dark-demo.gif"}
binary=${VEIL_DEMO_BINARY:-"$root/apps/desktop/src-tauri/target/release/omniscreenveil-desktop"}

if [[ ${2:-} != --isolated ]]; then
    if [[ ! -x $binary ]]; then
        echo "Build the Linux desktop release binary first: $binary" >&2
        exit 1
    fi
    exec xvfb-run -a -s '-screen 0 1280x900x24' dbus-run-session -- bash "$0" "$output" --isolated
fi

scratch=$(mktemp -d)
pids=()
trap 'if ((${#pids[@]})); then kill "${pids[@]}" 2>/dev/null || true; wait || true; fi; rm -r -- "$scratch"' EXIT
export XDG_CONFIG_HOME="$scratch/config" XDG_DATA_HOME="$scratch/data" XDG_CACHE_HOME="$scratch/cache"
export XDG_SESSION_TYPE=x11 GDK_BACKEND=x11 GTK_THEME=Adwaita:dark LANG=C.UTF-8
export WEBKIT_DISABLE_DMABUF_RENDERER=1
if command -v xcompmgr >/dev/null; then
    xcompmgr &
    pids+=("$!")
fi
xmessage -title 'Demo notes — safe sample content' -geometry 320x170+70+380 'DEMO ONLY — no private data' &
pids+=("$!")
xclock -title 'Sample dashboard' -geometry 280x160+690+80 &
pids+=("$!")
"$binary" >"$scratch/desktop.log" 2>&1 &
pids+=("$!")
if ! windows=$(timeout 15 xdotool search --sync --onlyvisible --name '^OmniScreenVeil$'); then
    cat "$scratch/desktop.log" >&2
    exit 1
fi
window=${windows%%$'\n'*}
xdotool windowmove "$window" 25 30 windowfocus "$window" mousemove 1240 870
sleep 2

frame() {
    sleep 0.65
    import -window "$window" "$scratch/$1.png"
}

frame 01-overview
xdotool key ctrl+2
frame 02-windows
xdotool key ctrl+4
sleep 0.4
# Capture both states on the same page so navigating away doesn't discard them.
xdotool mousemove --window "$window" 560 230 click 1 mousemove 1240 870
sleep 1
xdotool key ctrl+k
sleep 0.3
xdotool type 'protect all windows'
xdotool key Return
sleep 1
xdotool mousemove --window "$window" 560 230 click 1 mousemove 1240 870
frame 05-audit
xdotool key ctrl+1
frame 03-armed
xdotool key ctrl+3
frame 04-surface
magick -delay 280 "$scratch"/0*.png -resize '1120x>' +dither -colors 128 -loop 0 -layers optimize "$output"
magick identify -format '%s: %wx%h, %T centiseconds\n' "$output"
