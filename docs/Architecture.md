# Architecture

How the pieces connect. Related: [[Wheel]], [[Tauri-Commands]], [[Converters]], [[Tools]], [[Settings]], [[Languages]], [[Decisions]].

Flow: the user drops a file on the [[Wheel]] (or opens it from Nautilus) → the JS calls a command from [[Tauri-Commands]] with the path and the format → Rust runs the conversion in [[Converters]] (system tools such as ffmpeg, ImageMagick, pandoc, LibreOffice) → it returns the result path or an error to the interface.

Folders:
- `src/`: `index.html`, `styles.css`, `geometria.js` (wheel geometry), `main.js` (wheel, cards and flow), `editor.js` (trim editors), `ajustes.js` (settings screen), `i18n.js` and `locales/` (translations).
- `src-tauri/src/`: `lib.rs` (startup and command registration), `converters.rs`, `tools.rs`, `wheel.rs` (window and wheel commands), `thumbnail.rs`, `lang.rs` and `config.rs`. `main.rs` only calls `lib.rs`.
- `src-tauri/locales/` and `src-tauri/assets/`: translation catalogs for the backend, and the reference document and filter used for docx files.

See [[Decisions]].
