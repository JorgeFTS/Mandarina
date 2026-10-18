# Languages

Mandarina is available in **English (default), Spanish, Portuguese and French**. Everything the user sees goes in the languages of the app. English and Spanish are written in the code; the others are **data** (catalogs), so adding a language does not touch the logic.

## How the language is chosen

It is decided at startup and **decided again when it is changed in the settings**, without restarting, in this order: `MANDARINA_LANG` from the environment (to force it), the language saved in the settings (`language` in `config.json`, see [[Settings]]), `LC_ALL`, `LC_MESSAGES`, `LANGUAGE` (the first language of the list) and `LANG`. If the value starts with the code of a language (`es`, `pt`, `fr`; e.g. `pt_BR.UTF-8`) that one is used; **in any other case, or if there is none → English** (a system in Chinese or German sees the app in English). In the settings there is **Automatic** (follows the system) and one button per language, with its name in its own language.

- Rust: `src-tauri/src/lang.rs`. `Lang { En, Es, Pt, Fr }`, `LANGUAGES` (code, own name and variant: detection, settings and validation all come from here), `lang()` (computed on first use and kept in a `Mutex`), `reload()` (decides it again; `set_language` calls it; it does nothing in tests), `set_lang(Lang)` (tests only), `tr("English", "Español")` and `tr!(...)` with formatting. `get_language() -> String` returns the code (`"en"`, `"es"`, `"pt"`, `"fr"`).
- **Backend catalogs** (`src-tauri/locales/<code>.json`, loaded with `include_str!`): `{ "English text": "translation" }`. The key is the English text exactly as it is in the code, with its `{placeholders}`; the translation carries the same ones. A missing text comes out in English. `tr` looks there when the language is neither English nor Spanish. `tr!` takes each value with a name (`tr!("Cannot convert {name} to {format}", "...", name = n, format = f)`) because languages with a catalog cannot use `format!` (their text is not known at compile time) and fill the placeholders with `lang::fill`.
- JS: `src/i18n.js` defines `window.I18n` with `init(lang)` (a code that does not exist falls back to English), `t(key, variables)` (`{name}` placeholders; if a key is missing in a language English is used, and if it is missing altogether, the key itself), `plural` and `lang`. The extra languages are in `src/locales/<code>.js` and are registered in `window.IdiomasExtra` before `i18n.js` is loaded. `main.js` calls `get_language` before painting anything and again when the language is changed in the settings (`cambiarIdioma`: repaints the fixed texts, the center text and the wheel); in demo mode `?lang=<code>` is used (English by default, not `navigator.language`).

## What is translated

| Where | What |
|---|---|
| Interface (`index.html`, `main.js`, `editor.js`, `ajustes.js`) | all the texts, `aria-label`, notices, cards, progress titles, labels of tools and options **by `id`** (the JS ignores the `label` that `get_tools` sends; the backend keeps sending them for compatibility) |
| Backend error messages | every error that reaches the interface comes out in the active language |
| Output names of the tools | English `(compressed)`, `(no metadata)`, `(trimmed)`, `(rotated)`, `(flipped)`, `(resized)`, `(no audio)`; Spanish `(comprimido)`, `(sin metadatos)`, `(recortado)`, `(girado)`, `(volteado)`, `(redimensionado)`, `(sin audio)`; Portuguese `(comprimido)`, `(sem metadados)`, `(cortado)`, `(girado)`, `(espelhado)`, `(redimensionado)`, `(sem áudio)`; French `(compressé)`, `(sans métadonnées)`, `(coupé)`, `(pivoté)`, `(retourné)`, `(redimensionné)`, `(sans audio)` |
| Initial name in the trim editor | `name (trimmed)` / `(recortado)` / `(cortado)` / `(coupé)` |
| Nautilus menu | `Convert with Mandarina` / `Convertir con Mandarina` / `Converter com Mandarina` / `Convertir avec Mandarina`. The installer (`packaging/install-nautilus.sh`) chooses by language (with the same rule, including the language saved in the settings) and deletes the ones of the other languages. It is **not updated by itself** when the language is changed in the settings: the installer has to be run again. Its own messages come out in English or Spanish |
| README | `README.md` in English (the one GitHub shows), `README.es.md` in Spanish (there is none in Portuguese or French) |
| Package metadata (`Cargo.toml`, `tauri.conf.json`) | in English |

Not translated: the messages of `ffmpeg`, ImageMagick and the other programs (passed through as they are).

## Rules

- The English texts come first and are the reference; Spanish and the others must say the same.
- No visible text is written directly in the code outside the translation tables.
- The Rust tests use **English** (the default language) and there are specific tests for the others.
- **Catalog tests** (`lang.rs`): `los_catalogos_tienen_todos_los_textos` walks the source code looking for every `tr`/`tr!` text and fails if a catalog lacks any of them, or has one that no longer exists; `las_traducciones_llevan_los_mismos_marcadores` checks that each translation keeps its `{placeholders}`. There are no automatic tests in JS: the keys of each `locales/*.js` must be those of `en` (156 keys).
- The Portuguese (Brazil) and French translations were written by the assistant, **without a native speaker's review**: they should be reviewed, above all the technical terms (metadata, trim, flip).
- Long texts: `RÉDUIRE LA TAILLE` and `REDIMENSIONNER` fit in the segments of the tools wheel, but tightly. With a new language the tools wheel has to be checked (`?demo=herramientas&tipo=video&lang=<code>`).

## Adding a language

Summarized steps (the detailed ones are in `CONTRIBUTING.md`): `src/locales/<code>.js` (+ its `<script>` in `index.html`), `src-tauri/locales/<code>.json`, a variant in `Lang` plus a line in `LANGUAGES` and another in `CATALOGS` of `lang.rs`, and the menu name in `install-nautilus.sh`. The settings show it by themselves.

## Changing the language live

When a language is clicked in the settings: `set_language` writes `config.json` and calls `lang::reload()` (the backend messages and output names already come out in the new language); `ajustes.js` calls `alCambiarIdioma` (`cambiarIdioma` of `main.js`: asks `get_language`, `I18n.init`, repaints `document.title`, `lang`, the `data-i18n` nodes and the gear) and is drawn again; when the settings are closed, `abrirAjustes` recomputes the center text (`textoDeCentro`, which depends on the language because of the decimal comma) and draws the wheel again. If `MANDARINA_LANG` is defined in the environment it wins and changing the language in the settings is not noticed. Cards that are already shown (progress, errors) are not repainted; they are short-lived. Neither is the Nautilus menu: the installer has to be run again.
