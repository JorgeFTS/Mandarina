<p align="center">
  <img src="packaging/icon/mandarina.png" alt="Logo de Mandarina" width="140">
</p>

<h1 align="center">Mandarina</h1>

<p align="center">
  Conversor de archivos para Linux con una rueda radial flotante.<br>
  Todo se hace en tu equipo; no se sube nada a internet.
</p>

<p align="center">
  <a href="LICENSE"><img alt="Licencia: MIT" src="https://img.shields.io/badge/licencia-MIT-orange.svg"></a>
  <img alt="Plataforma: Linux y macOS (beta)" src="https://img.shields.io/badge/plataforma-Linux%20%7C%20macOS%20(beta)-blue.svg">
  <img alt="Hecho con Tauri 2" src="https://img.shields.io/badge/hecho%20con-Tauri%202-24C8DB.svg">
  <img alt="Versión 0.1.1" src="https://img.shields.io/badge/versi%C3%B3n-0.1.1-lightgrey.svg">
</p>

<p align="center">
  <a href="README.md">English</a> | Español
</p>

Seleccionas uno o varios archivos en Nautilus, pulsas un atajo o haces clic derecho, y aparece una rueda junto al cursor con los formatos a los que se pueden convertir. Eliges uno y la conversión se hace en tu equipo.

<p align="center">
  <img src="docs/images/mandarina-demo.gif" alt="Demostración: seleccionar un archivo en Nautilus, pulsar Alt+M, elegir un formato en la rueda y ver el archivo convertido junto al original" width="720">
</p>

> **Estado: beta (versión 0.1.1).** Funciona y el `.deb` se probó en sistemas limpios de Debian y Ubuntu, pero la app solo se ha usado en un escritorio real con Ubuntu y GNOME. Lee [Estado del proyecto](#estado-del-proyecto) antes de usarlo.

## ¿Qué puede convertir?

**Diez tipos de archivo y más de 250 conversiones**, todo en tu equipo. Eliges un archivo y la rueda muestra solo los formatos en los que ese archivo se puede convertir.

| | Tipo | Desde | A |
|---|---|---|---|
| 🖼️ | **Imágenes** | `jpg` `png` `webp` `heic*` `tiff` `avif` `bmp` `gif` `svg` | `jpg` `png` `webp` `tiff` `avif` `bmp` `pdf` |
| 🎵 | **Audio** | `mp3` `m4a` `wav` `flac` `ogg` `opus` `aiff` `wma` | `mp3` `m4a` `wav` `flac` `ogg` `opus` `aiff` `wma` |
| 🎬 | **Vídeo** | `mp4` `mov` `mkv` `webm` `avi` `wmv` | `mp4` `mov` `mkv` `webm` `avi` `wmv` `gif` `mp3` |
| 📄 | **PDF** | `pdf` | `docx` `jpg` `png` `epub` `txt` `md` |
| 📝 | **Documentos** | `docx` `doc` `odt` `rtf` `md` `txt` `epub` `html` | `docx` `md` `txt` `epub` `html` `pdf` `jpg` `png` |
| 📊 | **Presentaciones** | `pptx` `ppt` `odp` | `pdf` `jpg` `png` |
| 📈 | **Hojas de cálculo** | `xlsx` `xls` `ods` `csv` `tsv` | `xlsx` `ods` `csv` `tsv` `pdf` |
| 💬 | **Subtítulos** | `srt` `vtt` | `srt` `vtt` `txt` |
| 🗜️ | **Comprimidos** | `zip` `tar` `tar.gz` `rar*` | `zip` `tar` `tar.gz` |
| 📁 | **Carpetas** | `cualquier carpeta` | `zip` `tar` `tar.gz` |

\* `heic` y `rar` solo se pueden leer: Mandarina convierte *desde* ellos, pero no puede crearlos. Algunos destinos dependen de cómo se compiló tu ImageMagick o ffmpeg. El programa que hace cada conversión está en [docs/Converters.md](docs/Converters.md).

Además de los formatos, la [rueda de herramientas](#herramientas) puede reducir el tamaño, quitar metadatos, recortar, girar, voltear, redimensionar y quitar el audio, y varios archivos a la vez se pueden empaquetar en un solo ZIP.

## Contenido

- [¿Qué puede convertir?](#qué-puede-convertir)
- [Instalación](#instalación)
- [Uso](#uso)
- [Herramientas](#herramientas)
- [Ajustes](#ajustes)
- [Idioma](#idioma)
- [Limitaciones y problemas conocidos](#limitaciones-y-problemas-conocidos)
- [Estado del proyecto](#estado-del-proyecto)
- [Contribuir](#contribuir)
- [Colaboradores](#colaboradores)
- [Licencia](#licencia)

## Instalación

Cada [release](https://github.com/JorgeFTS/Mandarina/releases/tag/v0.1.1-beta) trae un paquete por sistema:

| Sistema | Archivo | Compruébalo con |
|---|---|---|
| Linux (Debian, Ubuntu, Linux Mint) | `Mandarina_0.1.1_amd64.deb` | `SHA256SUMS` |
| macOS (Apple Silicon e Intel, beta) | `Mandarina-0.1.1.dmg` | `Mandarina-0.1.1.dmg.sha256` |

`SHA256SUMS` lista los dos archivos, así que si descargaste solo uno, ejecuta `sha256sum -c --ignore-missing SHA256SUMS`. Los pasos para Linux están abajo; para macOS mira [macOS (beta)](#macos-beta).

Copia y pega cada bloque en una terminal.

### Debian, Ubuntu, Linux Mint y derivadas (recomendado)

```bash
wget https://github.com/JorgeFTS/Mandarina/releases/download/v0.1.1-beta/Mandarina_0.1.1_amd64.deb
sudo apt install ./Mandarina_0.1.1_amd64.deb
```

Eso instala la app y las herramientas de conversión que usa (unos 600 a 700 MB en un sistema vacío, sobre todo LibreOffice; mucho menos en un escritorio normal, donde la mayoría ya están instaladas). Después abre **Mandarina** desde el menú de aplicaciones, o ejecuta `mandarina` y arrastra un archivo encima.

Para tener el clic derecho y el atajo **Alt+M** en **Nautilus** (GNOME, Ubuntu), ejecuta esto y reinicia Nautilus (`nautilus -q` cierra sus ventanas):

```bash
bash /usr/share/mandarina/install-nautilus.sh
nautilus -q
```

El instalador solo lo ejecutas tú; no toca tus archivos por sí solo. Configura Nautilus y, en Linux Mint, Nemo (clic derecho > *Convertir con Mandarina*; reinícialo con `nemo -q`; para elegir uno, `MANDARINA_FM=nemo bash /usr/share/mandarina/install-nautilus.sh`). La entrada de Nemo se escribió a partir de su documentación y aún no se probó en un Mint real. Dolphin y Thunar todavía no están soportados.

Para omitir las herramientas de conversión e instalarlas después, como pide la app, usa `sudo apt install --no-install-recommends ./Mandarina_0.1.1_amd64.deb`.

El paquete se instaló y probó en contenedores limpios de Ubuntu 22.04, 24.04 y 26.04 y Debian 12 y 13 (ver [docs/Publishing.md](docs/Publishing.md)), pero todavía no en todos los escritorios reales. Linux Mint no se ha probado.

### Otras distribuciones (Arch, CachyOS, Fedora...): compilar desde el código

Todavía no hay paquete para ellas, así que se compila. Debería funcionar con los paquetes equivalentes, pero no se ha comprobado. Instala antes [Rust](https://rustup.rs) (1.85 o superior) y [Node.js](https://nodejs.org) con npm. Los nombres de paquete de abajo son los de Debian y Ubuntu.

```bash
sudo apt install ffmpeg imagemagick librsvg2-bin pandoc libreoffice poppler-utils ghostscript zip unzip 7zip
sudo apt install build-essential curl wget file libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
git clone https://github.com/JorgeFTS/Mandarina.git
cd Mandarina
npm install
npm run tauri build -- --no-bundle
```

La primera línea instala las herramientas de conversión (si falta alguna, solo fallan los formatos que la usan, y la app te dice qué paquete instalar) y la segunda las de compilación. El binario queda en `src-tauri/target/release/mandarina`. La primera compilación tarda unos minutos (usa LTO). Hay que recompilar cada vez que cambie `src/`, porque el binario lleva la interfaz incluida. Para una compilación más rápida y más grande usa `npm run tauri build -- --debug --no-bundle` (el binario queda en `src-tauri/target/debug/`), y `npm run tauri dev` para el modo desarrollo.

Para el clic derecho y el atajo en Nautilus:

```bash
packaging/install-nautilus.sh src-tauri/target/release/mandarina
nautilus -q
```

El instalador copia un script a `~/.local/share/nautilus/scripts/` y añade el atajo `Alt+M` a `~/.config/nautilus/scripts-accels`. El script se llama según el idioma activo (ver [Idioma](#idioma)): `Convertir con Mandarina`, `Convert with Mandarina`, `Converter com Mandarina` o `Convertir avec Mandarina`. Si había una entrada de otro idioma instalada, el instalador la borra para que quede una sola, y se puede ejecutar de nuevo en cualquier momento. Para otro atajo: `MANDARINA_ATAJO="<Control><Alt>m" packaging/install-nautilus.sh src-tauri/target/release/mandarina`.

Para crear tu propio `.deb`: `npm run tauri build -- --bundles deb` (queda en `src-tauri/target/release/bundle/deb/`).

### macOS (beta)

El soporte para macOS lo aportó [@justindurazzo](https://github.com/justindurazzo) y es más nuevo que el de Linux: el CI ejecuta `cargo test` en macOS. Para una descarga lista, usa el `.dmg` universal (Apple Silicon + Intel) de la [última release para macOS](https://github.com/JorgeFTS/Mandarina/releases/tag/v0.1.1-beta), que construye GitHub Actions con cada tag `v*`; compruébalo con el archivo `.sha256` que está a su lado. [Satsuma](https://github.com/justindurazzo/satsuma/releases/latest) es una alternativa: el port de Mandarina para macOS que mantiene el mismo colaborador, publicado por él y no por este proyecto, así que los problemas de ese DMG repórtalos en [su repositorio](https://github.com/justindurazzo/satsuma/issues). Para compilarlo tú, `packaging/macos/build-dmg.sh` genera el mismo `.dmg`. En todos los casos abres el `.dmg` y ejecutas el instalador desde la Terminal como explica el Léeme de su interior. El instalador copia la app, añade una Acción rápida de Finder (**Convert with Mandarina**; la app también aparece en el menú **Abrir con** de Finder) y ofrece instalar los conversores con [Homebrew](https://brew.sh). La app no está firmada con un Apple Developer ID ni notarizada, así que el instalador quita de su propia copia la marca de "descargado de internet"; los ajustes se guardan en `~/Library/Application Support/mandarina/`. En [docs/macOS.md](docs/macOS.md) (en inglés) se explica qué significa que no esté notarizada y está la lista de pruebas manuales.

### Desinstalar

Borra el script de `~/.local/share/nautilus/scripts/` y su línea en `~/.config/nautilus/scripts-accels`. Si instalaste el `.deb`, ejecuta `sudo apt remove mandarina`. Tus ajustes están en `~/.config/mandarina/`; borra esa carpeta para eliminarlos también.

## Uso

1. Selecciona uno o varios archivos (o una carpeta) en Nautilus.
2. Pulsa **Alt+M**, o haz clic derecho y elige **Scripts → Convertir con Mandarina** (con el nombre según el idioma, por ejemplo **Convert with Mandarina** en inglés; ver [Idioma](#idioma)).
3. Aparece la rueda junto al cursor, con un segmento por formato de destino.
4. Elige un formato (clic, o flechas y Enter). Una tarjeta muestra el progreso y se puede cancelar.
5. El resultado queda en la misma carpeta que el original con un nombre nuevo (`foto.png` → `foto.jpg`). Nunca se sobrescribe nada: si ya existe, se crea `foto 2.jpg`, `foto 3.jpg`...

Escape cierra la rueda. También puedes lanzar `mandarina` sin argumentos para arrastrar un archivo sobre la ventana.

<p align="center">
  <img src="docs/images/wheel.png" alt="La rueda de conversión junto al cursor, con un segmento por formato de destino" width="380">
</p>

Las conversiones las hacen programas del sistema (ffmpeg, ImageMagick, pandoc, LibreOffice, poppler...); Mandarina los llama desde Rust sin pasar por un intérprete de comandos.

## Herramientas

Con **un solo archivo**, el último segmento de la rueda es **HERRAMIENTAS** y abre una segunda rueda:

| Tipo | Herramientas |
|---|---|
| Imagen | reducir tamaño, quitar metadatos, recortar (con recuadro visual), girar, voltear, redimensionar |
| Vídeo | reducir tamaño, quitar metadatos, recortar (con tira de fotogramas), girar, voltear, redimensionar, quitar el audio |
| Audio | reducir tamaño, quitar metadatos, recortar (con forma de onda) |
| PDF | reducir tamaño |
| Cualquier otro archivo | zip |

<p align="center">
  <img src="docs/images/tools-wheel.png" alt="La rueda de herramientas: reducir tamaño, quitar metadatos, recortar, girar, voltear, redimensionar y comprimir en ZIP" width="340">
</p>

<p align="center">
  <img src="docs/images/tool-crop-image.png" alt="El editor de imagen: recorte con una caja visual, proporciones y el tamaño del resultado" width="420">
  &nbsp;
  <img src="docs/images/tool-trim-video.png" alt="El editor de vídeo: una tira de fotogramas con dos asas para recortar y la duración que se conserva" width="420">
</p>

El resultado va junto al original, con el mismo formato y un sufijo: `foto (comprimido).jpg`, y del mismo modo `(sin metadatos)`, `(recortado)`, `(girado)`, `(volteado)`, `(redimensionado)` y `(sin audio)`. Si comprimir no reduce al menos un 2 %, avisa en vez de crear un archivo casi igual. Wav, flac, aiff y bmp no ofrecen reducir tamaño. Reducir tamaño no es zipear: la herramienta ZIP empaqueta cualquier archivo en un `.zip`.

Los editores de recorte (imagen, audio y vídeo) incluyen un campo con el nombre del archivo nuevo. Empieza como `foto (recortado)`, junto a él se muestra la extensión del original y se valida mientras escribes. Comprimir un PDF necesita ghostscript. Más detalles en [docs/Tools.md](docs/Tools.md) (en inglés).

## Ajustes

El engranaje que aparece en la parte baja del círculo central abre los ajustes: los formatos que muestra cada rueda (entre 3 y 8 por tipo), el idioma y el color de acento.

<p align="center">
  <img src="docs/images/settings.png" alt="La pantalla de ajustes: idioma, color de acento y los formatos de cada tipo" width="420">
</p>

## Idioma

Mandarina está disponible en **inglés, español, portugués (Português) y francés (Français)**. Puedes elegir el idioma en los ajustes (el engranaje que aparece en la parte baja del círculo central de la rueda) o dejarlo en **Automático**. El cambio se aplica al momento.

En Automático, el idioma se elige una sola vez al arrancar, a partir de las variables de entorno del sistema, en este orden: `MANDARINA_LANG` (ver abajo), el idioma guardado en los ajustes, `LC_ALL`, `LC_MESSAGES`, `LANGUAGE` (el primer idioma de la lista) y `LANG`. Si el valor empieza por `es`, `pt` o `fr`, la app usa ese idioma; en cualquier otro caso, o si no hay ninguna variable, usa inglés (por ejemplo, un sistema en chino verá la app en inglés).

Para forzar un idioma, define `MANDARINA_LANG` como `en`, `es`, `pt` o `fr`, que tiene prioridad sobre las demás:

```bash
MANDARINA_LANG=pt mandarina
```

El idioma se aplica a la interfaz, los mensajes de error, los nombres de los archivos que crean las herramientas (`foto (comprimido).jpg` en español, `photo (compressed).jpg` en inglés, `foto (compressé).jpg` en francés) y la entrada del menú de Nautilus (ver más abajo). Los mensajes de ffmpeg, ImageMagick y los demás programas se muestran tal cual. Las traducciones al portugués y al francés se escribieron sin revisión de un hablante nativo, así que las correcciones son bienvenidas (ver `CONTRIBUTING.md` para añadir o corregir un idioma).

## Limitaciones y problemas conocidos

- **Wayland:** una aplicación no puede conocer la posición del cursor ni detectar arrastres o teclas como Shift fuera de su ventana. Por eso el script de Nautilus lanza la app con `GDK_BACKEND=x11` (XWayland) y la ventana se coloca junto al cursor en dos pasos. Sin `GDK_BACKEND=x11` no se coloca junto al cursor.
- Con **varios archivos**, la rueda ofrece solo los formatos que todos admiten (por ejemplo, 5 imágenes dan los formatos de imagen; una imagen y un documento solo `pdf`, `jpg` y `png`). Los que ya están en ese formato se omiten, y si uno falla se siguen convirtiendo los demás. Las carpetas se pueden comprimir. Con varios archivos de cualquier tipo, la rueda siempre ofrece ZIP y los empaqueta todos en un solo `Archivo.zip`. La rueda de herramientas solo está disponible con un archivo.
- **Gestores de archivos:** Nautilus y Nemo en Linux, y una Acción rápida de Finder en macOS. Todavía no hay integración con Dolphin ni Thunar.
- **GNOME 50 impide que la app arranque.** Con GNOME 50, WebKitGTK/GTK 3.24 aborta con `Settings schema 'org.gnome.settings-daemon.plugins.xsettings' does not contain a key named 'antialiasing'`, porque GNOME movió esas claves a un esquema `.deprecated`. Es un problema del sistema, no de Mandarina. Arreglo local sin `sudo`: copiar el esquema `xsettings` con las claves `antialiasing`, `hinting` y `rgba-order` añadidas, junto con `org.gnome.settings-daemon.enums.xml`, a `~/.local/share/glib-2.0/schemas/` y ejecutar `glib-compile-schemas` en esa carpeta. Bórralo cuando GNOME lo corrija. Detalles en [docs/Decisions.md](docs/Decisions.md) (en inglés).
- **VS Code instalado con snap:** si lanzas la app desde su terminal integrada, hereda variables de entorno del snap (`GTK_PATH`, `GIO_MODULE_DIR`, `GSETTINGS_SCHEMA_DIR`...) que hacen fallar a WebKitGTK. Lánzala desde otra terminal o limpia esas variables antes.
- Archivos de nombre raro, permisos o conversiones muy pesadas (vídeo largo, documentos enormes) no se han probado de forma sistemática.

## Estado del proyecto

Funciona y está probado:

- Los conversores y herramientas de Rust: `cargo test` ejecuta pruebas unitarias y de integración con ffmpeg, ImageMagick, pandoc, poppler, LibreOffice y ghostscript (las de integración se saltan si falta la herramienta).
- A mano: md a pdf/png/docx (incluidas las tablas de Markdown con líneas separadoras largas, cuyas columnas toman un ancho según su contenido), docx a pdf, doc a md/pdf, pptx a pdf/jpg, tsv a xlsx/csv, pdf a docx/md/epub/txt, y mp4 a webm (unas 1,8 veces más rápido que la primera versión, con la misma calidad). Un PDF a Word de un CV con una banda de color mantiene el texto visible encima (comprobado en LibreOffice, no en Word).
- El `.deb`: instalado, iniciado, usado para convertir una foto y desinstalado en contenedores limpios de Ubuntu 22.04, 24.04 y 26.04 y Debian 12 y 13, e instalado en un escritorio real con Ubuntu.

No verificado o pendiente:

- La rueda en la ventana real: posición junto al cursor, transparencia en pantalla, arrastre real y cancelar. Las capturas de diseño solo prueban la interfaz, no Tauri.
- Sin probar: rar, extracción de zip solo con 7z, SVG, vídeo a GIF, los avisos de "falta la herramienta" y archivos reales .xls, .ods y .odp.
- Solo se ha usado en un escritorio real con Ubuntu y GNOME 50 (Wayland). No hay pruebas en X11 puro, en otras distribuciones (el `.deb` no se instala en Arch, Fedora ni similares) ni en otros entornos de escritorio.
- El `.deb` (2,6 MB) no instala el script de Nautilus por sí solo; ejecutas `bash /usr/share/mandarina/install-nautilus.sh`.
- Un archivo Markdown muestra sus imágenes solo si están junto a él (si falta la carpeta `assets/`, el resultado sale sin imágenes y la app todavía no lo avisa).
- **macOS** está en beta: `cargo test` pasa en el CI y la Acción rápida se probó en Apple Silicon y Abrir con solo abriendo un archivo desde la línea de comandos, pero todavía no se ha probado un segundo monitor ni un Mac Intel real (lista en [docs/macOS.md](docs/macOS.md)). El `ffmpeg` normal de Homebrew no trae el codificador Vorbis, así que Mandarina pide `ffmpeg-full`.
- Pendiente: ruedas de acciones de archivo, reproducir audio y vídeo dentro del editor de recorte, y herramientas para varios archivos o para documentos.

## Contribuir

Ver [CONTRIBUTING.md](CONTRIBUTING.md) (en inglés). La documentación técnica está en [docs/](docs/Home.md) (en inglés): una nota corta por módulo.

## Colaboradores

- [@justindurazzo](https://github.com/justindurazzo): soporte para macOS (Acción rápida de Finder, instalador DMG, CI en macOS) y el port [Satsuma](https://github.com/justindurazzo/satsuma).

## Licencia

[MIT](LICENSE). Puedes usar, copiar, modificar y distribuir Mandarina, siempre que conserves el aviso de copyright.

## Agradecimientos

Todo el código, el diseño y el icono son propios. Construida con [Tauri](https://tauri.app), ffmpeg, ImageMagick, pandoc, LibreOffice y poppler.
