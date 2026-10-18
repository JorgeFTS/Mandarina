# Contributing to Mandarina

Thanks for your interest. The project is small and the code is kept simple on purpose.

## How the code is organized

- `src/`: the interface, in plain HTML, CSS and JavaScript (no framework, no bundler). `index.html`, `styles.css`, `geometria.js`, `main.js` (wheel and cards), `editor.js` / `editor.css` (trim editors) and `i18n.js` and `locales/` (translations).
- `src-tauri/src/`: the Rust backend. `converters.rs` (conversions), `tools.rs` (compress, trim, rotate...), `wheel.rs` (window and wheel commands), `thumbnail.rs`, `lang.rs` (language and translated texts; the catalogs are in `src-tauri/locales/`) and `lib.rs` (startup and command registration).
- `packaging/`: the Nautilus script (`nautilus/mandarina-script`), the installer (`install-nautilus.sh`) and the icon.
- `docs/`: see below.

Conversions call system programs with `std::process::Command`, with separate arguments and no shell.

## About `docs/`

`docs/` is the author's personal Obsidian vault. It is written in **Spanish** and it is not translated: it is technical notes, one per module. Open the folder as a vault in Obsidian to see the graph, starting at `docs/Inicio.md`. If you change how a module behaves, please update its note in the same change (a Spanish note is fine; the maintainer can polish it). The design of the language support is in `docs/Idiomas.md`.

## Tests

```bash
cd src-tauri
cargo test
```

Integration tests skip themselves when a tool (ffmpeg, pandoc, ghostscript...) is missing. Install them all (see [Requirements](README.md#requirements)) to run the full suite. The tests use English, the default language, and there are specific tests for Spanish. Run `cargo check` as well before sending a change.

## Conventions

- Never overwrite the user's files; always pick a new name.
- Error messages are short and, when a tool is missing, include the `apt install` command.
- Every text the user can see must exist in **English and Spanish**, and in every other language that has a catalog (`cargo test` tells you what is missing in the Rust side). English is the reference; the Spanish text must say the same thing. Use neutral Spanish (no slang).
- Keep the code simple, student level.

## Adding a language

English and Spanish are written inline in the code; every other language is just data, so adding one does not touch the logic. To add one, for example German (`de`), you need these steps (Portuguese `pt` and French `fr` are done the same way, copy them as an example):

1. `src/locales/de.js`: copy `src/locales/fr.js`, change `.fr` to `.de` and translate every value (keep the `{placeholders}` as they are). Add it to the `<script>` lines in `src/index.html`, before `i18n.js`. Missing keys fall back to English.
2. `src-tauri/locales/de.json`: copy `fr.json` and translate every value. Each key is the English text exactly as it appears in the Rust code, and the translation must keep the same `{placeholders}`.
3. `src-tauri/src/lang.rs`: add `De` to `Lang`, one line to `LANGUAGES` (`("de", "Deutsch", Lang::De)`) and one line to `CATALOGS` (`include_str!("../locales/de.json")`).
4. `packaging/install-nautilus.sh`: add `de` to `LANGS` and the menu name to `script_name`.
5. Optionally a `README.de.md`, linked from the "English | Español" line at the top of the other READMEs.

`cargo test` checks the Rust side for you: it fails if `de.json` lacks a text that is in the code, has one that is not, or changes a `{placeholder}`. For the JavaScript side, the keys of `src/locales/de.js` must match the ones in `src/i18n.js` (the demo page, `index.html?demo=rueda&lang=de`, warns in the console about missing keys). The language then shows up by itself in the settings.

Long words can overflow the wheel's segments (for example, `RÉDUIRE LA TAILLE` in French is already tight), so check the tools wheel with `index.html?demo=herramientas&tipo=video&lang=de`.

## Resumen en español

Mandarina es un proyecto pequeño con código simple a propósito. La interfaz está en `src/` (HTML, CSS y JS sin framework) y el backend en `src-tauri/src/` (Rust). Las pruebas se ejecutan con `cargo test` dentro de `src-tauri/` (las de integración se saltan solas si falta una herramienta). `docs/` es la bóveda de Obsidian del autor, escrita en español. Todo texto visible debe existir en inglés y español (y en los demás idiomas con catálogo). Para añadir un idioma, sigue los pasos de la sección "Adding a language": un archivo en `src/locales/`, otro en `src-tauri/locales/`, dos líneas en `lang.rs` y el instalador de Nautilus.
