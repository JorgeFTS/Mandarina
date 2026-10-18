# Mandarina

[English](README.md) | Español

Conversor de archivos para Linux con una rueda radial flotante. Seleccionas uno o varios archivos en Nautilus, pulsas un atajo o haces clic derecho, y aparece una rueda junto al cursor con los formatos a los que se pueden convertir. Eliges uno y la conversión se hace en tu equipo, sin subir nada a internet.

> Estado: prototipo temprano (versión 0.1.0). Lee la sección [Estado del proyecto](#estado-del-proyecto) antes de usarlo.

## Cómo funciona

1. Selecciona uno o varios archivos (o una carpeta) en Nautilus.
2. Pulsa **Alt+M**, o haz clic derecho y elige **Scripts → Convertir con Mandarina** (con el nombre según el idioma, por ejemplo **Convert with Mandarina** en inglés; ver [Idioma](#idioma)).
3. Aparece la rueda junto al cursor, con un segmento por formato de destino.
4. Elige un formato (clic, o flechas y Enter). Una tarjeta muestra el progreso y se puede cancelar.
5. El resultado queda en la misma carpeta que el original con un nombre nuevo (`foto.png` → `foto.jpg`). Nunca se sobrescribe nada: si ya existe, se crea `foto 2.jpg`, `foto 3.jpg`...

Escape cierra la rueda. También puedes lanzar `mandarina` sin argumentos para arrastrar un archivo sobre la ventana.

Las conversiones las hacen programas del sistema (ffmpeg, ImageMagick, pandoc, LibreOffice, poppler...); Mandarina los llama desde Rust sin pasar por un intérprete de comandos.

## Idioma

Mandarina está disponible en **inglés, español, portugués (Português) y francés (Français)**. Puedes elegir el idioma en los ajustes (el engranaje que aparece en la parte baja del círculo central de la rueda) o dejarlo en **Automático**. El cambio se aplica al momento.

En Automático, el idioma se elige una sola vez al arrancar, a partir de las variables de entorno del sistema, en este orden: `MANDARINA_LANG` (ver abajo), el idioma guardado en los ajustes, `LC_ALL`, `LC_MESSAGES`, `LANGUAGE` (el primer idioma de la lista) y `LANG`. Si el valor empieza por `es`, `pt` o `fr`, la app usa ese idioma; en cualquier otro caso, o si no hay ninguna variable, usa inglés (por ejemplo, un sistema en chino verá la app en inglés).

Para forzar un idioma, define `MANDARINA_LANG` como `en`, `es`, `pt` o `fr`, que tiene prioridad sobre las demás:

```bash
MANDARINA_LANG=pt mandarina
```

El idioma se aplica a la interfaz, los mensajes de error, los nombres de los archivos que crean las herramientas (`foto (comprimido).jpg` en español, `photo (compressed).jpg` en inglés, `foto (compressé).jpg` en francés) y la entrada del menú de Nautilus (ver más abajo). Los mensajes de ffmpeg, ImageMagick y los demás programas se muestran tal cual. Las traducciones al portugués y al francés se escribieron sin revisión de un hablante nativo, así que las correcciones son bienvenidas (ver `CONTRIBUTING.md` para añadir o corregir un idioma).

## Herramientas

Con **un solo archivo**, el último segmento de la rueda es **HERRAMIENTAS** y abre una segunda rueda:

| Tipo | Herramientas |
|---|---|
| Imagen | reducir tamaño, quitar metadatos, recortar (con recuadro visual), girar, voltear, redimensionar |
| Vídeo | reducir tamaño, quitar metadatos, recortar (con tira de fotogramas), girar, voltear, redimensionar, quitar el audio |
| Audio | reducir tamaño, quitar metadatos, recortar (con forma de onda) |
| PDF | reducir tamaño |
| Cualquier otro archivo | zip |

El resultado va junto al original, con el mismo formato y un sufijo: `foto (comprimido).jpg`, y del mismo modo `(sin metadatos)`, `(recortado)`, `(girado)`, `(volteado)`, `(redimensionado)` y `(sin audio)`. Si comprimir no reduce al menos un 2 %, avisa en vez de crear un archivo casi igual. Wav, flac, aiff y bmp no ofrecen reducir tamaño. Reducir tamaño no es zipear: la herramienta ZIP empaqueta cualquier archivo en un `.zip`.

Los editores de recorte (imagen, audio y vídeo) incluyen un campo con el nombre del archivo nuevo. Empieza como `foto (recortado)`, junto a él se muestra la extensión del original y se valida mientras escribes. Comprimir un PDF necesita ghostscript. Más detalles en `docs/Herramientas.md`.

## Formatos soportados

| Tipo | Destinos | Herramienta |
|---|---|---|
| Imagen | jpg, png, webp, tiff, avif, bmp, pdf | ImageMagick, rsvg-convert (SVG) |
| Audio | mp3, m4a, wav, flac, ogg, opus, aiff, wma | ffmpeg |
| Vídeo | mp4, mov, mkv, webm, avi, wmv, gif, mp3 | ffmpeg |
| PDF | docx, jpg, png, epub, txt, md | pdftoppm, pdftotext, pandoc, LibreOffice |
| Documento | docx, md, txt, epub, html, pdf, jpg, png | pandoc, LibreOffice |
| Presentación | pdf, jpg, png | LibreOffice |
| Hoja de cálculo | xlsx, ods, csv, tsv, pdf | LibreOffice |
| Subtítulo | srt, vtt, txt | ffmpeg |
| Comprimido | zip, tar, tar.gz (rar solo como entrada) | zip, tar, unzip, 7z |
| Carpeta | zip, tar, tar.gz | zip, tar |

HEIC solo se admite como entrada. Algunos destinos dependen de lo que tenga compilado tu ImageMagick o ffmpeg.

## Requisitos

Probado en Ubuntu con GNOME 50. En otras distribuciones debería funcionar con los equivalentes de estos paquetes, pero no se ha comprobado.

Herramientas de conversión (necesarias para usar la app; sin alguna de ellas fallan solo los formatos que la usan, y la app indica qué paquete instalar):

```bash
sudo apt install ffmpeg imagemagick librsvg2-bin pandoc libreoffice poppler-utils ghostscript zip unzip p7zip-full
```

Para compilar, además:

```bash
sudo apt install build-essential curl wget file libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Y necesitas [Rust](https://rustup.rs) (edición 2024, es decir, Rust 1.85 o superior) y [Node.js](https://nodejs.org) con npm.

## Compilar y ejecutar

```bash
npm install
npm run tauri dev                                # modo desarrollo
npm run tauri build -- --debug --no-bundle       # binario independiente (depuración)
npm run tauri build -- --no-bundle               # binario de release (más lento, con LTO)
npm run tauri build -- --bundles deb             # paquete .deb
```

El binario queda en `src-tauri/target/debug/mandarina` (o `release/`). Hay que recompilar cada vez que cambie `src/`, porque el binario lleva la interfaz incluida. Más detalles del empaquetado en [docs/Publicar.md](docs/Publicar.md).

## Instalar el clic derecho y el atajo en Nautilus

```bash
packaging/install-nautilus.sh                    # usa src-tauri/target/debug/mandarina
packaging/install-nautilus.sh /ruta/a/mandarina  # o la ruta de tu binario
nautilus -q                                      # reinicia Nautilus para que lea el atajo
```

El instalador copia un script a `~/.local/share/nautilus/scripts/` y añade el atajo `Alt+M` a `~/.config/nautilus/scripts-accels`. El script se llama según el idioma activo (ver [Idioma](#idioma)): `Convertir con Mandarina`, `Convert with Mandarina`, `Converter com Mandarina` o `Convertir avec Mandarina`. Si estaba instalado el del otro idioma, el instalador lo borra para que quede una sola entrada, y se puede ejecutar de nuevo en cualquier momento. Para otro atajo: `MANDARINA_ATAJO="<Control><Alt>m" packaging/install-nautilus.sh`.

Para desinstalar, borra el script de `~/.local/share/nautilus/scripts/` y su línea en `scripts-accels`.

## Limitaciones y problemas conocidos

- **Wayland:** una aplicación no puede conocer la posición del cursor ni detectar arrastres o teclas como Shift fuera de su ventana. Por eso el script de Nautilus lanza la app con `GDK_BACKEND=x11` (XWayland) y la ventana se coloca junto al cursor en dos pasos. Sin `GDK_BACKEND=x11` no se coloca junto al cursor.
- Con **varios archivos**, la rueda ofrece solo los formatos que todos admiten (por ejemplo, 5 imágenes dan los formatos de imagen; una imagen y un documento solo `pdf`, `jpg` y `png`). Los que ya están en ese formato se omiten, y si uno falla se siguen convirtiendo los demás. Las carpetas se pueden comprimir. Con varios archivos de cualquier tipo, la rueda siempre ofrece ZIP y los empaqueta todos en un solo `Archivo.zip`. La rueda de herramientas solo está disponible con un archivo.
- **Solo Nautilus.** No hay integración con otros gestores de archivos.
- **GNOME 50 impide que la app arranque.** Con GNOME 50, WebKitGTK/GTK 3.24 aborta con `Settings schema 'org.gnome.settings-daemon.plugins.xsettings' does not contain a key named 'antialiasing'`, porque GNOME movió esas claves a un esquema `.deprecated`. Es un problema del sistema, no de Mandarina. Arreglo local sin `sudo`: copiar el esquema `xsettings` con las claves `antialiasing`, `hinting` y `rgba-order` añadidas, junto con `org.gnome.settings-daemon.enums.xml`, a `~/.local/share/glib-2.0/schemas/` y ejecutar `glib-compile-schemas` en esa carpeta. Bórralo cuando GNOME lo corrija. Detalles en [docs/Decisiones.md](docs/Decisiones.md).
- **VS Code instalado con snap:** si lanzas la app desde su terminal integrada, hereda variables de entorno del snap (`GTK_PATH`, `GIO_MODULE_DIR`, `GSETTINGS_SCHEMA_DIR`...) que hacen fallar a WebKitGTK. Lánzala desde otra terminal o limpia esas variables antes.
- Archivos de nombre raro, permisos o conversiones muy pesadas (vídeo largo, documentos enormes) no se han probado de forma sistemática.

## Estado del proyecto

Funciona y está probado:

- Los conversores y herramientas de Rust: `cargo test` ejecuta pruebas unitarias y de integración con ffmpeg, ImageMagick, pandoc, poppler, LibreOffice y ghostscript (las de integración se saltan si falta la herramienta).
- A mano: md a pdf/png, docx a pdf, doc a md/pdf, pptx a pdf/jpg, tsv a xlsx/csv, pdf a docx/md/epub/txt.

No verificado o pendiente:

- La rueda en la ventana real: posición junto al cursor, transparencia en pantalla, arrastre real y cancelar. Las capturas de diseño solo prueban la interfaz, no Tauri.
- Sin probar: rar, extracción de zip solo con 7z, SVG, vídeo a GIF, los avisos de "falta la herramienta" y archivos reales .xls, .ods y .odp.
- Solo se ha probado en Ubuntu con GNOME 50 (Wayland). No hay pruebas en X11 puro, otras distribuciones ni otros entornos de escritorio.
- Paquete `.deb` generado pero no instalado ni probado en un sistema limpio. No instala el script de Nautilus.
- Pendiente: ruedas de acciones de archivo, reproducir audio y vídeo dentro del editor de recorte, y herramientas para varios archivos o para documentos.

## Contribuir

Ver [CONTRIBUTING.md](CONTRIBUTING.md) (en inglés, con un resumen en español al final). La documentación técnica está en [docs/](docs/Inicio.md), la bóveda de Obsidian del autor.

## Licencia

[MIT](LICENSE). Puedes usar, copiar, modificar y distribuir Mandarina, siempre que conserves el aviso de copyright.

## Agradecimientos e inspiración

Mandarina se inspira en el comportamiento de Tangerine for Mac (Leap Studio): una rueda de formatos que aparece junto al cursor. Mandarina es un proyecto independiente: no está afiliada a Leap Studio ni a Tangerine, y no copia su diseño, su icono, sus textos ni sus recursos. Todo el código, el diseño y el icono son propios.

Construida con [Tauri](https://tauri.app), ffmpeg, ImageMagick, pandoc, LibreOffice y poppler.
