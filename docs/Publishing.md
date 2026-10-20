# Publishing

Notes on packaging and on preparing the repository for GitHub. Related: [[Nautilus]], [[Decisions]], [[Home]], [[Manual-Testing]].

## Repository

- Published at https://github.com/JorgeFTS/Mandarina (branch `main`, first commit `b8e1f48`; `repository` is set in `Cargo.toml`). Commits carry no co-author lines. `.gitignore` ignores `node_modules/`, `src-tauri/target/` and `src-tauri/gen/` (Tauri regenerates `gen/schemas` when building; `Cargo.lock` is versioned) and `.claude/`. It also ignores `test-files/` (personal test files, 170 MB), `*.deb`/`*.rpm`/`*.AppImage`, `.env*` and, by the author's decision, `CLAUDE.md` and `SKILLS.md` (notes for working with Claude Code; they stay on disk but are not published).
- **MIT** license (`LICENSE`, with "Copyright (c) 2026 JorgeFTS", the git username; it can be changed to a full name; also in `Cargo.toml` and `package.json`).
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

The `.deb` (path `src-tauri/target/release/bundle/deb/`) contains `usr/bin/mandarina` (6.5 MB), the Nautilus installer in `usr/share/mandarina/` (`install-nautilus.sh` and `nautilus/mandarina-script`, configured in `bundle.linux.deb.files` of `tauri.conf.json`; the installer finds the installed program by itself when it is not inside a repository), `usr/share/applications/Mandarina.desktop` (category Utility, `Exec=mandarina`) and hicolor icons (32, 128 and 256@2). It **does not install** the Nautilus script or the shortcut by itself: the user runs `bash /usr/share/mandarina/install-nautilus.sh` (a post-install script would touch the `~/.local` of every user and is not done).

### Dependencies

Configured in `tauri.conf.json` (`bundle.linux.deb`):

- `Depends: libwebkit2gtk-4.1-0, libgtk-3-0`. Tauri already adds them by itself; if they are repeated in `depends` they come out duplicated in the `control` file, so the list is left empty there.
- `Recommends: ffmpeg, imagemagick, librsvg2-bin, pandoc, libreoffice, poppler-utils, ghostscript, zip, unzip, 7zip | p7zip-full` (`p7zip-full` no longer exists on Ubuntu 26.04, where `7zip` replaced it; on 24.04 and Debian 13 it is only a transitional package that installs `7zip`; `7zip` exists on Ubuntu 22.04 to 26.04 and Debian 12 and 13, and gives the app a `7z` or `7zz` binary, both of which it looks for). They are recommended and not required because each format uses its own tool and the app tells which one is missing; `libreoffice` is very heavy. `apt` installs the recommended ones by default.

`targets` is `["deb"]` (with `"all"` rpm and AppImage are also attempted, untested).

### Release

The release `v0.1.0-beta` exists as a private **draft** on GitHub with the `.deb` and `SHA256SUMS` (rebuilt after each fix with `gh release upload v0.1.0-beta --clobber`); it is not published yet. When it is, the READMEs already point to its download URL (`releases/download/v0.1.0-beta/`), so push them only after publishing, or the link gives a 404.

### Other distributions

The `.deb` only serves Debian, Ubuntu and derivatives. Planned, in this order: a `PKGBUILD` for Arch, CachyOS and Manjaro (with `sudo pacman -S` in the missing-tool messages), then an AppImage, which also covers Fedora and openSUSE. Dolphin, Nemo and Thunar integration is left until someone asks. Nothing of this is done.

## Untested

- Running the app from the applications menu of a real desktop (the `.deb` was installed on a real Ubuntu one and tried in clean containers, see below).
- AppImage, rpm, Flatpak and Snap.
- The `.desktop` launches the app without arguments (drag mode), without `GDK_BACKEND=x11`; on Wayland it may not be placed next to the cursor.
- There are `dead_code` warnings when compiling, coming from Rust code under development.

## Clean-install test of the `.deb` (2026-10-18)

The `.deb` built from the code of that day was installed in clean Docker containers of **Ubuntu 22.04, 24.04 and 26.04 and Debian 12 and 13**. In each one: `apt install ./mandarina.deb` with and without the recommended packages, no missing mandatory library, the files in place and a `.desktop` that points to `mandarina`; then the app was started in a virtual screen (Xvfb), the wheel was opened on a photo, the PNG segment was clicked with `xdotool`, the converted file was created and the app closed by itself after "Done"; finally `apt remove` left nothing in `/usr`. All five passed. Findings: `p7zip-full` does not exist on Ubuntu 26.04 (fixed with `7zip | p7zip-full`, and the app's message now says `sudo apt install 7zip`); `ldd` warns about optional (weak) glibc versions on Debian 12 and Ubuntu 22.04, because the binary is built on a newer system, but it runs.

Installing the recommended packages in an empty system downloads about **620 MB (24.04) to 715 MB (26.04)**, 640 to 750 packages, because `libreoffice` is a recommended package; without the recommended ones it is about 130 to 145 MB (the WebKitGTK stack, which a desktop already has). The test does **not** cover a real desktop: GNOME, the Nautilus script and the Alt+M shortcut, Wayland placement of the wheel, GPU rendering and window transparency (the virtual screen has no compositor, so the background is black). That is what [[Manual-Testing]] is for.

## Continuous integration

`.github/workflows/tests.yml` runs `cargo test` on every push to `main` and on every pull request (Ubuntu 24.04). It installs the WebKitGTK libraries Tauri needs and all the conversion tools (ffmpeg, ImageMagick, pandoc, poppler, ghostscript, zip, 7z and LibreOffice), because the integration tests skip themselves when a tool is missing and would otherwise pass without testing anything. It does not build the `.deb` nor open a window; that stays in [[Manual-Testing]].
