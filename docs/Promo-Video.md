# Promo video

The demo video of the README (`docs/media/mandarina-demo.mp4`, `docs/media/mandarina-demo.webm` and the silent `docs/images/mandarina-demo.gif`) is generated from the **real interface** of the app, not drawn by hand. Related: [[Wheel]], [[Settings]], [[Manual-Testing]].

## How it works

- `promo/scene.html` copies the markup of `src/index.html` and adds a fake desktop (wallpaper, a mock Nautilus window, captions, key caps, a cursor and an end card). It loads the real `src/main.js`, `geometria.js`, `ajustes.js`, `i18n.js` and the stylesheets, in **demo mode** (`?demo=`), with the dark theme variables of the CSS unwrapped from their `prefers-color-scheme: dark` blocks.
- `promo/scene.js` is a **pure function of time**: it reads `?t=<ms>`, decides what is on screen at that instant (title, window, cursor path, hover, progress, settings, end card) and puts the real app in that state by calling its own functions (`cargarArchivos`, `resaltar`, `irANivel`, `mostrarProgreso`, `mostrarListo`, `abrirAjustes`, `cambiarIdioma`...). The hover of the wheel is not scripted: it comes from the cursor position with the real `segmentoEn`, so the highlight is always where the cursor is.
- `promo/build.sh` serves the repository with `python3 -m http.server`, takes one headless Chrome screenshot per frame (696 frames, 24 fps, 1280x720, in parallel), builds the sound track with `promo/audio.py` (soft effects synthesized by ffmpeg and a quiet pad) and encodes the files. It needs `google-chrome`, `ffmpeg` (libx264, libvpx-vp9, libopus) and `python3`. It takes a couple of minutes: `promo/build.sh [work-folder]`.
- The storyline is 29 s: title → select a file in Nautilus → Alt+M → the wheel → pick WEBP → progress and "Done" → tools of a video (reduce size, trim, rotate, zip) → settings (accent color and language, live) → end card. The times are constants at the top of the plan in `scene.js` and in `audio.py`; if one changes, change the other.

## Notes

- The file names, the progress and the conversion are demo data (`api` of the demo mode): nothing is converted. The interface, the geometry, the colors, the texts and the translations are the real ones.
- Chrome only supports the animations of the page in real time, so everything that moves (entrances, cursor, bar) is computed from `t` in the scene instead of relying on CSS animations, which are disabled in the scene (`animation: none`).
- The WebM carries Opus audio. ffmpeg prints one `Error parsing Opus packet header` when reading it; the audio decodes normally (checked: duration, volume, and that Chrome plays it), and the same happens when muxing with different options.
- If the interface changes (new labels, colors, segments), run `promo/build.sh` again and commit the three files.
