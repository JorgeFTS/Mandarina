# Tauri commands

Rust functions marked with `#[tauri::command]`, registered in `invoke_handler` in `src-tauri/src/lib.rs`. The JS calls them with `window.__TAURI__.core.invoke(...)` (`withGlobalTauri` is on, there is no bundler).

They delegate the work to [[Converters]] and [[Tools]], and the [[Wheel]] uses them.

## Contract

### `get_targets(path: String) -> Vec<String>`
Formats the file can be converted to, in the order they appear in the wheel, limited and ordered by the user's choice in the [[Settings]]. An empty list if the type is not supported. It excludes the file's own format. It also accepts folders: targets `zip`, `tar`, `tar.gz` (compress the folder).

### `convert(path: String, format: String) -> Result<String, String>`
Converts `path` to `format` (one of those returned by `get_targets`). The output file is created next to the original with the same name and the new extension; if it already exists, ` 2`, ` 3`... is added. It returns the output path. On error it returns a short, readable message to show in the interface (for example, the missing tool and how to install it), in the active language. It is asynchronous: it must not block the interface.

### Floating wheel commands

The app is launched in two ways: `mandarina /path/file` (from the Nautilus right click) or with no arguments (a small window onto which a file is dragged). The window is transparent, borderless and always on top; the wheel is drawn in the center and the transparent area around it closes the app when clicked.

- `initial_paths() -> Vec<String>`: every command-line argument that does not start with `-`, as absolute paths, that exists, without duplicates and in the order received. An empty list if there is none.
- `file_info(path: String) -> Result<FileInfo, String>` with `FileInfo { name: String, size: u64, is_dir: bool }` (`size` in bytes; for folders, a recursive sum).
- `show_wheel() -> ()`: shows the window (which starts hidden). If the app was launched with files, it **enlarges it by hand until it covers the primary monitor** (`set_position` + `set_size`, without `set_fullscreen`) and transparent, only so that the page receives the cursor; otherwise it centers it.
- `place_wheel(x: f64, y: f64) -> ()` (async): sets 460x460 and places the window with its center at (x, y), the cursor position in page pixels, inside the monitor. It changes the size, waits 60 ms and repeats the position three times (`resize_and_move`): without that pause the window manager ignores the move and the window stays where it was. It is async so it can sleep without blocking the main thread. See [[Wheel]].
- `center_wheel() -> ()` (async, uses `resize_and_move`): sets 460x460 and centers the window on the monitor (for the message cards).
- `resize_center(width: u32, height: u32) -> ()` (async, uses `resize_and_move`): puts the window at that **logical** size, centered on the monitor (for the editor and the settings). `center_wheel` still returns 460x460 for the cards.
- `cancel() -> ()`: cancels the conversion in progress (kills the child process and deletes the half-written output). The pending `convert` ends with the `[cancelled]` error.
- `quit() -> ()`: closes the app.
- `get_language() -> String`: the code of the active language (`"en"`, `"es"`, `"pt"`, `"fr"`; see [[Languages]]); the interface asks for it before painting anything.
- **Stable error codes:** the cancellation and "already optimized" errors arrive as `"[cancelled] <text>"` and `"[optimized] <text>"` (the text is in the active language). The interface decides by the code, never by the text.

### Real progress and thumbnail

- **`convert-progress` event** (emitted by the backend with `app.emit` during `convert`; the interface listens with `window.__TAURI__.event.listen`). Payload: `{ "percent": number }`, from 0 to 100, increasing, at most about 10 times per second. It is only emitted in ffmpeg conversions (audio, video, subtitles) when the duration is known (with `ffprobe`); in the rest (ImageMagick, LibreOffice, pandoc...) nothing is emitted and the interface keeps the simulated bar. 100 is never emitted: the end of `convert` marks it.
- `thumbnail(path: String) -> Result<String, String>`: a thumbnail about 200 px on a side as a **data URL** (`data:image/jpeg;base64,...`). Images and SVG with ImageMagick/rsvg-convert, video with an ffmpeg frame, PDF with the first page (pdftoppm; presentations, documents and spreadsheets do not: only PDF). For any other type, or if it takes more than 3 s, it returns `Err`. It is async, does not use the cancel flag, and the interface must call it without blocking the wheel: the icon is shown first and replaced by the thumbnail if it arrives.

### Several files at once

The app receives all the selected files (`mandarina path1 path2 ...`). The single-file commands (`get_targets`, `convert`, `file_info`, `thumbnail`) stay the same; these are added:

- `get_targets_many(paths: Vec<String>) -> Vec<String>`: formats for the wheel. Only the files of a supported type count (the others are ignored here and will show up as failed when converting). A format is offered if **all** those files support it according to the target list of their type (counting the format itself) and **not all** of them are already in that format. Order: that of the list of the first supported file. With a single file it returns the same as `get_targets`. If none of the files is archive-type, `zip` is always offered (see [[Converters]]). An empty list if no file is supported or there is no format in common.
- `convert_many(paths: Vec<String>, format: String) -> Result<BatchResult, String>` (async). It converts one by one, each result next to its original (same naming and page-folder rules as `convert`). Those already in that format are skipped. If a file fails, it goes on with the rest. `BatchResult { outputs: Vec<String>, skipped: Vec<String> (names), failed: Vec<{ name: String, error: String }> }`. It only gives `Err` if the list is empty or if it is cancelled (`[cancelled]`; the results already created are kept). A file of an unsupported type goes into `failed` with "Unsupported file type". With `zip`, `tar` or `tar.gz` and no archives among the files, all of them are packed into **one** archive (`pack`).
- Events during `convert_many`:
  - `convert-file` `{ "index": number, "total": number, "name": string }` when starting each file that is going to be converted (`index` from 1; `total` = files to convert, without the skipped or unsupported ones).
  - **Global** `convert-progress` `{ "percent": number }`: `(finished files + percentage of the current one / 100) / total * 100`, increasing, 99 at most, at most about 10 times per second. If the current file does not report progress, the percentage only jumps when each file ends.
- `take_warnings() -> Vec<String>`: the images that a `.md` links to and that were not found during the last `convert` or `convert_many` (empty after being read once). The interface calls it when the conversion ends and, if there are any, shows a warning card instead of "Done" (see [[Converters]]).
- The interface always uses `get_targets_many`; to convert, `convert` with 1 file and `convert_many` with more than one.

### Tools (design in [[Tools]])

Serialized types (JSON):
- `ToolInfo { id: String, label: String, options: Vec<ToolOption>, editor: Option<String> }` with `editor` = `"crop"` (a box over an image) or `"trim"` (start and end in time) or `null`. `label` is short uppercase for the wheel (e.g. `"REDUCE SIZE"`).
- `ToolOption { id: String, label: String }`.
- `MediaInfo { kind: String ("imagen" | "audio" | "video"), width: Option<u32>, height: Option<u32>, duration: Option<f64> }`. The measurements of an image are **after applying the EXIF orientation** (`-auto-orient`); the crop is applied on that oriented image.

Commands:
- `get_tools(path: String) -> Vec<ToolInfo>` (no error: an empty list if there are no tools). By type: image → `compress`, `strip_metadata`, `trim` (editor `crop`), `rotate`, `flip`, `resize`; video → `compress`, `strip_metadata`, `trim` (editor `trim`), `rotate`, `flip`, `resize`, `mute`; audio → `compress`, `strip_metadata`, `trim` (editor `trim`); pdf → `compress`; the rest (documents, spreadsheets, subtitles...) → only `zip`; folders and archives → empty. Every type with tools ends with `zip` (`run_tool` resolves it with `pack`). Labels: REDUCE SIZE, METADATA, TRIM, ROTATE, FLIP, RESIZE, NO AUDIO, ZIP (translated by language). Options: `rotate` → `left` ("90° ↺"), `right` ("90° ↻"), `180` ("180°"); `flip` → `horizontal`, `vertical`; `resize` → ids `pct:75`, `pct:50`, `pct:25` ("75%", "50%", "25%") and, for images, `w:1920`, `w:1280`, `w:800` ("1920 px"...) and, for video, `h:1080`, `h:720`, `h:480` ("1080p"...). At most 8 options.
- `run_tool(path: String, tool: String, option: Option<String>, params: Option<serde_json::Value>) -> Result<String, String>` (async; it also receives the `AppHandle` internally): runs the tool and returns the **path of the output file**. `params` only for `trim`: audio/video `{ "start": f64, "end": f64 }` in seconds (0 ≤ start < end ≤ duration); image (`crop`) `{ "x": u32, "y": u32, "w": u32, "h": u32 }` in pixels of the oriented image, inside the limits. Optionally, `params` may carry `"nombre": string`: the name of the new file **without extension** (the output keeps that of the original; if the user types it, it is removed). It is cleaned (spaces at the ends) and rejected (`Err`) if it ends up empty, contains `/` or control characters, is `.` or `..` or exceeds ~230 characters. If a file with that name already exists it is numbered (`name 2.ext`), never overwritten. Without `nombre`, the name is the usual one (`photo (trimmed).jpg`). It validates and returns a readable `Err` if something does not add up. The ffmpeg ones emit `convert-progress` `{percent}` (0 to 99) and all of them can be cancelled with `cancel` (`[cancelled]`, leaving no half-written output).
- `media_info(path: String) -> Result<MediaInfo, String>`.
- `preview_image(path: String, max_side: u32) -> Result<String, String>`: the oriented image reduced to `max_side` px as a JPEG data URL (for the trim editor). A maximum process wait of 3 s.
- `video_frame(path: String, seconds: f64, max_side: u32) -> Result<String, String>`: one frame as a JPEG data URL.
- `waveform(path: String, buckets: u32) -> Result<Vec<f32>, String>`: amplitude peaks between 0.0 and 1.0, one per bucket, to draw the waveform (from ~100 to 400 buckets).

The editors do not use `convert`. Output names of the tools: see [[Tools]].

### Contract of the visual editor (JS)

The files `src/editor.js` and `src/editor.css` do not touch `main.js`, `index.html` or `styles.css`; `main.js` loads those two files and calls:

```js
// Returns a promise: null if cancelled, or the params of run_tool if applied
// ({x,y,w,h} for an image, {start,end} for audio and video, and always `nombre`: the text of the
// "New file name" field, without extension)
window.Editor.abrir({
  tipo: "imagen" | "audio" | "video",
  ruta: "/path/file",
  nombre: "file.ext",
  contenedor: HTMLElement,   // the editor is drawn inside it and empties it when finished
  api: {                      // functions that return promises, with positional arguments
    media_info(path),
    preview_image(path, maxSide),
    video_frame(path, seconds, maxSide),
    waveform(path, buckets),
  },
}) -> Promise<object | null>
```

The container measures about 740x560 px (the window is 780x640). `Editor` draws its own **Apply** and **Cancel** buttons, supports Escape (cancel) and Enter (apply), light/dark mode and accessible keyboard use (arrows move the focused end). It does not call `run_tool` or close the window: `main.js` does that.

Output names: next to the original, `name.ext`; if it exists, `name 2.ext`, `name 3.ext`... (with a space, as on the Mac; no longer `_1`). The page folders follow `name_pages`, `name_pages 2`...

## Formats

The full table of types and targets is in [[Converters]]. For pdf, document and presentation, jpg/png with several pages generates a folder of pages and `convert` returns the path of the folder; with a single page it returns a loose file.

## Drag and drop

Tauri 2 intercepts the system drop: the HTML5 `drop` does not give the path. `getCurrentWebview().onDragDropEvent` is used (globally: `window.__TAURI__.webview.getCurrentWebview()`), which gives `paths` and the position in physical pixels (divide by `devicePixelRatio`).

## Settings (see [[Settings]])

- `get_config() -> Vec<KindFormats>`: for each configurable type (`image`, `audio`, `video`, `pdf`, `document`, `spreadsheet`) it returns `{ key, all, enabled, min, max }`: all the possible formats, the ones visible now in their order, and the limits (3 and 8).
- `set_formats(kind: String, formats: Vec<String>) -> Result<(), String>`: saves the visible formats of a type; an error if the type or a format does not exist, or if they are not between 3 and 8.
- `reset_formats(kind: String) -> Result<(), String>`: goes back to the usual list of that type.
- `get_prefs() -> { language: "auto"|code, languages: [{ code, name }], accent: String, accents: Vec<String> }`: the saved language and color, the available languages (from `LANGUAGES`) and the possible colors.
- `set_language(value: String)` (`"auto"` or a language code; applied immediately with `lang::reload`) and `set_accent(value: String)` (one of `accents`): they save the setting; an error if the value does not exist.
- Effect: `get_targets` and `get_targets_many` already return only the chosen formats.
