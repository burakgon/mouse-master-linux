#!/usr/bin/env bash
# Records docs/demo.gif: the app on simulated devices while a scripted
# "showcase" presses buttons and draws gestures. Needs a graphical session and
# ffmpeg. Stops the installed service for the duration of the recording.
set -euo pipefail
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRAMES="$(mktemp -d)"
trap 'rm -rf "$FRAMES"; pkill -TERM -f "mouse-masterd --demo" || true; systemctl --user start mouse-master.service 2>/dev/null || true' EXIT

cargo build --manifest-path "$SRC/Cargo.toml" -p mouse-masterd
cmake --build "$SRC/build" --target mouse-master

systemctl --user stop mouse-master.service 2>/dev/null || true
"$SRC/target/debug/mouse-masterd" --demo --showcase --config "$FRAMES/config.toml" &
sleep 2
MM_RECORD="$FRAMES" MM_RECORD_FPS=20 MM_RECORD_SECONDS=14 MM_SCREENSHOT_DELAY=500 \
    MM_PAGE=buttons MM_CONTROL=gesture MM_THEME=dark "$SRC/build/app/mouse-master"

ffmpeg -loglevel error -y -framerate 20 -i "$FRAMES/frame_%04d.jpg" \
    -vf "fps=15,scale=1100:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=192:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
    "$SRC/docs/demo.gif"
echo "wrote docs/demo.gif"
