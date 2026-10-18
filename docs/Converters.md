# Converters

File: `src-tauri/src/converters.rs` (a single file). It is a Rust port of the converters of the earlier Python version of the project. The commands it exposes are in [[Tauri-Commands]].

## Types and targets (wheel order, without the file's own format)

| Type | Targets | Tool |
|---|---|---|
| image | jpg, png, webp, tiff, avif, bmp, pdf | ImageMagick, rsvg-convert (SVG) |
| audio | mp3, m4a, wav, flac, ogg, opus, aiff, wma | ffmpeg |
| video | mp4, mov, mkv, webm, avi, wmv, gif, mp3 | ffmpeg |
| pdf | docx, jpg, png, epub, txt, md | pdftoppm, pdftotext, pandoc, LibreOffice |
| document | docx, md, txt, epub, html, pdf, jpg, png | pandoc, LibreOffice |
| presentation | pdf, jpg, png | LibreOffice |
| spreadsheet | xlsx, ods, csv, tsv, pdf | LibreOffice |
| subtitle | srt, vtt, txt | ffmpeg, Rust (txt) |
| archive | zip, tar, tar.gz (rar is input only) | zip, tar, unzip, 7z |
| folder | zip, tar, tar.gz (compress the folder) | zip, tar |

Aliases: `jpeg`→`jpg`, `tif`→`tiff`, `heif`→`heic`, `htm`→`html`, `tgz`→`tar.gz`. HEIC is input only (the local ImageMagick does not write it).

## How it works

- `category` and `targets_of` decide the type and the targets; `convert_blocking` validates, computes the output with `unique_path` (it never overwrites: `name 2.ext`, `name 3.ext`...) and dispatches to `convert_image`, `convert_ffmpeg`, `convert_pdf`, `convert_document`, `convert_slides`, `convert_spreadsheet`, `convert_subtitle` or `convert_archive`.
- Everything uses `std::process::Command`, with no shell; absolute path, and `--` wherever the tool supports it (a file `-x.pdf` is not read as an option).
- `TempDir` is a temporary folder that deletes itself, also on error. LibreOffice uses its own profile (`-env:UserInstallation`) so it does not clash with an instance that is already open.
- `tool_exists` searches the PATH without running anything.
- If a conversion fails or is cancelled, the half-written output is deleted.
- **Cancel:** `run_in` uses `spawn` and a `try_wait` loop every 50 ms that looks at the global `AtomicBool` `CANCEL`; every child goes in its own process group and the group is killed with `kill -KILL -pgid` (so what `soffice` launches dies too). It returns the `[cancelled]` error. The tests that convert take a `Mutex` so they do not step on the flag.
- **Real progress:** `convert(app, path, format)` creates a `Progress` closure that emits the `convert-progress` event (`{ "percent": f64 }`, from 0 to 99, at most every 100 ms, never 100) and calls `convert_with`. `convert_ffmpeg` and `convert_subtitle` get the duration with `ffprobe` and read `-progress pipe:1` (it accepts `out_time_us`, `out_time_ms` and `out_time=`). With no duration or no callback nothing is emitted. The pure logic is `progress_seconds`, `next_percent` and `parse_duration`; `convert_blocking` only exists for tests.
- **Several files:** `get_targets_many` (rule: all the supported files must accept the format and not all of them may already be in it; `type_targets` and `is_already` are the helper functions) and `convert_many`, which uses `convert_one` for each file (without touching `CANCEL`, which is reset only once at the start). It returns `BatchResult { outputs, skipped, failed }`; a failure does not stop the batch and cancelling keeps what was already created. It emits `convert-file` and the global progress (`global_percent`, `GlobalProgress`).
- **Thumbnail:** `src-tauri/src/thumbnail.rs`, command `thumbnail(path)` → JPEG data URL of at most 200 px (images and SVG with ImageMagick/rsvg-convert, video with an ffmpeg frame, PDF with the first page through pdftoppm). At most 3 s, it does not use `CANCEL` and does not write next to the original. Base64 done by hand. Names with `[`, `]`, `*` or `:` work by using an absolute path plus `[0]` (the `file:` prefix breaks with those names).
- **Output names:** `name.ext`, and if it exists `name 2.ext`, `name 3.ext`... (page folders: `name_pages 2`).
- **Folder as input:** `Kind::Folder`; `convert_folder` compresses from the parent folder, including the name inside the archive.
- The wheel commands (`initial_paths`, `file_info`, `show_wheel`, `cancel`, `quit`) are in `src-tauri/src/wheel.rs`.
- Errors: a short message in the active language (see [[Languages]]); if a tool is missing, it gives the `apt install` command.

## Decisions

- pdf/document/presentation → jpg/png: with **several pages** it creates a folder `<name>_pages` (`_pages 2`...) and `convert` returns the folder; with **a single page** it returns a loose file `<name>.jpg` and leaves no folder (`single_page_to_file`). A difference from the Python version, which always created a folder.
- pdf → md and epub go through DOCX and then pandoc.
- The LibreOffice TSV quotes the texts, the same as the Python version.
- ffmpeg uses `-n` (do not overwrite) instead of `-y`.
- PDF → pptx/xlsx from the Python version are left out (the xlsx needed openpyxl).

## Tests

`cargo test`: unit tests of types, targets, aliases and names, plus integration tests with ffmpeg, ImageMagick, pandoc, pdftotext, pdftoppm, LibreOffice (csv→xlsx/tsv), zip→tar.gz, packing and documents with HTML. The integration ones skip themselves if the tool is missing. Tried by hand: md→pdf/png, docx→pdf, doc→md/pdf, pptx→pdf/jpg, tsv→xlsx/csv, pdf→docx/md/epub/txt.

**Also tried:** SVG to png/pdf/jpg, video to GIF, the missing tool notice, progress with a 3 s audio, cancelling with progress active.

**Untested:** rar (there is no `rar` or `unrar`; the test `convierte_rar_a_zip` is skipped), zip extraction with 7z without `unzip`, thumbnails of heic and avif, real .xls, .ods and .odp files, and the events and thumbnails in a real Tauri window. See [[Manual-Testing]].

Related: [[Architecture]], [[Decisions]].

## Packing files (zip, tar, tar.gz)

- `pack(paths, format)` (in `converters.rs`) packs one or several files or folders of any type into a single archive next to the first one: `name.zip` with one, `Archive.zip` (`Archivo.zip` in Spanish) with several. It links everything into a temporary folder so that it ends up at the root of the archive (zip follows links, tar with `-h`).
- With **several files** and none of them an archive, `get_targets_many` always offers `zip` (if the list already has 8 formats, the last one is dropped to make room for it, since the wheel takes 8) and `convert_many` with zip/tar/tar.gz makes **a single file** with all of them, not one per file. If any of them is a zip/tar/rar, conversion goes on file by file.
- With **a single** file, zip is the `zip` tool of [[Tools]], not a conversion target (the wheel has 8 segments at most).

## Images of documents in md and html

`convert_pandoc` passes `--extract-media=<name>_media` when the target is `md` or `html` (with cwd = the folder of the result, so that the links come out relative: `name_media/media/rId1.jpg`). The folder is chosen by `unique_media_folder` (`name_media`, `name_media_2`... with no spaces, because they go inside links); pandoc only creates it if the document has images, and if the conversion fails it is deleted. Without this the links were broken and the images were lost. The `html` target also goes as a complete page (`-s`, with `pagetitle` = the file name): as a fragment it had no `<meta charset>` and the accents looked wrong when opened in a browser. `epub` already carries the images inside and `txt` has none. Tried by hand with a 265-page docx (pdf 6 s, txt/md/html/epub 2 to 3 s, 265 jpg 24 s).

## Markdown with HTML (GitHub README) to docx and txt

A `.md` with `<table>`, `<img>`, `<br>` or `<b>` inside, converted directly to docx, loses that HTML: pandoc leaves it as raw HTML and the docx writer drops it (HTML tables came out as loose paragraphs and `<img>` images disappeared, including the logo of a cover page). `convert_pandoc` goes through HTML when the source is `.md` and the target is `docx` or `txt` (the pdf/jpg/png targets too, because they go through an intermediate docx): `pandoc x.md -t html` and then `pandoc mid.html -f html`. Measured with a 6800-line report: tables from 71 to 135, images from 141 to 160, and the headings, links and code blocks stay the same; 6 s. The relative images of the `.md` are looked up next to the `.md` (`--resource-path`), so **the `assets/` folder has to be beside it**: a `.md` copied to another folder without its images comes out with the alternative text in their place (and pandoc warns on stderr, which the app does not show). The page size, the centering and the page breaks are explained below.

## PDF to docx: limits

`pdf → docx` uses the `writer_pdf_import` filter of LibreOffice, which does not rebuild the text: each line ends up in a loose text box at the position it had in the PDF. A 195-page PDF generated by Chrome from a `.md` came out as a 47 MB docx with almost 10,000 text boxes, with no headings, tables or links, 147 s of conversion, and LibreOffice could not open it again and render it in 10 minutes. The text is complete, but when rendered the end of several lines comes out cut off (`1ASI0732` as `1ASI073`) and the bold is lost. It is fine for short, simple documents; for a long report it is better to convert from the `.md`, the `.docx` or the editable original.

## Format of the docx files: A4, centering and page breaks

Every pandoc `docx` target (also the intermediate docx used by pdf/jpg/png) carries `--reference-doc` and `--lua-filter`, with two files that live inside the program (`include_bytes!`/`include_str!` in `converters.rs`, sources in `src-tauri/assets/`):

- `reference-a4.docx`: an **A4** page with 2.5 cm margins (pandoc uses Letter by default) and the paragraph style `Centered` (centered). It was made with `pandoc -o x.docx --print-default-data-file reference.docx`, changing the `sectPr` of `word/document.xml` and adding the style to `word/styles.xml`. Always A4; it does not depend on the language.
- `center.lua`: `<div align="center">` gets the `Centered` style, and `<div style="page-break-before: always">` (or `page-break-after`) becomes a page break (`openxml`). Note: when going from Markdown to HTML pandoc writes `data-align`, not `align`, and the filter looks at both.

pandoc loses `<p align="center">` when reading it, so `wrap_centered_paragraphs` wraps it in a `<div align="center">` in the intermediate HTML (only the form with double quotes; `<h1 align=...>` and the `align` of `<td>` are left alone). A Markdown table inside a centered div is not centered (it stays on the left).

Tried with a 6800-line report: A4 page, 11 page breaks and 35 centered paragraphs; the cover comes out centered with its logo. Manual test with a real file: `MANDARINA_REAL=/path/file.md MANDARINA_FORMAT=docx cargo test archivo_real -- --ignored --nocapture` (it creates the result next to the file).
