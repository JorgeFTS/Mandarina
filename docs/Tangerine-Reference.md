# Reference: Tangerine for Mac

Source: press kit and demo video in `~/Downloads/` (Leap Studio, v1.4, $29.99, Mac Apple silicon only, 100% offline). It is inspiration for behavior only: do not copy their images, icon, texts or exact design.

## Behavior

- When a file is dragged, a wheel appears next to the cursor. **Three keys, three wheels:** Shift = conversions, Command = file tools, Control = file actions. The wheel stays open when the key is released.
- The file is dropped on a segment. Then a floating progress card appears ("Converting to MP3", bar, X to cancel) and the result appears next to the original with the name `sample 2.mp3`.
- Center of the wheel: thumbnail of the file and a pill with the size (2.4 MB); hovering a segment shows the format. The highlighted segment turns orange.

## Content of each wheel

- **Conversion (image):** JPG, WEBP, HEIC, TIFF, AVIF, BMP, PDF, DOCX (8 segments). Audio: MP3, M4A, FLAC, OGG, OPUS, AIFF (6).
- **Tools (image):** COMPRESS, METADATA, EDIT, ANNOTATE, ADD BG, CROP, REDACT, each with an icon and a label.
- **Tool windows:** Edit Photo (sliders for exposure, contrast, shadows, temperature...), Add BG (backgrounds, padding, corners, shadow, aspect ratio), Trim Audio (waveform with draggable ends), Convert Audio Channels (mono/stereo), Bleep.
- **Control (file actions):** it does not appear in the kit.

## What this means for Mandarina on Linux

- Wayland does not allow detecting global drags or Shift/Ctrl outside the window: the three wheels would open from the right click (three entries or a submenu) or with the wheel visible in our window. See [[Decisions]].
- Planned order: 1) conversion wheel ([[Wheel]], [[Converters]]), 2) progress card with cancel, 3) tools wheel (done: see [[Tools]]).
- Name: "Mandarina" is the Spanish translation of "Tangerine". It is worth thinking about whether there is a risk of trademark confusion before publishing.
