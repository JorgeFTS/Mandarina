# macOS

How Mandarina runs on macOS: what is different from Linux, how it is packaged, and how to test it. Related: [[Nautilus]], [[Publishing]], [[Manual-Testing]], [[Decisions]].

The Linux code paths are unchanged. Everything macOS-specific is behind `#[cfg(target_os = "macos")]`, in `src-tauri/tauri.macos.conf.json` (merged by Tauri only when building on macOS), or in `packaging/macos/`.

## What changes on macOS

- **Wheel next to the cursor.** On Linux the window covers the monitor and waits for the first mouse event (Wayland does not expose the cursor). On macOS the backend can read it, so the new command `cursor_in_window` returns the cursor in page pixels and the page uses it directly. On Linux it returns `null` and the old flow runs. `show_wheel` covers the monitor *under the cursor* (multi-display), and `place_wheel` uses the window's real origin because macOS keeps windows below the menu bar.
- **Idle close without focus.** A Quick Action launches the app from a background service, and macOS may show the window without making it key; then the page gets no `mousemove` and the 4 s idle timer closed the wheel while the user was using it. While the wheel is open the page also polls `cursor_in_window` every 250 ms and treats cursor movement as activity. On Linux the first poll returns `null` and the polling stops.
- **No Dock icon.** The app runs with the `Accessory` activation policy, like `skipTaskbar` on Linux.
- **PATH.** Apps launched from Finder get `/usr/bin:/bin:/usr/sbin:/sbin`, so at startup the app prepends `~/.local/bin`, the `bin` of Homebrew's keg-only `ffmpeg-full`, `/opt/homebrew/bin`, `/usr/local/bin`, `/opt/local/bin` and `LibreOffice.app/Contents/MacOS` (in `/Applications` and `~/Applications`).
- **Install hints.** `need()` turns the `sudo apt install …` hints into `brew install …` (`ffmpeg` → `ffmpeg-full`, because Homebrew's `ffmpeg` is built without libvorbis and OGG would fail; `poppler-utils` → `poppler`, `librsvg2-bin` → `librsvg`, `7zip` → `sevenzip`, `libreoffice` → `--cask libreoffice`).
- **Transparent window.** Needs Tauri's `macos-private-api` feature and `app.macOSPrivateApi: true`. Both are in the shared files (`Cargo.toml`, `tauri.conf.json`) rather than macOS-only ones: tauri-build compares the `tauri` features in `[dependencies]` with the merged config and fails the build if they differ, and it does not look at `[target.'cfg(…)'.dependencies]`. Both settings only have an effect on macOS.
- **Cancel.** `kill_group` runs `kill -KILL -- -<pgid>`; macOS's BSD `kill` accepts `--` too. `run_se_cancela_y_mata_al_hijo` (a shell and its grandchild) and `cancelar_libreoffice_no_deja_procesos` (a real LibreOffice conversion) check that nothing is left running, and both pass on macOS.
- **Config** lives in `~/Library/Application Support/mandarina/config.json`, the usual macOS place (`MANDARINA_CONFIG_DIR` still wins). A `~/.config/mandarina/config.json` saved by an earlier version is copied there once, the first time the app reads its settings.

## Opening it: Finder Quick Action

`packaging/macos/Convert with Mandarina.workflow` is an Automator Quick Action for files and folders in Finder. It runs `open -n -b com.mandarina.converter --args <files>`: `open` lets macOS bring the window to the front, `-b` finds the app wherever it is installed, and `-n` starts a new wheel each time. The installer copies it to `~/Library/Services`. Optional shortcut: System Settings → Keyboard → Keyboard Shortcuts → Services → Files and Folders.

## Opening it: Open With

The bundle declares the supported extensions (`fileAssociations` in `tauri.macos.conf.json`) with rank **Alternate**, so Mandarina appears in Finder's right-click → Open With menu but never becomes the default app for a type. macOS delivers those files as an Apple Event (`RunEvent::Opened` in `lib.rs`), not as arguments. They are kept in `OPENED` (`wheel.rs`), and `initial_paths` / `show_wheel` use the arguments plus those files. If the event arrives after the page already started without files, the page gets them through the `open-files` event and opens the wheel at the cursor. If the wheel is already showing other files, the new ones replace them and the wheel moves to the cursor; during a conversion, the editor or the settings the event is ignored. Dropping files on the app icon goes the same way.

## Packaging

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm install
packaging/macos/build-dmg.sh     # → dist/Mandarina-<version>.dmg and .dmg.sha256
```

The script builds a universal app (Apple Silicon + Intel) and puts it in a DMG with `Install Mandarina.command`, `Read Me.txt`, `LICENSE.txt` and the Quick Action (in a hidden `.support/` folder). Publish the `.sha256` file next to the DMG in every release. The **macOS DMG** GitHub Actions workflow does both on every `v*` tag (see [[Publishing]]).

### Signing, quarantine and the installer

The app is ad-hoc signed (`signingIdentity: "-"` in `tauri.macos.conf.json`). Without that setting Tauri only has the linker's signature on the binary and the bundle itself is unsigned, so macOS cannot remember privacy answers ("allow access to the Desktop folder") and asks on every launch; with a full ad-hoc signature it asks once per build. It is not signed with an Apple Developer ID and not notarized. Everything downloaded through a browser gets the `com.apple.quarantine` attribute, and Gatekeeper refuses to open a quarantined app that is not notarized: depending on the macOS version it says the app is "damaged" or from an "unidentified developer".

The installer removes that attribute from the installed copy (`xattr -dr com.apple.quarantine`). That is what the user would otherwise do by hand with System Settings → Privacy & Security → Open Anyway; it does not change system settings or turn off Gatekeeper. Because the user is trusting the download, the Read Me asks them to compare the DMG with the published SHA-256 checksum first. The Read Me runs the installer with `bash <script>` because double-clicking an unsigned `.command` file is blocked the same way.

With an Apple Developer ID (99 USD/year), Tauri can sign and notarize during the build (`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`), and then the quarantine step and the `bash` instruction can go.

## Manual test checklist (macOS)

Same idea as [[Manual-Testing]]. Boxes marked **(already seen)** have evidence from an earlier session.

- [ ] `cd src-tauri && cargo test` passes on macOS. **(already seen, Apple Silicon, macOS 26, with all converters installed)**
- [ ] `packaging/macos/build-dmg.sh` builds, `lipo -archs Mandarina.app/Contents/MacOS/mandarina` says `x86_64 arm64`. **(already seen)**
- [ ] Mount the DMG, run the installer with `bash`: it installs to `/Applications`, or to `~/Applications` when `/Applications` is not writable, and adds the Quick Action. **(already seen, ~/Applications case)**
- [x] Quick Action on **one file**: the wheel opens next to the cursor and stays open while the mouse moves over it; converting writes the result next to the original. **(seen 2026-10-20)**
- [x] Quick Action with **several files** selected: the multi-file wheel opens with the right count, and converting writes all of them. **(seen 2026-10-20: the wheel showed 3 files and converted all of them)**
- [x] **Names with spaces**, quotes and accents (`Mi foto (1) ñ.png`), and a name starting with a space. **(seen: `Mi foto (1) ñ.png`, ` leading space.png`, `it's "quoted".png` and `My Folder` arrive intact)**
- [x] A **folder**: the wheel offers zip, tar and tar.gz. **(seen 2026-10-20)**
- [x] **Cancel** a long conversion from the progress card (a large video, and a big spreadsheet → PDF through LibreOffice): the card closes, no partial file is left, and `pgrep soffice` / `pgrep ffmpeg` show nothing. **(seen 2026-10-20: 4-minute 1080p video to WEBM, and a 150,000-row CSV to PDF; no partial file, no ffmpeg/soffice left)**
- [x] **Missing converter**: rename or hide one tool (e.g. run without LibreOffice): the error card names the tool and shows the `brew install …` command. **(seen 2026-10-20 with LibreOffice hidden: "soffice not found. Install it with: brew install --cask libreoffice")**
- [ ] Wheel near a screen edge and on a **second display**: it stays fully on that display. **(screen edge seen 2026-10-20; second display not tested yet)**
- [x] Clicking outside, Escape and 4 s idle still close the wheel. **(seen 2026-10-20)**
- [x] Light and dark appearance (System Settings → Appearance). **(seen 2026-10-20)**
- [ ] **Open With**: right-click a file → Open With → Mandarina opens the wheel at the cursor; Mandarina is listed but the default app for the type does not change. **(already seen: `open -a Mandarina <file>`, which sends the same Apple Event, loads the file; the previous build ignored it and stayed on the start screen)**
- [ ] **Settings migration**: with an old `~/.config/mandarina/config.json`, the first launch copies it to `~/Library/Application Support/mandarina/`, and changes in the settings panel are saved there. **(already seen, copy on first launch)**
- [ ] **Apple Silicon** and **Intel** Mac (or Rosetta: `arch -x86_64 /Applications/Mandarina.app/Contents/MacOS/mandarina <file>`). **(already seen: `cargo test --target x86_64-apple-darwin` passes under Rosetta and the universal app runs translated; not yet on a real Intel Mac)**
