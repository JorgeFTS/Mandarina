# Comandos Tauri

Funciones de Rust marcadas con `#[tauri::command]` en `src-tauri/src/lib.rs`, registradas en `invoke_handler`. El JS las llama con `window.__TAURI__.core.invoke(...)` (`withGlobalTauri` está activado, no hay bundler).

Delegan el trabajo en [[Conversores]] y los usa la [[Rueda]].

## Contrato

### `get_targets(path: String) -> Vec<String>`
Formatos a los que se puede convertir el archivo, en el orden en que aparecen en la rueda. Lista vacía si el tipo no está soportado. Excluye el formato del propio archivo.

### `convert(path: String, format: String) -> Result<String, String>`
Convierte `path` a `format` (uno de los devueltos por `get_targets`). El archivo de salida se crea junto al original con el mismo nombre y la nueva extensión; si ya existe, se añade ` 2`, ` 3`... Devuelve la ruta de salida. En error devuelve un mensaje corto y legible para mostrar en la interfaz (por ejemplo, la herramienta que falta y cómo instalarla). Es asíncrono: no debe bloquear la interfaz.

### Comandos de la rueda flotante (nuevos)

La app se lanza de dos formas: `mandarina /ruta/archivo` (desde el clic derecho de Nautilus) o sin argumentos (ventana pequeña a la que se arrastra un archivo). La ventana es transparente, sin bordes y siempre encima; la rueda se dibuja en el centro y la zona transparente de alrededor cierra la app al hacer clic.

- `initial_path() -> Option<String>`: primer argumento de la línea de comandos si es una ruta existente (archivo o carpeta), si no `None`.
- `file_info(path: String) -> Result<FileInfo, String>` con `FileInfo { name: String, size: u64, is_dir: bool }` (`size` en bytes; para carpetas, suma recursiva).
- `show_wheel() -> ()`: muestra la ventana (que arranca oculta). Si la app se lanzó con archivos, la **agranda a mano hasta cubrir el monitor principal** (`set_position` + `set_size`, sin `set_fullscreen`) y transparente, solo para que la página reciba el cursor; si no, la centra.
- `place_wheel(x: f64, y: f64) -> ()` (async): fija 460x460 y coloca la ventana con su centro en (x, y), la posición del cursor en píxeles de la página, dentro del monitor. Cambia el tamaño, espera 60 ms y repite la posición tres veces (`resize_and_move`): sin esa pausa el gestor de ventanas ignora el movimiento y la ventana se queda donde estaba. Es async para poder dormir sin bloquear el hilo principal. Ver [[Rueda]].
- `get_language() -> String`: el código del idioma activo (`"en"`, `"es"`, `"pt"`, `"fr"`; ver [[Idiomas]]); la interfaz lo pide antes de pintar nada.
- **Códigos de error estables:** los errores de cancelación y de "ya está optimizado" llegan como `"[cancelled] <texto>"` y `"[optimized] <texto>"` (el texto va en el idioma activo). La interfaz decide por el código, nunca por el texto.
- `center_wheel() -> ()` (async, usa `resize_and_move`): fija 460x460 y centra la ventana en el monitor (para las tarjetas de mensajes).
- `cancel() -> ()`: cancela la conversión en curso (mata el proceso hijo y borra la salida a medias). La `convert` pendiente termina con `Err("Cancelado")`.
- `quit() -> ()`: cierra la app.
- `get_targets` también acepta carpetas: destinos `zip`, `tar`, `tar.gz` (comprimir la carpeta).

### Progreso real y miniatura (nuevos)

- **Evento `convert-progress`** (lo emite el backend con `app.emit` durante `convert`; la interfaz lo escucha con `window.__TAURI__.event.listen`). Payload: `{ "percent": number }`, de 0 a 100, creciente, como máximo unas 10 veces por segundo. Solo se emite en conversiones de ffmpeg (audio, vídeo, subtítulos) cuando se conoce la duración (con `ffprobe`); en el resto (ImageMagick, LibreOffice, pandoc...) no se emite nada y la interfaz mantiene la barra simulada. No se emite 100: lo marca el fin de `convert`. `convert` conserva la misma firma desde JS (`path`, `format`).
- `thumbnail(path: String) -> Result<String, String>`: miniatura de unos 200 px de lado como **data URL** (`data:image/jpeg;base64,...`). Imágenes y SVG con ImageMagick/rsvg-convert, vídeo con un fotograma de ffmpeg, PDF y presentaciones con la primera página (pdftoppm; presentaciones, documentos y hojas no: solo PDF). Para cualquier otro tipo, o si tarda más de 3 s, devuelve `Err`. Es async, no usa el flag de cancelar y la interfaz debe llamarla sin bloquear la rueda: primero se muestra el icono y luego se sustituye por la miniatura si llega.

### Varios archivos a la vez (nuevos)

La app recibe todos los archivos seleccionados (`mandarina ruta1 ruta2 ...`). Los comandos de un solo archivo (`get_targets`, `convert`, `file_info`, `thumbnail`) siguen igual; estos se suman:

- `initial_paths() -> Vec<String>`: **sustituye a `initial_path`** (se elimina). Todos los argumentos que no empiecen por `-`, como rutas absolutas, que existan, sin duplicados y en el orden recibido. Lista vacía si no hay ninguno.
- `get_targets_many(paths: Vec<String>) -> Vec<String>`: formatos para la rueda. Solo cuentan los archivos de un tipo soportado (los demás se ignoran aquí y aparecerán como fallidos al convertir). Un formato se ofrece si **todos** esos archivos lo admiten según la lista de destinos de su tipo (sin contar el formato propio) y **no todos** ya están en ese formato. Orden: el de la lista del primer archivo soportado. Con un solo archivo devuelve lo mismo que `get_targets`. Lista vacía si ningún archivo es soportado o no hay formatos en común.
- `convert_many(paths: Vec<String>, format: String) -> Result<BatchResult, String>` (async). Convierte de uno en uno, cada resultado junto a su original (mismas reglas de nombres y de carpeta de páginas que `convert`). Los que ya están en ese formato se saltan. Si un archivo falla, sigue con el resto. `BatchResult { outputs: Vec<String>, skipped: Vec<String> (nombres), failed: Vec<{ name: String, error: String }> }`. Solo da `Err` si la lista está vacía o si se cancela (`Err("Cancelado")`; los resultados ya creados se conservan). Un archivo de tipo no soportado entra en `failed` con "Tipo de archivo no soportado".
- Eventos durante `convert_many`:
  - `convert-file` `{ "index": number, "total": number, "name": string }` al empezar cada archivo que se va a convertir (`index` desde 1; `total` = archivos a convertir, sin los saltados ni los no soportados).
  - `convert-progress` `{ "percent": number }` **global**: `(archivos terminados + porcentaje del actual / 100) / total * 100`, creciente, máximo 99, como mucho unas 10 veces por segundo. Si el archivo actual no informa de su avance, el porcentaje solo salta al terminar cada archivo.
- La interfaz usa `get_targets_many` siempre; para convertir, `convert` con 1 archivo (sin cambios) y `convert_many` con más de uno.

### Herramientas (nuevos; diseño en [[Herramientas]])

Tipos serializados (JSON):
- `ToolInfo { id: String, label: String, options: Vec<ToolOption>, editor: Option<String> }` con `editor` = `"crop"` (recuadro sobre una imagen) o `"trim"` (inicio y fin en tiempo) o `null`. `label` va en mayúsculas cortas para la rueda (p. ej. `"REDUCIR TAMAÑO"`).
- `ToolOption { id: String, label: String }`.
- `MediaInfo { kind: String ("imagen" | "audio" | "video"), width: Option<u32>, height: Option<u32>, duration: Option<f64> }`. Las medidas de una imagen son **después de aplicar la orientación EXIF** (`-auto-orient`); el recorte se aplica sobre esa imagen orientada.

Comandos:
- `get_tools(path: String) -> Vec<ToolInfo>` (sin error: lista vacía si no hay herramientas). Por tipo: imagen → `compress`, `strip_metadata`, `trim` (editor `crop`), `rotate`, `flip`, `resize`; vídeo → `compress`, `strip_metadata`, `trim` (editor `trim`), `rotate`, `flip`, `resize`, `mute`; audio → `compress`, `strip_metadata`, `trim` (editor `trim`); pdf → `compress`; el resto (documentos, hojas, subtítulos...) → solo `zip`; carpetas y comprimidos → vacía. Todos los tipos con herramientas terminan con `zip` (`run_tool` lo resuelve con `pack`). Labels: REDUCIR TAMAÑO, METADATOS, RECORTAR, GIRAR, VOLTEAR, REDIMENSIONAR, SIN AUDIO, ZIP. Opciones: `rotate` → `left` ("90° ↺"), `right` ("90° ↻"), `180` ("180°"); `flip` → `horizontal`, `vertical`; `resize` → ids `pct:75`, `pct:50`, `pct:25` ("75 %", "50 %", "25 %") y, en imagen, `w:1920`, `w:1280`, `w:800` ("1920 px"...) y, en vídeo, `h:1080`, `h:720`, `h:480` ("1080p"...). Máximo 8 opciones.
- `run_tool(path: String, tool: String, option: Option<String>, params: Option<serde_json::Value>) -> Result<String, String>` (async; recibe además el `AppHandle` por dentro): ejecuta la herramienta y devuelve la **ruta del archivo de salida**. `params` solo para `trim`: audio/vídeo `{ "start": f64, "end": f64 }` en segundos (0 ≤ start < end ≤ duración); imagen (`crop`) `{ "x": u32, "y": u32, "w": u32, "h": u32 }` en píxeles de la imagen orientada, dentro de los límites. Opcionalmente, `params` puede llevar `"nombre": string`: el nombre del archivo nuevo **sin extensión** (la salida conserva la del original; si el usuario la escribe, se quita). Se limpia (espacios en los extremos) y se rechaza (`Err`) si queda vacío, lleva `/` o caracteres de control, es `.` o `..` o pasa de ~230 caracteres. Si ya existe un archivo con ese nombre se numera (`nombre 2.ext`), nunca se sobrescribe. Sin `nombre`, el nombre es el de siempre (`foto (recortado).jpg`). Valida y devuelve `Err` legible si algo no cuadra. Las de ffmpeg emiten `convert-progress` `{percent}` (0 a 99) y todas se cancelan con `cancel` (`Err("Cancelado")`, sin dejar salida a medias).
- `media_info(path: String) -> Result<MediaInfo, String>`.
- `preview_image(path: String, max_side: u32) -> Result<String, String>`: imagen orientada reducida a `max_side` px como data URL JPEG (para el editor de recorte). Máx. 3 s de espera de proceso.
- `video_frame(path: String, seconds: f64, max_side: u32) -> Result<String, String>`: un fotograma como data URL JPEG.
- `waveform(path: String, buckets: u32) -> Result<Vec<f32>, String>`: picos de amplitud entre 0.0 y 1.0, uno por cubo, para dibujar la forma de onda (de ~100 a 400 cubos).
- `resize_center(width: u32, height: u32) -> ()` (async, usa `resize_and_move`): pone la ventana con ese tamaño **lógico** centrada en el monitor (para el editor). `center_wheel` sigue devolviendo 460x460 para las tarjetas.

Los editores no usan `convert`. Nombres de salida de las herramientas: ver [[Herramientas]].

### Contrato del editor visual (JS)

Los archivos `src/editor.js` y `src/editor.css` los escribe otro agente que no toca `main.js`, `index.html` ni `styles.css`; `main.js` carga esos dos archivos y llama a:

```js
// Devuelve una promesa: null si se cancela, o los params de run_tool si se aplica
// ({x,y,w,h} para imagen, {start,end} para audio y vídeo, y siempre `nombre`: el texto del
// campo "Nombre del archivo nuevo", sin extensión)
window.Editor.abrir({
  tipo: "imagen" | "audio" | "video",
  ruta: "/ruta/archivo",
  nombre: "archivo.ext",
  contenedor: HTMLElement,   // el editor se dibuja aquí dentro y lo vacía al terminar
  api: {                      // funciones que devuelven promesas, con argumentos posicionales
    media_info(path),
    preview_image(path, maxSide),
    video_frame(path, seconds, maxSide),
    waveform(path, buckets),
  },
}) -> Promise<object | null>
```

El contenedor mide unos 740x560 px (la ventana es de 780x640). `Editor` dibuja sus propios botones **Aplicar** y **Cancelar**, soporta Escape (cancelar) y Enter (aplicar), modo claro/oscuro y teclado accesible (flechas mueven el extremo enfocado). No llama a `run_tool` ni cierra la ventana: eso lo hace `main.js`.

Nombres de salida: junto al original, `nombre.ext`; si existe, `nombre 2.ext`, `nombre 3.ext`... (con espacio, como en Mac; ya no `_1`). Las carpetas de páginas siguen `nombre_pages`, `nombre_pages 2`...

## Formatos

La tabla completa de tipos y destinos está en [[Conversores]]. Para pdf, documento y presentación, jpg/png con varias páginas genera una carpeta de páginas y `convert` devuelve la ruta de la carpeta; con una sola página devuelve un archivo suelto.

## Arrastrar y soltar

Tauri 2 intercepta el drop del sistema: el HTML5 `drop` no da la ruta. Se usa `getCurrentWebview().onDragDropEvent` (en global: `window.__TAURI__.webview.getCurrentWebview()`), que da `paths` y la posición en píxeles físicos (dividir entre `devicePixelRatio`).

Estado: `get_targets` y `convert` implementados en `src-tauri/src/converters.rs` (ver [[Conversores]]). El comando `greet` de la plantilla ya se borró.

## Configuración (ver [[Configuracion]])

- `get_config() -> Vec<KindFormats>`: por cada tipo configurable (`image`, `audio`, `video`, `pdf`, `document`, `spreadsheet`) devuelve `{ key, all, enabled, min, max }`: todos los formatos posibles, los visibles ahora en su orden, y los límites (3 y 8).
- `set_formats(kind: String, formats: Vec<String>) -> Result<(), String>`: guarda los formatos visibles de un tipo; error si el tipo o un formato no existe, o si no son entre 3 y 8.
- `reset_formats(kind: String) -> Result<(), String>`: vuelve a la lista de siempre de ese tipo.
- `get_prefs() -> { language: "auto"|código, languages: [{ code, name }], accent: String, accents: Vec<String> }`: idioma y color guardados, los idiomas disponibles (de `LANGUAGES`) y los colores posibles.
- `set_language(value: String)` (`"auto"` o un código de idioma; se aplica al momento con `lang::reload`) y `set_accent(value: String)` (uno de `accents`): guardan el ajuste; error si el valor no existe.
- Efecto: `get_targets` y `get_targets_many` ya devuelven solo los formatos elegidos.
