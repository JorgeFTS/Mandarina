# Publishing

Notes on packaging and on preparing the repository for GitHub. Related: [[Nautilus]], [[Decisions]], [[Home]], [[Manual-Testing]].

## Repository

- Published at https://github.com/JorgeFTS/Mandarina (branch `main`, first commit `b8e1f48`; `repository` is set in `Cargo.toml`). Commits carry no co-author lines. `.gitignore` ignores `node_modules/`, `src-tauri/target/` and `src-tauri/gen/` (Tauri regenerates `gen/schemas` when building; `Cargo.lock` is versioned) and `.claude/`. It also ignores `test-files/` (personal test files, 170 MB), `*.deb`/`*.rpm`/`*.AppImage`, `.env*` and, by the author's decision, `CLAUDE.md` and `SKILLS.md` (notes for working with Claude Code; they stay on disk but are not published).
- **MIT** license (`LICENSE`, with "Copyright (c) 2026 JorgeFTS", the git username; it can be changed to a full name; also in `Cargo.toml` and `package.json`).
- Pending for the author: a real author in `authors` (now "Autores de Mandarina"), and whether the name "Mandarina" (the Spanish translation of "Tangerine") can be confused with the Leap Studio brand.
- The temporary debug log (`/tmp/mandarina-debug.log`) has been removed.
- The READMEs are `README.md` (English, the one GitHub shows) and `README.es.md` (Spanish). See [[Languages]].
- Package metadata (`Cargo.toml` `[package]` and `bundle` in `tauri.conf.json`) is in English. The `.desktop` of the `.deb` is also English only (Tauri does not localize the name or the comment without its own `desktopTemplate`; this has not been done). `ghostscript` was added to `Recommends` because reducing the size of a PDF needs it.

## Icon

`packaging/icon/mandarina.svg` (made by hand) → 1024x1024 `mandarina.png` with `rsvg-convert` → `npx tauri icon packaging/icon/mandarina.png`. The `icons/android` and `icons/ios` folders that the command generates were deleted, because they are not used. To change the icon, repeat those steps.

## Release build and .deb

Rebuilt on 2026-10-18 with the current code (languages, settings and `assets/` included) in the normal `src-tauri/target/`; the measurements of the first table are from the first test:

| Step | Result |
|---|---|
| `npm run tauri build -- --no-bundle` | OK, 1 min 52 s from scratch (LTO). 6.2 MB binary (the debug build weighs ~250 MB). |
| `npm run tauri build -- --bundles deb` | OK, 1 min 07 s. `Mandarina_0.1.0_amd64.deb` of 2.4 MB (2.57 MiB, package name `mandarina`, with the current code). |
| Release binary startup (`medir.sh`) | 1st cold run ~2.9 s until the wheel is ready; next ones ~0.9 to 1.1 s. Full-screen window at ~0.5 s. |

The `.deb` (path `src-tauri/target/release/bundle/deb/`) contains `usr/bin/mandarina` (6.5 MB), `usr/share/applications/Mandarina.desktop` (category Utility, `Exec=mandarina`) and hicolor icons (32, 128 and 256@2). It **does not install** the Nautilus script or the shortcut: `packaging/install-nautilus.sh` is still manual, and it can be given `/usr/bin/mandarina`. A post-install script could be added, but it would touch the `~/.local` of every user and it is not done.

### Dependencies

Configured in `tauri.conf.json` (`bundle.linux.deb`):

- `Depends: libwebkit2gtk-4.1-0, libgtk-3-0`. Tauri already adds them by itself; if they are repeated in `depends` they come out duplicated in the `control` file, so the list is left empty there.
- `Recommends: ffmpeg, imagemagick, librsvg2-bin, pandoc, libreoffice, poppler-utils, ghostscript, zip, unzip, p7zip-full`. They are recommended and not required because each format uses its own tool and the app tells which one is missing; `libreoffice` is very heavy. `apt` installs the recommended ones by default.

`targets` is `["deb"]` (with `"all"` rpm and AppImage are also attempted, untested).

## Untested

- Installing the `.deb` on a clean system (it was not installed on this machine) and running the app from the applications menu.
- AppImage, rpm, Flatpak and Snap.
- The `.desktop` launches the app without arguments (drag mode), without `GDK_BACKEND=x11`; on Wayland it may not be placed next to the cursor.
- There are `dead_code` warnings when compiling, coming from Rust code under development.
