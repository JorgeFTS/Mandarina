# Manual testing

A checklist to try Mandarina in the real window before publishing a version. The automatic tests (`cargo test`) cover the logic; this covers what is only seen with the app open. Related: [[Wheel]], [[Tools]], [[Settings]], [[Languages]], [[Nautilus]], [[Publishing]]. On macOS, use the checklist in [[macOS]].

Tick each box as you try it and note below what happened if something fails. Boxes marked **(already seen)** have evidence from an earlier session.

## 0. Preparation

- [ ] `cd src-tauri && cargo test` passes and `npm run tauri build -- --debug --no-bundle` builds without warnings.
- [ ] `bash packaging/install-nautilus.sh` and then `nautilus -q`. In Nautilus, right click → Scripts → "Convert with Mandarina" (or the name for your language) and the **Alt+M** shortcut show up.
- [ ] Linux Mint only: `bash packaging/install-nemo.sh` and then `nemo -q`. In Nemo, right click on a file and on a folder → "Convert with Mandarina" shows up and opens the wheel; try a name with spaces and several selected files.
- [ ] There are test files in a local `test-files/` folder (image, audio, video, pdf, docx, csv, srt, a folder). That folder is not in the repository (it is in `.gitignore`); create your own.
- To see errors that the interface does not show, run the app from a terminal: `GDK_BACKEND=x11 src-tauri/target/debug/mandarina "path/file"`. There is no debug log in `/tmp` anymore.

## 1. Opening and closing the wheel (Nautilus flow)

- [ ] Alt+M on a file opens the wheel **next to the cursor**, with the thumbnail and the size in the center.
- [ ] A click **outside** the circle closes it.
- [ ] **Escape** closes it; on the tools or options wheel it only goes back one level.
- [ ] Without moving the mouse or typing, it closes by itself after ~4 s.
- [ ] Switching to another window (losing focus) closes it.
- [ ] After coming back from the **settings**, the **trim editor** or **cancelling** a conversion, and without moving the mouse, it closes by itself after ~4 s.
- [ ] The center of the disc, on a secondary wheel, goes back to the previous one.
- [ ] During a conversion it does **not** close when clicking outside or switching windows.
- [ ] "Done" closes after 1 s; with the mouse over it, it waits.
- [ ] An error or notice card stays until **Close** or Escape is pressed.
- [ ] The window does not leave the screen when the wheel is opened near an edge.

## 2. Conversions (one per type)

- [ ] image: a JPG → PNG and → PDF.
- [ ] audio: an MP3 → WAV. The bar moves with the real progress.
- [ ] video: an MP4 → WEBM; **cancel with the ✕** halfway: it goes back to the wheel and leaves no half-written file.
- [ ] document: a DOCX → PDF; an MD → DOCX.
- [ ] pdf: a multi-page PDF → JPG creates a `name_pages` folder; a one-page one creates a single file.
- [ ] spreadsheet: an XLSX → CSV. Subtitles: an SRT → VTT.
- [ ] folder → ZIP (the zip carries the folder name inside).
- [ ] Repeating a conversion gives `name 2.ext`, it never overwrites.
- [ ] A file of an unsupported type (for example `.xyz`): an "Unsupported format" notice.

## 3. Several files

- [ ] 3 images → PDF: "3 files converted" and 3 PDFs.
- [ ] Image + audio: it only offers **ZIP**.
- [ ] Several files of mixed types → ZIP: a single `Archive.zip` with all of them.
- [ ] Several files with one already in the chosen format: "1 skipped" shows up.

## 4. Tools (one file)

- [ ] **Reduce size** on a large JPG and on a PDF; the new file weighs less.
- [ ] Reducing the size of the same file twice: a soft "Already optimized" notice.
- [ ] It does not offer reduce size on WAV, FLAC or BMP.
- [ ] Metadata, rotate, flip and resize on an image; the measurements of the result are the expected ones.
- [ ] No audio, rotate and resize on a video.
- [ ] **Trim** an image, an audio and a video with the visual editor. With a **custom name**: the progress card shows the new name, an empty name or one with `/` leaves Apply disabled, and typing the extension does not duplicate it.
- [ ] **ZIP** of a single file (for example `photo.jpg`): it creates `photo.zip` next to the original.
- [ ] On a document (docx, csv...) only ZIP shows up as a tool.

## 5. Settings

- [ ] The gear is **not visible** until the mouse is over the center circle, and it disappears when it leaves. It does not cover the size pill.
- [ ] When opened, the window grows to 600x660 and when closed it goes back to the wheel in the same place.
- [ ] Unchecking a format removes it from the wheel; with **3** checked the checked boxes are blocked; with **8** the unchecked ones.
- [ ] Removing a format and checking it again works without using "Restore". "Restore" goes back to the usual list.
- [ ] The accent color changes instantly across the whole app, and also when the app is opened again.
- [ ] Changing the language instantly repaints the settings screen, the wheel and the center text (decimal comma in French, Spanish and Portuguese).
- [ ] The settings are saved in `~/.config/mandarina/config.json` and are still there when the app is reopened **(already seen: the file had audio without AIFF, `es` and `pink`)**.
- [ ] Keyboard: Tab to the gear and Enter opens it **without closing the app**.
- [ ] With `MANDARINA_LANG=fr` defined in the environment, it wins over what was chosen in the settings (expected).
- [ ] A system in a language without a translation (for example `LANG=zh_CN.UTF-8`) shows the app in English.

## 6. Languages

- [ ] Each language (`en`, `es`, `pt`, `fr`) shows the wheel, the cards and the errors translated; no text stays in another language and no untranslated key (like `settings.x`) shows up.
- [ ] The long texts fit in the segments of the tools wheel: "RÉDUIRE LA TAILLE", "REDIMENSIONNER", "REDUZIR TAMANHO".
- [ ] The output names change with the language: `photo (compressé).jpg` in French.
- [ ] After changing the language, running `install-nautilus.sh` again renames the menu entry.
- [ ] A native speaker reviews the Portuguese and French (pending).

## 7. Documents with formatting

- [ ] A `.md` with images and HTML tables → DOCX, **with its `assets/` folder next to it**: tables and images present, A4 page, centered cover and page breaks.
- [ ] The same `.md` without `assets/`: it comes out with the alternative text of the images (expected).
- [ ] docx → HTML opens in the browser with the accents right and with its images in `name_media/`.
- [ ] A docx of about 200 pages converts to PDF without the wheel looking frozen.
- [ ] A long PDF → DOCX: a poor and slow result is expected (see [[Converters]]); it must not freeze the wheel.

## 8. The `.deb` package and the release build

- [ ] `npm run tauri build -- --bundles deb` finishes and the `.deb` weighs a few MB.
- [ ] Install it on a clean system (`sudo apt install ./Mandarina_*.deb`); `ffmpeg`, `pandoc`, `libreoffice` and the rest arrive as recommended packages.
- [ ] `mandarina /path/file` from a terminal opens the wheel with the release binary.
- [ ] Running it from the applications menu (no arguments): it opens the circle to drag a file onto.
- [ ] On **Wayland** the wheel appears next to the cursor with the release binary, without `GDK_BACKEND=x11`.
- [ ] Uninstalling (`sudo apt remove mandarina`) leaves nothing of the app behind.
- [ ] With a tool missing (for example, renaming `ffmpeg` for a moment), the error says what to install.

## If something fails

Note: what you did, what you expected, what happened, whether it happened with the normal window or from the terminal, and the terminal message if there is one. A screenshot helps a lot.
