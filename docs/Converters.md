# Conversores

Archivo: `src-tauri/src/converters.rs` (~800 líneas, un solo archivo). Port a Rust de `~/Desktop/Projects/mandarina/converters.py`. Los comandos que expone están en [[Comandos-Tauri]].

## Tipos y destinos (orden de rueda, sin el propio formato)

| Tipo | Destinos | Herramienta |
|---|---|---|
| imagen | jpg, png, webp, tiff, avif, bmp, pdf | ImageMagick, rsvg-convert (SVG) |
| audio | mp3, m4a, wav, flac, ogg, opus, aiff, wma | ffmpeg |
| vídeo | mp4, mov, mkv, webm, avi, wmv, gif, mp3 | ffmpeg |
| pdf | docx, jpg, png, epub, txt, md | pdftoppm, pdftotext, pandoc, LibreOffice |
| documento | docx, md, txt, epub, html, pdf, jpg, png | pandoc, LibreOffice |
| presentación | pdf, jpg, png | LibreOffice |
| hoja de cálculo | xlsx, ods, csv, tsv, pdf | LibreOffice |
| subtítulo | srt, vtt, txt | ffmpeg, Rust (txt) |
| comprimido | zip, tar, tar.gz (rar solo entrada) | zip, tar, unzip, 7z |
| carpeta | zip, tar, tar.gz (comprimir la carpeta) | zip, tar |

Alias: `jpeg`→`jpg`, `tif`→`tiff`, `heif`→`heic`, `htm`→`html`, `tgz`→`tar.gz`. HEIC es solo entrada (ImageMagick local no lo escribe).

## Cómo funciona

- `category` y `targets_of` deciden el tipo y los destinos; `convert_blocking` valida, calcula la salida con `unique_path` (nunca sobrescribe: `_1`, `_2`...) y despacha a `convert_image`, `convert_ffmpeg`, `convert_pdf`, `convert_document`, `convert_slides`, `convert_spreadsheet`, `convert_subtitle` o `convert_archive`.
- Todo con `std::process::Command`, sin shell; ruta absoluta, y `--` donde la herramienta lo admite (un archivo `-x.pdf` no se lee como opción).
- `TempDir` es una carpeta temporal que se borra sola, también en error. LibreOffice usa un perfil propio (`-env:UserInstallation`) para no chocar con una instancia abierta.
- `tool_exists` busca en el PATH sin ejecutar nada.
- Si una conversión falla o se cancela se borra la salida a medias.
- **Cancelar:** `run_in` usa `spawn` y un bucle `try_wait` cada 50 ms que mira el `AtomicBool` global `CANCEL`; cada hijo va en su propio grupo de procesos y se mata el grupo con `kill -KILL -pgid` (así también muere lo que lance `soffice`). Devuelve `Err("Cancelado")`. Los tests que convierten toman un `Mutex` para no pisarse el flag.
- **Progreso real:** `convert(app, path, format)` crea un cierre `Progress` que emite el evento `convert-progress` (`{ "percent": f64 }`, de 0 a 99, como mucho cada 100 ms, nunca 100) y llama a `convert_with`. `convert_ffmpeg` y `convert_subtitle` obtienen la duración con `ffprobe` y leen `-progress pipe:1` (acepta `out_time_us`, `out_time_ms` y `out_time=`). Sin duración o sin callback no se emite nada. La lógica pura es `progress_seconds`, `next_percent` y `parse_duration`; `convert_blocking` solo existe para pruebas.
- **Varios archivos:** `get_targets_many` (regla: todos los archivos soportados deben admitir el formato y no todos deben estar ya en él; `type_targets` e `is_already` son las funciones auxiliares) y `convert_many`, que usa `convert_one` para cada archivo (sin tocar `CANCEL`, que se reinicia una sola vez al empezar). Devuelve `BatchResult { outputs, skipped, failed }`; un fallo no detiene el lote y la cancelación conserva lo ya creado. Emite `convert-file` y el progreso global (`global_percent`, `GlobalProgress`).
- **Miniatura:** `src-tauri/src/thumbnail.rs`, comando `thumbnail(path)` → data URL JPEG de máximo 200 px (imágenes y SVG con ImageMagick/rsvg-convert, vídeo con un fotograma de ffmpeg, PDF con la primera página con pdftoppm). Máximo 3 s, no usa `CANCEL` ni escribe junto al original. Base64 hecho a mano. Los nombres con `[`, `]`, `*` o `:` funcionan usando ruta absoluta más `[0]` (el prefijo `file:` rompe con esos nombres).
- **Nombres de salida:** `nombre.ext`, y si existe `nombre 2.ext`, `nombre 3.ext`... (carpetas de páginas: `nombre_pages 2`).
- **Carpeta como entrada:** `Kind::Folder`; `convert_folder` comprime desde la carpeta padre incluyendo el nombre dentro del archivo.
- Los comandos de la rueda (`initial_path`, `file_info`, `show_wheel`, `cancel`, `quit`) están en `src-tauri/src/wheel.rs`.
- Errores: mensaje corto en español; si falta una herramienta, indica el `apt install`.

## Decisiones

- pdf/documento/presentación → jpg/png: con **varias páginas** crea una carpeta `<nombre>_pages` (`_pages 2`...) y `convert` devuelve la carpeta; con **una sola página** devuelve un archivo suelto `<nombre>.jpg` y no deja carpeta (`single_page_to_file`). Diferencia con el Python, que siempre creaba carpeta.
- pdf → md y epub pasan por DOCX y luego pandoc.
- El TSV de LibreOffice entrecomilla los textos, igual que el Python.
- ffmpeg usa `-n` (no sobrescribir) en vez de `-y`.
- PDF → pptx/xlsx del Python quedan fuera (el xlsx necesitaba openpyxl).

## Pruebas

`cargo test`: 51 (unitarias de tipos, destinos, alias, nombres; integración con ffmpeg, ImageMagick, pandoc, pdftotext, pdftoppm, LibreOffice csv→xlsx/tsv, zip→tar.gz). Las de integración se saltan si falta la herramienta. Probado a mano: md→pdf/png, docx→pdf, doc→md/pdf, pptx→pdf/jpg, tsv→xlsx/csv, pdf→docx/md/epub/txt.

**Probado además:** SVG a png/pdf/jpg, vídeo a GIF, aviso de herramienta que falta, progreso con un audio de 3 s, cancelar con progreso activo.

**Sin probar:** rar (no hay `rar` ni `unrar`; la prueba `convierte_rar_a_zip` se salta), extracción de zip con 7z sin `unzip`, miniatura de heic y avif, archivos .xls, .ods, .odp reales, y los eventos y miniaturas en una ventana Tauri real.

Relacionado: [[Arquitectura]], [[Decisiones]].

## Empaquetar archivos (zip, tar, tar.gz)

- `pack(paths, formato)` (en `converters.rs`) empaqueta uno o varios archivos o carpetas de cualquier tipo en un solo archivo junto al primero: `nombre.zip` con uno, `Archive.zip` / `Archivo.zip` con varios. Enlaza todo en una carpeta temporal para que quede en la raíz del archivo (zip sigue los enlaces, tar con `-h`).
- Con **varios archivos** y ninguno comprimido, `get_targets_many` ofrece siempre `zip` (si la lista ya tiene 8 formatos se quita el último para dejarle sitio, la rueda admite 8) y `convert_many` con zip/tar/tar.gz hace **un solo archivo** con todos, no uno por archivo. Si alguno es un zip/tar/rar sigue la conversión archivo por archivo.
- Con **un archivo** suelto el zip es la herramienta `zip` de [[Herramientas]], no un destino de conversión (la rueda tiene 8 segmentos como máximo).

## Imágenes de los documentos en md y html

`convert_pandoc` pasa `--extract-media=<nombre>_media` cuando el destino es `md` o `html` (con cwd = la carpeta del resultado, para que los enlaces salgan relativos: `nombre_media/media/rId1.jpg`). La carpeta la elige `unique_media_folder` (`nombre_media`, `nombre_media_2`... sin espacios, porque van dentro de enlaces); pandoc solo la crea si el documento tiene imágenes, y si la conversión falla se borra. Sin esto los enlaces quedaban rotos y las imágenes se perdían. El destino `html` va además como página completa (`-s`, con `pagetitle` = nombre del archivo): como fragmento no llevaba `<meta charset>` y los acentos se veían mal al abrirlo en un navegador. `epub` ya lleva las imágenes dentro y `txt` no las tiene. Probado a mano con un docx de 265 páginas (pdf 6 s, txt/md/html/epub 2 a 3 s, 265 jpg 24 s).

## Markdown con HTML (README de GitHub) a docx y txt

Un `.md` con `<table>`, `<img>`, `<br>` o `<b>` dentro, directo a docx, pierde ese HTML: pandoc lo deja como HTML crudo y el escritor de docx lo descarta (las tablas HTML salían como párrafos sueltos y las imágenes `<img>` desaparecían, incluido el logo de una portada). `convert_pandoc` pasa por HTML cuando el origen es `.md` y el destino es `docx` o `txt` (los destinos pdf/jpg/png también, porque pasan por un docx intermedio): `pandoc x.md -t html` y luego `pandoc mid.html -f html`. Medido con un informe de 6800 líneas: tablas 71 a 135, imágenes 141 a 160, y los títulos, enlaces y bloques de código quedan igual; 6 s. Las imágenes relativas del `.md` se buscan junto al `.md` (`--resource-path`), así que **la carpeta `assets/` tiene que estar al lado**: un `.md` copiado a otra carpeta sin sus imágenes sale con el texto alternativo en su lugar (y pandoc avisa por stderr, que la app no muestra). El tamaño de hoja, el centrado y los saltos de página se explican abajo.

## PDF a docx: límites

`pdf → docx` usa el filtro `writer_pdf_import` de LibreOffice, que no reconstruye el texto: cada línea queda en un cuadro de texto suelto con la posición del PDF. En un PDF de 195 páginas generado por Chrome desde un `.md` salió un docx de 47 MB con casi 10 000 cuadros de texto, sin títulos, tablas ni enlaces, 147 s de conversión, y LibreOffice no logró volver a abrirlo y renderizarlo en 10 minutos. El texto está completo, pero al renderizar el final de varias líneas sale cortado (`1ASI0732` como `1ASI073`) y se pierden las negritas. Sirve para documentos cortos y simples; para un informe largo conviene convertir desde el `.md`, el `.docx` o el original editable.

## Formato de los docx: A4, centrado y saltos de página

Todo destino `docx` de pandoc (también el docx intermedio que usan pdf/jpg/png) lleva `--reference-doc` y `--lua-filter`, con dos archivos que van dentro del programa (`include_bytes!`/`include_str!` en `converters.rs`, fuentes en `src-tauri/assets/`):

- `reference-a4.docx`: hoja **A4** con márgenes de 2,5 cm (pandoc usa Carta por defecto) y el estilo de párrafo `Centered` (centrado). Se hizo con `pandoc -o x.docx --print-default-data-file reference.docx`, cambiando el `sectPr` de `word/document.xml` y añadiendo el estilo a `word/styles.xml`. Siempre A4; no depende del idioma.
- `center.lua`: `<div align="center">` pasa al estilo `Centered`, y `<div style="page-break-before: always">` (o `page-break-after`) pasa a un salto de página (`openxml`). Ojo: al pasar de Markdown a HTML pandoc escribe `data-align`, no `align`, y el filtro mira los dos.

`<p align="center">` lo pierde pandoc al leerlo, así que `wrap_centered_paragraphs` lo envuelve en un `<div align="center">` en el HTML intermedio (solo la forma con comillas dobles; los `<h1 align=...>` y los `align` de `<td>` no se tocan). Una tabla de Markdown dentro de un div centrado no se centra (queda a la izquierda).

Probado con un informe de 6800 líneas: hoja A4, 11 saltos de página y 35 párrafos centrados; la portada queda centrada con su logo. Prueba manual con un archivo real: `MANDARINA_REAL=/ruta/archivo.md MANDARINA_FORMAT=docx cargo test archivo_real -- --ignored --nocapture` (crea el resultado junto al archivo).
