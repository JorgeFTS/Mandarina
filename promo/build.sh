#!/usr/bin/env bash
# Rebuilds the demo video from the real interface of the app (see docs/Promo-Video.md):
#   docs/media/mandarina-demo.mp4   (H.264 + AAC)
#   docs/media/mandarina-demo.webm  (VP9 + Opus)
#   docs/images/mandarina-demo.gif  (silent, for the README)
# Needs google-chrome, ffmpeg (libx264, libvpx-vp9, libopus), python3. Usage: promo/build.sh [work-folder]
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
work="${1:-/tmp/mandarina-promo}"
fps=24; frames=696          # 29 s at 24 fps
port=8790
mkdir -p "$work/frames" "$root/docs/media" "$root/docs/images"

# A local server serves the repository, so the scene can load src/ exactly as the app does
(cd "$root" && python3 -m http.server "$port" >/dev/null 2>&1 & echo $! > "$work/server.pid")
trap 'kill "$(cat "$work/server.pid")" 2>/dev/null || true' EXIT
sleep 1

render() {   # render <frame number>: one headless screenshot of the scene at that time
  local i=$1 t n; t=$(python3 -c "print(round($i*1000/$fps))"); n=$(printf '%04d' "$i")
  [ -s "$work/frames/f_$n.png" ] && return 0
  google-chrome --headless=new --disable-gpu --hide-scrollbars --window-size=1280,720 \
    --force-device-scale-factor=1 --virtual-time-budget=3000 --user-data-dir="$work/profile-$i" \
    --screenshot="$work/frames/f_$n.png" "http://localhost:$port/promo/scene.html?demo=&lang=en&t=$t" >/dev/null 2>&1
  rm -rf "$work/profile-$i"
}
export -f render; export work port fps
seq 0 $((frames - 1)) | xargs -P "$(nproc)" -I{} bash -c 'render {}'

python3 "$root/promo/audio.py" "$work/audio.wav"
fade="volume=0.75,afade=t=out:st=27.5:d=1.5"
# the tiny noise keeps the end of the Opus track from being digital silence
ffmpeg -y -v error -framerate $fps -i "$work/frames/f_%04d.png" -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p "$work/video.mp4"
ffmpeg -y -v error -i "$work/video.mp4" -i "$work/audio.wav" -map 0:v -map 1:a -af "$fade" -c:v copy -c:a aac -b:a 160k -movflags +faststart -shortest "$root/docs/media/mandarina-demo.mp4"
ffmpeg -y -v error -framerate $fps -i "$work/frames/f_%04d.png" -c:v libvpx-vp9 -b:v 0 -crf 33 -row-mt 1 -pix_fmt yuv420p -an "$work/video.webm"
ffmpeg -y -v error -i "$work/audio.wav" -af "$fade,aeval='val(0)+0.0006*(random(0)-0.5)'" -c:a libopus -b:a 96k "$work/audio.opus"
ffmpeg -y -v error -i "$work/video.webm" -i "$work/audio.opus" -map 0:v -map 1:a -c copy -shortest "$root/docs/media/mandarina-demo.webm"
ffmpeg -y -v error -i "$root/docs/media/mandarina-demo.mp4" \
  -vf "fps=12,scale=840:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff:max_colors=200[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
  -loop 0 "$root/docs/images/mandarina-demo.gif"
ls -la "$root/docs/media" "$root/docs/images/mandarina-demo.gif"
