# Settings

User settings: which target formats the conversion wheel shows, the language and the accent color. Code in `src-tauri/src/config.rs`; commands in [[Tauri-Commands]]. The screen is in `src/ajustes.js` and `src/ajustes.css`.

## What is saved

`~/.config/mandarina/config.json` (or `$XDG_CONFIG_HOME/mandarina/`; on macOS `~/Library/Application Support/mandarina/`, see [[macOS]]; `MANDARINA_CONFIG_DIR` forces it, useful for tests):

```json
{ "formats": { "image": ["webp", "jpg", "png"], "audio": ["mp3", "wav", "ogg"] }, "language": "es", "accent": "blue" }
```

The list of each type is in the order in which it appears in the wheel. With no file, or with a broken or invalid file, the usual lists from [[Converters]] are used (startup never fails).

## Rules

- Configurable types: `image`, `audio`, `video`, `pdf`, `document`, `spreadsheet`. Presentations and subtitles have only 3 targets and are left alone; so are archives and folders.
- Between **3 and 8** formats per type: with fewer, once the file's own format is removed almost no segments are left; 8 is the maximum number of segments of the wheel and what audio, video and document have by default. Since the file's own format is not shown, normally 7 are left and the TOOLS or ZIP segment fits (with 8 visible and a file whose format is not in the list, e.g. aac, TOOLS does not fit, as before). `set_formats` rejects anything else with an error; when reading the file, a list that does not comply is ignored and the usual one is used. (The maximum used to be 7, and after removing one it could not be checked again without Restore.)
- Only formats that exist for that type are valid (they are lowercased and duplicates are removed).
- It is written to a temporary file and renamed, and only what could be saved stays in memory.

## Language and color

- `language`: `"auto"` (no saved value) or the code of a language in `LANGUAGES` in `lang.rs` (`en`, `es`, `pt`, `fr`; `check_language` accepts them). It is applied **immediately**: `set_language` writes the file and calls `lang::reload()`, and the interface repaints its texts (see [[Languages]]). Order of decision: `MANDARINA_LANG` from the environment, then the saved language, then `LC_ALL`/`LANG`... (`lang::detect_now` passes the saved language as if it were the value of `MANDARINA_LANG` when that variable does not exist; if `MANDARINA_LANG` is defined it wins and the change is not noticed). `saved_language()` reads the file directly, without the lock of `current()`, so that a save error (which calls `tr!`) cannot block itself. The name of the Nautilus menu entry is chosen by the installer (which also reads this setting when it runs) and is not updated by itself when the setting changes.
- `accent`: one of `ACCENTS` (`orange`, `red`, `pink`, `purple`, `blue`, `green`; the first one is the usual one). The tones are in `src/ajustes.js` (`ACENTOS`) and `aplicarAcento` changes `--naranja` and `--naranja-claro` on `:root`; all the CSS uses those variables (the editor tint is a `color-mix` with `--ed-naranja`). It is applied live and at startup (`aplicarAcentoGuardado` in `main.js`). The tool segments stay bluish gray and do not change.
- The light or dark theme follows the system and is not configurable (forcing it would mean redoing the stylesheets).

## Where it applies

- `get_targets` (one file): `shown_targets_with` = the full list from `targets_of`, filtered and ordered by the user's choice. `targets_of` is still the full list and is the one `convert` validates against, so a hidden format can still be converted from the command line or from tests.
- `get_targets_many` (several files): the intersection uses the **visible** lists of each type, and ZIP is always added (see [[Converters]]).
- In tests `config::current()` always returns the default values, so they do not depend on the machine.

## Screen

- A small **gear** at the bottom of the center circle of the conversion wheel (it does not show on the tools or options wheels). It is invisible and only shows with the mouse over the center circle (`alMoverRaton` gives it the class `visible`) or when focused with the keyboard. The content of the center moves up 18 px (`padding-bottom` of `#centro`) to leave room for it under the pill.
- `abrirAjustes()` (`main.js`) does the same as the editor: it hides the wheel, resizes the window to 600x660 with `resize_center`, opens `#vista-ajustes` with `window.Ajustes.abrir` and, when closed, puts the window back. If something changed, it asks for `get_targets_many` again and rebuilds the wheel. While the screen is open `vistaActual === "ajustes"`.
- At the top, **Language** (Automatic and one button per language from `get_prefs().languages`, each with its own name) and **Accent color** (6 circles); then the title **Wheel formats** and one section per type (`get_config`) with a checkbox chip per format, the counter "N shown" and **Restore** (`reset_formats`, disabled if it is already the usual list). Each checkbox is saved immediately with `set_formats`. At the maximum (8) the unchecked ones are blocked and at the minimum (3) the checked ones; with 8 by default one can always be removed. A backend error shows in red next to **Done**.
- When a language is chosen, the screen itself is drawn again in the new language (`alCambiarIdioma`). Escape and **Done** close it. The pure functions (`conCambio`, `puedeCambiar`) are exported for tests.
- Demo without Tauri: `?demo=rueda`, click on the center and on the gear (fake data in `CONFIG_DEMO`).

## Pending

Drag to reorder, and reaching the settings also from the start circle (with no file).
