# Tools

The second wheel of the app, inspired by the tools wheel of [[Tangerine-Reference]]. It opens from a **TOOLS** segment of the conversion wheel and only with **one file** selected (it is not offered with several). Commands and types in [[Tauri-Commands]]; interface in [[Wheel]]; Rust in `src-tauri/src/tools.rs`.

## Flow of use

1. Conversion wheel → **TOOLS** segment (the last one of the wheel; it only appears if `get_tools` returns something).
2. Tools wheel (the same wheel component, other segments). The center of the disc (or Escape) goes back to the previous wheel.
3. Depending on the tool:
   - without options (**Reduce size**, **Metadata**, **No audio**, **ZIP**): it runs directly;
   - with options (**Rotate**, **Flip**, **Resize**): a third wheel with the options;
   - with an editor (**Trim**): the window grows and the visual editor opens (a box over the image, or a waveform / frames with two draggable ends for audio and video); Apply and Cancel buttons.
4. Progress card and "Done ✓" in the center, as in the conversions.

## Tools by type (first version)

| Type | Tools |
|---|---|
| image | reduce size, metadata, trim (box), rotate, flip, resize, zip |
| video | reduce size, metadata, trim (time), rotate, flip, resize, no audio, zip |
| audio | reduce size, metadata, trim (time), zip |
| pdf | reduce size (ghostscript), zip |
| document, presentation, spreadsheet, subtitles, svg, heic | zip only |
| archives, folders and unknown types | none (the segment does not show) |

If a type has only one tool, the conversion wheel shows that segment directly (e.g. ZIP on a document) instead of TOOLS.

## Rules

- The output goes next to the original, in the **same format**, with a suffix: `name (compressed).ext`, `(no metadata)`, `(trimmed)`, `(rotated)`, `(flipped)`, `(resized)`, `(no audio)` (they change with the language, see [[Languages]]); if it exists, `name (compressed) 2.ext`. It is never overwritten.
- A single compression level, balanced.
- **Name of the new file (Trim only):** the three editors have a text field, with the initial value `name (trimmed)` and the extension of the original in gray at its right. It is validated live (empty, `/`, `.`, `..`, more than 200 characters) and Apply is disabled if it is not valid. The editor returns `nombre` along with the other parameters and `run_tool` uses it (`custom_name`, `output_path_named`): it keeps the extension of the original, removes the one the user types at the end and never overwrites (`name 2.ext`). Enter inside the field applies.
- The ffmpeg tools emit `convert-progress` and can be cancelled with `cancel`, like the conversions.
- Programs: ImageMagick, ffmpeg and ghostscript (`gs`). exiftool is not needed.

## How each tool is done (measured on this machine)

| Tool | Chosen command | Measured result |
|---|---|---|
| image: reduce jpg | `magick -auto-orient -strip -quality 75 -sampling-factor 4:2:0 -interlace Plane` | 843 KB → 175 KB |
| image: reduce webp / avif / png / tiff / gif | `-quality 75` / `-quality 60` / `png:compression-level=9` + filter and strategy / `-compress zip` / `-layers Optimize` | avif 1.4 MB → 70 KB; screenshot png 50 → 21 KB, but with photos it hardly saves anything; tiff and gif save little |
| image: metadata, trim, rotate, flip, resize | `-auto-orient -strip`; `-crop WxH+X+Y +repage`; `-rotate`; `-flop`/`-flip`; `-resize N%` or `Nx>` (does not enlarge) | checked with measurements and pixels |
| video: reduce | libx264 crf 28 + aac 128k (mp4, mov, mkv); libx264 + libmp3lame (avi); libvpx-vp9 crf 33 + opus (webm, slow); wmv2 | 7.9 MB → 2.0 MB |
| video: metadata / no audio | `-map_metadata -1 -map_chapters -1 -c copy` / `-map 0:v -c copy` | not re-encoded |
| video: trim | `-ss S -i ... -t D` and it **re-encodes** | frame-exact cut |
| video: rotate, flip, resize | `transpose`, `hflip`/`vflip`, `scale` with even dimensions (does not enlarge) | the audio is copied |
| audio: reduce | mp3 libmp3lame 128k; m4a aac 128k; ogg libvorbis q4; opus 96k; wma 128k | mp3 803 → 321 KB |
| audio: metadata / trim | `-map_metadata -1 -c copy` / `-ss -i -t -c copy` | trim accurate to about 25 ms (flac is re-encoded) |
| pdf: reduce | `gs -sDEVICE=pdfwrite -dPDFSETTINGS=/ebook` (output to a temporary file) | 5.1 MB → 131 KB |

## Decisions

- If the result does not weigh less than 98% of the original, it is deleted and an `[optimized] Already optimized` error is returned (the interface shows it as a soft notice). The `[optimized]` and `[cancelled]` codes do not change with the language (see [[Languages]]).
- **REDUCE SIZE is not offered** for wav, flac, aiff (lossless) or bmp: there is no way to compress them without losing quality or format.
- `get_tools` returns empty for svg, heic and heif (ImageMagick does not write them with the same extension, or they are vector formats).
- Reducing audio drops the embedded cover art; reducing and trimming video leave only the first video track and the audio ones, without subtitles.
- Removing the metadata of an image also removes the ICC profile.
- Videos with rotation by metadata: `media_info` swaps width and height, and ffmpeg applies the rotation when re-encoding.
- The ffmpeg and ImageMagick messages come out with the prefix "Could not apply the tool:" but the detail stays in English.
- In the conversion wheel, TOOLS is dropped if there are already 8 target formats (it happens with formats such as aac or flv, which are not in their own list).
- The wheel waits for `get_tools` for up to 400 ms; if it does not arrive, it is painted without the segment.

## Tests

Part of the `cargo test` suite: table of tools, output names, parameter validation, waveform peaks, and integration with real files (reduce, metadata, trim, rotate, flip, resize, no audio, progress and cancel). **Untested:** the real window (`resize_center`, the editor with the mouse), `convert` together with tools, trims of heic, svg and animated gif, videos with several audio tracks or subtitles, and real avif end to end. See [[Manual-Testing]].

## Pending / not included

Playing the audio or video inside the editor, trimming video in space (only in time), tools for documents and spreadsheets, and running tools with several files.

- **Progress card with a custom name:** if the editor returns `nombre`, the progress card shows that name (plus the extension of the original, `nombreConExtension` in `src/main.js`) instead of the name of the source file.
- **Reducing size is not zipping:** `compress` re-encodes the file so that it weighs less and leaves it in the same format (its label went from COMPRIMIR to REDUCIR TAMAÑO / REDUCE SIZE; the id and the `(compressed)` suffix did not change).
- **ZIP (`zip`):** a tool for any file that is not an archive or a folder. It calls `converters::pack`, which leaves `name.zip` next to the original (it never overwrites). With several files the same `pack` is used from the conversion wheel, see [[Converters]].
