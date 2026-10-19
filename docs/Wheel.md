# Wheel

Files: `src/index.html`, `src/styles.css`, `src/geometria.js` (pure geometry), `src/main.js` (logic). No framework and no bundler. It uses the commands of [[Tauri-Commands]] through `window.__TAURI__`.

The window (see `tauri.conf.json`) is transparent, borderless, always on top and **starts hidden**: the interface paints and then calls `show_wheel`.

**Where the wheel appears (flow A):** on Wayland the app cannot know where the cursor is, and under XWayland the position reported by Tauri is off, so it is done in two steps: 1) `show_wheel` **enlarges the window by hand until it covers the primary monitor** (transparent, stage with `opacity: 0`) only so that the page receives the first cursor event (`esperarCursor`, at most 600 ms; if none arrives, it uses the center of the monitor); 2) `place_wheel(x, y)` **shrinks the window to 460x460 with its center at that point** (clamped to the monitor) and the interface shows the wheel. In flow B the window is small and centered from the start.

**Why not `set_fullscreen`:** that mode is managed by the GNOME compositor, which hides the top bar and animates the transition; it looked like a flicker ("reset") every time the wheel opened. With `set_size` + `set_position` the compositor does not intervene. Note: when `set_fullscreen` was removed, the pauses between resizing and moving the window were removed too, and without them the move was ignored (the window stayed at +138+64 and did not follow the cursor or center); they were restored in `resize_and_move` (60 ms and three position attempts). Limitation: the window covers the **primary** monitor, not the one with the cursor (with several monitors the wheel would only be placed correctly on the primary one).

Since the window no longer covers the screen, a click outside does not close it: `vigilarAbandono` (which checks the view again when it fires, so it does not close in the middle of a conversion) closes the app when focus is lost (armed 1 s after placing the window), after 4 s without moving the mouse or pressing keys with the wheel open (`TIEMPO_ABANDONO`), or with Escape. Measured with the debug build (250 MB): the first run after compiling takes ~2.8 s until the wheel is ready (the binary is not in memory yet) and the next ones ~0.8 s.

**Messages go in the center:** the wheel opens next to the cursor, but the cards (progress, "Done", errors, notices) are shown in the **center of the monitor**. When going from the wheel to the card, `moverYMostrar("centro", ...)` hides the stage for a moment, moves the window with `center_wheel` and shows the card; if it is cancelled, `moverYMostrar("cursor", ...)` puts it back where it was opened (`posRueda`). An initial notice (e.g. "Unsupported format") goes straight to the center. While the window is moving (`enTransicion`) it does not close and no more clicks are accepted.

## Two ways of use

- **A. With a file** (right click in Nautilus, see [[Nautilus]]): `initial_paths` → `file_info` (each one) + `get_targets_many` → wheel. A click on a segment (or arrows + Enter) converts. Escape or a click outside the wheel closes the app.
- **B. Without a file:** a dotted circle "Drop a file here". When a file is dragged over it (`onDragDropEvent`: `enter`, `over`, `drop`, `leave`) the wheel appears and the segment under the cursor is highlighted (angle and distance, with `position / devicePixelRatio`). Dropping on a segment converts. Escape or a click outside goes back to the circle and does not close.

## Views

1. **Wheel:** from 2 to 8 segments (ring with a gap, separation and rounded corners), an icon by type and a pill with the size; when hovering a segment it turns orange and the pill shows the format.
2. **Converting card:** "Converting to MP3", name, bar and an X that calls `cancel`. With ffmpeg the bar follows the real percentage; in the other cases it does not measure real progress: it rises fast and slows down near 92% until the conversion ends (`animarBarra`); it lasts at least 2.2 s even if the file is small and, when finished, it fills and waits 0.45 s before showing "Done".
3. **Result card:** a ✓ mark, "Done" and the output name, with no buttons (the result is already in the same folder as the original); it closes by itself after 1 s, paused if the cursor is over it. Error: a red card with the message. "Cancelled" goes back to the wheel with no message.

## Details

- Light and dark mode with `prefers-color-scheme`; no `backdrop-filter`; 150 ms animation and `prefers-reduced-motion`.
- Accessible: segments with `role="button"`, `aria-label`, visible focus and keyboard.
- **Demo mode** without Tauri (open `index.html` in a browser, or `?demo`): fake data. `?demo=rueda|convirtiendo|listo|error|aviso` jumps to that view.
- To test the geometry with Node, copy `geometria.js` to `.cjs` (`package.json` has `"type": "module"`).
- **Several files:** the state is a list (`rutas`, `infos`). The wheel offers the formats that all of them support; the center shows "N files · total size", the icon of the common type (or a stack of files if the types are mixed) and the thumbnail of the first one with a numeric badge. With 1 file `convert` is used; with more, `convert_many`: the card says "Converting to MP3 · 2 of 5" (event `convert-file`) and the bar follows the global progress. Result: "N files converted" (and "M skipped"), or, with failures, a card with an orange border "K of N converted" with up to 3 errors and a Close button with no automatic close; if all fail, an error card. With no common format: "These files have no format in common". Demos: `?demo=varios|variosfallo|variosnocomun|variosrueda|variossaltados`.
- **Thumbnail:** after painting the wheel, `thumbnail(path)` is called without waiting; if a data URL arrives it replaces the icon (88 px, rounded corners, 250 ms fade) and the pill stays below. If it fails the icon stays and no error is shown. If the user changes file before it arrives, the old response is ignored.
- **Real progress:** `animarBarra` has two modes. With ffmpeg (audio, video, subtitles) and with batches, the bar starts at 0 and follows **only** the `convert-progress` events (the text says "Converting to MP3 · 42%"); if none arrives in 3 s, it switches to the simulated one. With images, documents and the rest, a simulated bar rises (fast and slowing near 92%) with no percentage in the text. It never goes backwards. It listens before calling `convert` and does `unlisten` when it ends. A fixed bug: the simulated one used to start always and got ahead of the real percentage (the bar was at 50% while the text said 0%); now the real one is not simulated.
- **Tools:** see [[Tools]]. The wheel has three levels (`conversion`, `herramientas`, `opciones`) with the same geometry; the TOOLS segment (wrench, bluish-gray tone) goes last and only with one file. The center of the disc or Escape go back one level. The tools with an editor open `#vista-editor` with `window.Editor` (`src/editor.js` and `src/editor.css`, prefix `.ed-`): the window becomes 780x640 (`resize_center`) and goes back to 460x460 when finished; while the editor is open the idle close and the close on losing focus do not apply (`vistaActual === "editor"`). Demos: `?demo=herramientas|opciones|editorimagen|editoraudio|editorvideo|herramientaejecutando` with `&tipo=imagen|audio|video|pdf`.
- Pending: file action wheels.

## Not verified in the real window

Real position next to the cursor, transparency on screen, real dragging and `cancel`. The Chrome headless screenshots only prove the design, not Tauri. See [[Manual-Testing]].

Related: [[Architecture]], [[Converters]].

## Fixed bug: Apply in the editor closed the app

The click on **Apply** (or Cancel) in the editor resolved the promise and, in that same click, the event kept bubbling up to `alHacerClic`; since `abrirEditor` had already set the view to "rueda", the click fell "outside the wheel" and called `salir()`: the app closed before trimming. It was fixed by ignoring in `alHacerClic` the clicks inside `#vista-editor` and waiting 80 ms before changing the view. It was found with a temporary debug log that has since been removed.

## Languages

See [[Languages]]. `src/i18n.js` (`window.I18n`, 156 keys in `en` and `es`, more languages in `src/locales/`) is loaded before `main.js` and `editor.js`; `iniciar()` asks for `get_language` before painting. In demo mode, `?lang=<code>` (English by default). The labels of tools and options are translated by id. Errors are recognized by code (`[cancelled]`, `[optimized]`), not by text. Another demo: `?demo=optimizado`.

- **Settings:** a gear, hidden at the bottom of the center circle (only visible with the mouse over it) that opens the screen for the wheel formats (`#vista-ajustes`, `src/ajustes.js`). See [[Settings]]. A click on the gear is not handled by `alHacerClic` (with the keyboard it arrives with coordinates 0,0 and would be taken as "outside the wheel" and close the app).
- **When the wheel closes (flow A, the Nautilus one; `modoLibre` never closes the app, it goes back to the circle):** a click outside the circle, Escape (on the tools and options wheels it only goes back one level), loss of focus (armed 1 s after opening) and 4 s without moving the mouse or pressing keys (`vigilarAbandono`); the last three only while the view is the wheel and there is no transition. It **does not close** during a conversion (it is cancelled with the ✕), nor with the editor or the settings open, and the error and notice cards wait until Close or Escape is pressed (the window is `alwaysOnTop` and has no taskbar entry). "Done" closes after 1 s unless the mouse is over it. `mostrarRueda` arms the idle close again (`rearmarAbandono`) when coming back from the settings, the editor or a cancel.
