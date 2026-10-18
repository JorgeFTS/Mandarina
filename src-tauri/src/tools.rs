use crate::converters::{
    category, get_ext, get_stem, move_file, need, pack, probe_duration, run_in, run_progress, st, unique_path,
    Kind, Progress, TempDir, CANCEL,
};
use crate::lang::{has_code, optimized_error, tr, CANCELLED, OPTIMIZED};
use crate::thumbnail::{base64, capture, capture_with, timeout_message};
use crate::tr;
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

// Tiempo máximo de espera de media_info, preview_image y video_frame, y de waveform
const QUICK_WAIT: Duration = Duration::from_secs(5);
const WAVE_WAIT: Duration = Duration::from_secs(20);
// La forma de onda se calcula con audio mono de 8 kHz y 16 bits
const WAVE_RATE: f64 = 8000.0;
// Un resultado de "comprimir" solo se conserva si pesa menos del 98 % del original
const MIN_SAVING_PERCENT: u64 = 98;

#[derive(Serialize, Debug, PartialEq)]
pub struct ToolOption {
    id: String,
    label: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ToolInfo {
    id: String,
    label: String,
    options: Vec<ToolOption>,
    // "crop" (recuadro sobre una imagen), "trim" (inicio y fin en tiempo) o None
    editor: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct MediaInfo {
    // "imagen", "audio" o "video"
    kind: String,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<f64>,
}

// ---------- Tabla de herramientas ----------

fn tool(id: &str, label: &str, options: Vec<ToolOption>, editor: Option<&str>) -> ToolInfo {
    ToolInfo { id: id.into(), label: label.into(), options, editor: editor.map(String::from) }
}

fn options(list: &[(&str, &str)]) -> Vec<ToolOption> {
    list.iter().map(|(id, label)| ToolOption { id: id.to_string(), label: label.to_string() }).collect()
}

fn rotate_tool() -> ToolInfo {
    tool("rotate", &tr("ROTATE", "GIRAR"), options(&[("left", "90° ↺"), ("right", "90° ↻"), ("180", "180°")]), None)
}

fn flip_tool() -> ToolInfo {
    tool("flip", &tr("FLIP", "VOLTEAR"), options(&[("horizontal", "Horizontal"), ("vertical", "Vertical")]), None)
}

// Las imágenes se redimensionan por ancho y los vídeos por alto
fn resize_tool(video: bool) -> ToolInfo {
    let mut list = vec![("pct:75", "75 %"), ("pct:50", "50 %"), ("pct:25", "25 %")];
    if video {
        list.extend([("h:1080", "1080p"), ("h:720", "720p"), ("h:480", "480p")]);
    } else {
        list.extend([("w:1920", "1920 px"), ("w:1280", "1280 px"), ("w:800", "800 px")]);
    }
    tool("resize", &tr("RESIZE", "REDIMENSIONAR"), options(&list), None)
}

// Herramientas de un archivo según su tipo. Carpetas y tipos sin herramientas: lista vacía.
// SVG (vectorial) y HEIC/HEIF (ImageMagick los lee pero no los escribe) tampoco tienen.
fn tools_of(path: &Path) -> Vec<ToolInfo> {
    let compress = || tool("compress", &tr("REDUCE SIZE", "REDUCIR TAMAÑO"), vec![], None);
    let strip = || tool("strip_metadata", &tr("METADATA", "METADATOS"), vec![], None);
    // Formatos que no se pueden comprimir sin perder calidad: la herramienta no se ofrece
    let sin_compresion = ["bmp", "wav", "flac", "aiff", "aif"].contains(&get_ext(path).as_str());
    let mut list = match category(path) {
        Some(Kind::Image) if !["svg", "heic", "heif"].contains(&get_ext(path).as_str()) => vec![
            compress(),
            strip(),
            tool("trim", &tr("TRIM", "RECORTAR"), vec![], Some("crop")),
            rotate_tool(),
            flip_tool(),
            resize_tool(false),
        ],
        Some(Kind::Video) => vec![
            compress(),
            strip(),
            tool("trim", &tr("TRIM", "RECORTAR"), vec![], Some("trim")),
            rotate_tool(),
            flip_tool(),
            resize_tool(true),
            tool("mute", &tr("NO AUDIO", "SIN AUDIO"), vec![], None),
        ],
        Some(Kind::Audio) => vec![compress(), strip(), tool("trim", &tr("TRIM", "RECORTAR"), vec![], Some("trim"))],
        Some(Kind::Pdf) => vec![compress()],
        _ => Vec::new(),
    };
    if sin_compresion {
        list.retain(|t| t.id != "compress");
    }
    // Cualquier archivo se puede empaquetar en un zip (los comprimidos y las carpetas ya tienen sus conversiones)
    if !matches!(category(path), None | Some(Kind::Archive) | Some(Kind::Folder)) {
        list.push(tool("zip", "ZIP", vec![], None));
    }
    list
}

// Sufijo del nombre de salida según el idioma: "photo (compressed).jpg" / "foto (compressed).jpg"
fn suffix_of(tool: &str) -> String {
    match tool {
        "compress" => tr("compressed", "comprimido"),
        "strip_metadata" => tr("no metadata", "sin metadatos"),
        "trim" => tr("trimmed", "recortado"),
        "rotate" => tr("rotated", "girado"),
        "flip" => tr("flipped", "volteado"),
        "resize" => tr("resized", "redimensionado"),
        _ => tr("no audio", "sin audio"),
    }
}

// Ruta de salida junto al original, con su misma extensión (tal cual, con sus mayúsculas):
// "photo (compressed).jpg", y si existe "photo (compressed) 2.jpg", "photo (compressed) 3.jpg"...
fn output_path(src: &Path, tool: &str) -> std::path::PathBuf {
    let folder = src.parent().unwrap_or(Path::new("."));
    let ext = src.extension().unwrap_or_default().to_string_lossy().to_string();
    unique_path(folder, &format!("{} ({})", get_stem(src), suffix_of(tool)), &ext)
}

// Nombre que el usuario escribió en el editor (campo opcional `nombre` de `params`), sin extensión.
// Devuelve Ok(None) si no hay nombre; si lo hay, se limpia: sin espacios en los extremos y sin la
// extensión del original si la escribió (la salida siempre lleva la del original). Se rechazan los
// nombres vacíos, con "/" o caracteres de control, "." y "..", y los demasiado largos.
fn custom_name(params: Option<&Value>, ext: &str) -> Result<Option<String>, String> {
    let Some(raw) = params.and_then(|p| p.get("nombre")).and_then(Value::as_str) else {
        return Ok(None);
    };
    let mut name = raw.trim();
    let suffix = format!(".{}", ext.to_lowercase());
    if !ext.is_empty() && name.to_lowercase().ends_with(&suffix) {
        name = name[..name.len() - suffix.len()].trim_end();
    }
    if name.is_empty() {
        return Err(tr("Type a name for the new file", "Escribe un nombre para el archivo nuevo"));
    }
    if name == "." || name == ".." || name.contains('/') || name.chars().any(char::is_control) {
        return Err(tr("The name is not valid: it cannot contain \"/\" or be \".\" or \"..\"", "El nombre no es válido: no puede llevar «/» ni ser «.» o «..»"));
    }
    // Los sistemas de archivos limitan el nombre a 255 bytes: se deja margen para " 2" y la extensión
    if name.len() + ext.len() > 230 {
        return Err(tr("The name is too long", "El nombre es demasiado largo"));
    }
    Ok(Some(name.to_string()))
}

// Igual que output_path, pero con el nombre elegido por el usuario si lo hay
fn output_path_named(src: &Path, tool: &str, name: Option<&str>) -> std::path::PathBuf {
    match name {
        None => output_path(src, tool),
        Some(name) => {
            let folder = src.parent().unwrap_or(Path::new("."));
            let ext = src.extension().unwrap_or_default().to_string_lossy().to_string();
            unique_path(folder, name, &ext)
        }
    }
}

// ---------- Validación ----------

// Mensajes que se repiten
fn invalid_option() -> String {
    tr("Invalid option", "Opción no válida")
}

fn missing_option() -> String {
    tr("Choose an option", "Falta elegir una opción")
}

fn tool_unavailable() -> String {
    tr("Tool not available for this file", "Herramienta no disponible para este archivo")
}

fn unsupported_type() -> String {
    tr("Unsupported file type", "Tipo de archivo no soportado")
}

fn cannot_read_image() -> String {
    tr("Could not read the image", "No se pudo leer la imagen")
}

fn cannot_read_file() -> String {
    tr("Could not read the file", "No se pudo leer el archivo")
}

fn cannot_make_image() -> String {
    tr("Could not generate the image", "No se pudo generar la imagen")
}

fn took_too_long() -> String {
    tr("Took too long", "Tardó demasiado")
}

// Número de un campo de `params` (finito)
fn number(params: &Value, key: &str) -> Result<f64, String> {
    params
        .get(key)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .ok_or_else(|| tr!("\"{key}\" is missing or is not a number", "Falta «{key}» o no es un número", key = key))
}

// Inicio y fin en segundos: 0 <= inicio < fin <= duración (con 0,05 s de margen por redondeos)
fn parse_trim(params: Option<&Value>, duration: f64) -> Result<(f64, f64), String> {
    let params = params.ok_or_else(|| tr("The start and end are missing", "Faltan el inicio y el fin"))?;
    let start = number(params, "start")?;
    let end = number(params, "end")?;
    if start < 0.0 {
        return Err(tr("The start cannot be negative", "El inicio no puede ser negativo"));
    }
    if start >= end {
        return Err(tr("The start must be before the end", "El inicio debe ser menor que el final"));
    }
    if end > duration + 0.05 {
        return Err(tr("The end is past the duration", "El final pasa de la duración"));
    }
    let end = end.min(duration);
    if end - start < 0.1 {
        return Err(tr("The selection is too short", "El tramo es demasiado corto"));
    }
    Ok((start, end))
}

// Recuadro (x, y, ancho, alto) en píxeles de la imagen ya orientada; dentro de sus límites.
// Los decimales se redondean al píxel más cercano.
fn parse_crop(params: Option<&Value>, img_w: u32, img_h: u32) -> Result<(u32, u32, u32, u32), String> {
    let params = params.ok_or_else(|| tr("The crop box is missing", "Falta el recuadro"))?;
    let mut values = [0u64; 4];
    for (value, key) in values.iter_mut().zip(["x", "y", "w", "h"]) {
        let n = number(params, key)?;
        if n < 0.0 || n > u32::MAX as f64 {
            return Err(tr!("The value \"{key}\" is not valid", "El valor «{key}» no es válido", key = key));
        }
        *value = n.round() as u64;
    }
    let [x, y, w, h] = values;
    if w == 0 || h == 0 {
        return Err(tr("The crop box is empty", "El recuadro está vacío"));
    }
    if x + w > img_w as u64 || y + h > img_h as u64 {
        return Err(tr("The crop box goes outside the image", "El recuadro se sale de la imagen"));
    }
    Ok((x as u32, y as u32, w as u32, h as u32))
}

// Valida la opción elegida contra las de la herramienta (las que no tienen opciones la ignoran)
fn check_option(info: &ToolInfo, option: Option<&str>) -> Result<Option<String>, String> {
    if info.options.is_empty() {
        return Ok(None);
    }
    match option {
        Some(o) if info.options.iter().any(|x| x.id == o) => Ok(Some(o.to_string())),
        Some(_) => Err(invalid_option()),
        None => Err(missing_option()),
    }
}

// "pct:50" -> ("pct", 50)
fn parse_resize(option: &str) -> Result<(&str, u32), String> {
    let (kind, n) = option.split_once(':').ok_or_else(invalid_option)?;
    let n = n.parse::<u32>().ok().filter(|n| *n > 0).ok_or_else(invalid_option)?;
    Ok((kind, n))
}

// El resultado de comprimir solo vale si ahorra algo claro; si no, el original ya estaba optimizado
fn worth_keeping(original: u64, compressed: u64) -> bool {
    compressed * 100 < original * MIN_SAVING_PERCENT
}

// ---------- Comandos de ImageMagick ----------

// Argumentos de magick (sin el programa) para una herramienta de imagen. `crop` ya viene validado.
// Todas aplican antes -auto-orient: así se conserva la orientación aunque -strip borre el EXIF.
// En un GIF animado se juntan los fotogramas (-coalesce) antes y se optimizan (-layers Optimize) después.
fn image_args(ext: &str, tool: &str, option: Option<&str>, crop: Option<(u32, u32, u32, u32)>, src: &str, out: &str) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = vec![src.into(), "-auto-orient".into()];
    let gif = ext == "gif";
    if gif {
        args.push("-coalesce".into());
    }
    let mut extra: Vec<String> = Vec::new();
    match tool {
        "compress" => match ext {
            // 4:2:0 y progresivo ahorran bastante más que bajar la calidad
            "jpg" | "jpeg" => extra.extend(["-strip", "-quality", "75", "-sampling-factor", "4:2:0", "-interlace", "Plane"].map(String::from)),
            "webp" => extra.extend(["-strip", "-quality", "75"].map(String::from)),
            "avif" => extra.extend(["-strip", "-quality", "60"].map(String::from)),
            // Sin pérdida: compresión máxima y filtro adaptativo. En fotos ahorra casi nada.
            "png" => extra.extend(
                ["-strip", "-define", "png:compression-level=9", "-define", "png:compression-filter=5", "-define", "png:compression-strategy=1"]
                    .map(String::from),
            ),
            "tif" | "tiff" => extra.extend(["-strip", "-compress", "zip"].map(String::from)),
            "gif" => extra.extend(["-strip", "-fuzz", "2%"].map(String::from)),
            "bmp" => return Err(tr("The BMP format does not support compression", "El formato BMP no admite compresión")),
            _ => return Err(tr("This format cannot be compressed", "No se puede comprimir este formato")),
        },
        "strip_metadata" => extra.push("-strip".into()),
        "trim" => {
            let (x, y, w, h) = crop.ok_or_else(|| tr("The crop box is missing", "Falta el recuadro"))?;
            extra.extend(["-crop".to_string(), format!("{w}x{h}+{x}+{y}"), "+repage".into()]);
        }
        "rotate" => {
            let degrees = match option {
                Some("left") => "270",
                Some("right") => "90",
                Some("180") => "180",
                _ => return Err(invalid_option()),
            };
            extra.extend(["-rotate".to_string(), degrees.into()]);
        }
        // flop es el espejo izquierda-derecha y flip el de arriba-abajo
        "flip" => extra.push(if option == Some("horizontal") { "-flop" } else { "-flip" }.into()),
        "resize" => {
            let (kind, n) = parse_resize(option.ok_or_else(missing_option)?)?;
            // "800x>" fija el ancho en 800 solo si la imagen es más ancha: nunca se amplía
            let size = if kind == "pct" { format!("{n}%") } else { format!("{n}x>") };
            extra.extend(["-resize".to_string(), size]);
        }
        _ => return Err(tool_unavailable()),
    }
    args.extend(extra);
    if gif {
        args.extend(["-layers".to_string(), "Optimize".into()]);
    }
    args.push(out.into());
    Ok(args)
}

// ---------- Comandos de ffmpeg ----------

// Códec de audio del contenedor al recodificar (los pares estándar de cada formato)
fn audio_codec(ext: &str) -> Vec<&'static str> {
    match ext {
        "mp3" | "avi" => vec!["-c:a", "libmp3lame", "-b:a", "128k"],
        "ogg" => vec!["-c:a", "libvorbis", "-q:a", "4"],
        "opus" | "webm" => vec!["-c:a", "libopus", "-b:a", "96k"],
        "wma" | "wmv" => vec!["-c:a", "wmav2", "-b:a", "128k"],
        _ => vec!["-c:a", "aac", "-b:a", "128k"],
    }
}

// Códec de vídeo del contenedor. `compress` busca ahorrar espacio; si no (recortar, girar...)
// se recodifica con calidad alta para que se note poco.
fn video_codec(ext: &str, compress: bool) -> Vec<&'static str> {
    match ext {
        "webm" => vec!["-c:v", "libvpx-vp9", "-crf", if compress { "33" } else { "28" }, "-b:v", "0", "-deadline", "good", "-cpu-used", "4", "-row-mt", "1"],
        "wmv" => vec!["-c:v", "wmv2", "-q:v", if compress { "16" } else { "4" }],
        _ => {
            let mut args = vec!["-c:v", "libx264", "-crf", if compress { "28" } else { "20" }, "-preset", if compress { "medium" } else { "veryfast" }, "-pix_fmt", "yuv420p"];
            if ext == "mp4" || ext == "mov" {
                args.extend(["-movflags", "+faststart"]);
            }
            args
        }
    }
}

// Filtro de vídeo de girar, voltear y redimensionar. Las dimensiones salen siempre pares
// (los códecs lo exigen); la coma dentro del filtro va escapada.
fn video_filter(tool: &str, option: Option<&str>) -> Result<String, String> {
    let filter = match (tool, option) {
        ("rotate", Some("left")) => "transpose=2".to_string(),
        ("rotate", Some("right")) => "transpose=1".to_string(),
        ("rotate", Some("180")) => "hflip,vflip".to_string(),
        ("flip", Some("horizontal")) => "hflip".to_string(),
        ("flip", Some("vertical")) => "vflip".to_string(),
        ("resize", Some(option)) => {
            let (kind, n) = parse_resize(option)?;
            if kind == "pct" {
                // ancho = N % del original redondeado a par; el alto se calcula solo (-2 = par)
                format!("scale=trunc(iw*{n}/200)*2:-2")
            } else {
                // alto N, o el del original si es menor (no se amplía)
                format!("scale=-2:min({n}\\,ih)")
            }
        }
        _ => return Err(invalid_option()),
    };
    Ok(filter)
}

// Argumentos de ffmpeg (sin el programa) para una herramienta de audio o vídeo.
// `trim` ya viene validado. Con `progress` pide a ffmpeg el avance por stdout.
// Decisión de recortar: el audio se corta copiando (sin pérdida; precisión de un paquete, unos 25 ms
// en mp3/aac; flac se recodifica sin pérdida) y el vídeo se recodifica (corte exacto al fotograma; con copia solo cortaría en
// fotogramas clave y el inicio saldría roto).
fn ffmpeg_args(kind: Kind, ext: &str, tool: &str, option: Option<&str>, trim: Option<(f64, f64)>, src: &str, out: &str, progress: bool) -> Result<Vec<String>, String> {
    let video = kind == Kind::Video;
    let mut args: Vec<String> = ["-nostdin", "-hide_banner", "-v", "error", "-n"].map(String::from).to_vec();
    if progress {
        args.extend(["-progress", "pipe:1", "-nostats"].map(String::from));
    }
    // Recortar: se salta al inicio antes de abrir (rápido) y se limita la duración de la salida
    let mut duration_args: Vec<String> = Vec::new();
    if let Some((start, end)) = trim {
        args.extend(["-ss".to_string(), format!("{start:.3}")]);
        duration_args.extend(["-t".to_string(), format!("{:.3}", end - start)]);
    }
    args.extend(["-i".to_string(), src.to_string()]);
    args.extend(duration_args);
    let mut rest: Vec<String> = Vec::new();
    match (tool, video) {
        ("compress", false) => {
            // wav, flac y aiff no pierden calidad: bajar el bitrate no es posible (flac ahorra ~0,01 %)
            if ["wav", "flac", "aiff"].contains(&ext) {
                return Err(tr("This format cannot be compressed without losing quality", "Este formato no se puede comprimir sin perder calidad"));
            }
            // Se descarta la carátula incrustada (-map 0:a)
            rest.extend(["-map", "0:a"].map(String::from));
            rest.extend(audio_codec(ext).into_iter().map(String::from));
        }
        ("compress", true) => {
            rest.extend(["-map", "0:v:0", "-map", "0:a?"].map(String::from));
            rest.extend(video_codec(ext, true).into_iter().map(String::from));
            rest.extend(audio_codec(ext).into_iter().map(String::from));
        }
        // Copia sin recodificar; -map_metadata -1 borra etiquetas globales y de pistas, y quita capítulos.
        // bitexact evita que ffmpeg escriba su propia etiqueta "encoder".
        ("strip_metadata", _) => {
            let maps: &[&str] = if video { &["-map", "0:v", "-map", "0:a?", "-map", "0:s?"] } else { &["-map", "0:a"] };
            rest.extend(maps.iter().map(|s| s.to_string()));
            rest.extend(["-map_metadata", "-1", "-map_chapters", "-1", "-c", "copy", "-fflags", "+bitexact"].map(String::from));
        }
        // flac se recodifica (sin pérdida): al copiar conservaría en la cabecera la duración original
        ("trim", false) => rest.extend(["-map", "0:a", "-c", if ext == "flac" { "flac" } else { "copy" }].map(String::from)),
        ("trim", true) => {
            rest.extend(["-map", "0:v:0", "-map", "0:a?"].map(String::from));
            rest.extend(video_codec(ext, false).into_iter().map(String::from));
            rest.extend(audio_codec(ext).into_iter().map(String::from));
        }
        ("rotate" | "flip" | "resize", true) => {
            rest.extend(["-map", "0:v:0", "-map", "0:a?", "-vf"].map(String::from));
            rest.push(video_filter(tool, option)?);
            rest.extend(video_codec(ext, false).into_iter().map(String::from));
            // El audio no cambia: se copia
            rest.extend(["-c:a", "copy"].map(String::from));
        }
        ("mute", true) => rest.extend(["-map", "0:v", "-map", "0:s?", "-c", "copy"].map(String::from)),
        _ => return Err(tool_unavailable()),
    }
    args.extend(rest);
    args.push(out.to_string());
    Ok(args)
}

fn as_refs(args: &[String]) -> Vec<&str> {
    args.iter().map(|s| s.as_str()).collect()
}

// ---------- Ejecutar una herramienta ----------

// Medidas de la imagen después de la orientación EXIF (solo el primer fotograma)
fn image_size(src: &str) -> Result<(u32, u32), String> {
    let first_frame = format!("{src}[0]");
    let args = ["identify", "-auto-orient", "-format", "%w %h\n", first_frame.as_str()];
    let bytes = capture("magick", &args, Instant::now() + QUICK_WAIT).map_err(|_| cannot_read_image())?;
    parse_size(&String::from_utf8_lossy(&bytes)).ok_or_else(cannot_read_image)
}

// "640 480\n" -> (640, 480)
fn parse_size(text: &str) -> Option<(u32, u32)> {
    let mut numbers = text.lines().next()?.split_whitespace();
    let w = numbers.next()?.parse().ok()?;
    let h = numbers.next()?.parse().ok()?;
    Some((w, h))
}

// Comprime un PDF con ghostscript a una carpeta temporal y lo mueve a `out` (ghostscript interpreta
// el "%" de un nombre de salida, así que no se le da la ruta final)
fn compress_pdf(src: &str, out: &Path) -> Result<(), String> {
    let gs = need(&["gs"], "sudo apt install ghostscript")?;
    let tmp = TempDir::new()?;
    let tmp_out = tmp.path().join("salida.pdf");
    let output = format!("-sOutputFile={}", st(&tmp_out));
    let args = ["-sDEVICE=pdfwrite", "-dPDFSETTINGS=/ebook", "-dNOPAUSE", "-dBATCH", "-dQUIET", output.as_str(), "-f", src];
    run_in(&gs, &args, None)?;
    if std::fs::metadata(&tmp_out).map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(tr("Could not compress the PDF", "No se pudo comprimir el PDF"));
    }
    move_file(&tmp_out, out)
}

// Ejecuta la herramienta y devuelve la ruta de salida. Núcleo de run_tool: en la app `progress` emite
// el evento convert-progress y en las pruebas guarda los valores.
fn run_tool_with(path: &str, tool: &str, option: Option<&str>, params: Option<&Value>, progress: Option<Progress>) -> Result<String, String> {
    // Cada herramienta nueva empieza sin cancelación pendiente
    CANCEL.store(false, Ordering::SeqCst);
    let src = std::path::absolute(path).map_err(|e| tr!("Invalid path: {e}", "Ruta no válida: {e}", e = e))?;
    if !src.is_file() {
        return Err(tr("File does not exist", "El archivo no existe"));
    }
    let kind = category(&src).ok_or_else(unsupported_type)?;
    let info = tools_of(&src)
        .into_iter()
        .find(|t| t.id == tool)
        .ok_or_else(tool_unavailable)?;
    if tool == "zip" {
        return pack(&[path.to_string()], "zip");
    }
    let option = check_option(&info, option)?;
    let option = option.as_deref();
    let ext = get_ext(&src);
    let name = custom_name(params, &ext)?;
    let (src_text, out) = (st(&src), output_path_named(&src, tool, name.as_deref()));
    let out_text = st(&out);

    let result = match kind {
        Kind::Image => {
            let magick = need(&["magick"], "sudo apt install imagemagick")?;
            // El recorte se valida contra la imagen orientada
            let crop = if tool == "trim" {
                let (w, h) = image_size(&src_text)?;
                Some(parse_crop(params, w, h)?)
            } else {
                None
            };
            let args = image_args(&ext, tool, option, crop, &src_text, &out_text)?;
            run_in(&magick, &as_refs(&args), None)
        }
        Kind::Audio | Kind::Video => {
            need(&["ffmpeg"], "sudo apt install ffmpeg")?;
            let duration = probe_duration(&src_text);
            let trim = if tool == "trim" {
                Some(parse_trim(params, duration.ok_or_else(|| tr("Could not read the duration", "No se pudo leer la duración"))?)?)
            } else {
                None
            };
            // Para el progreso cuenta la duración de lo que se escribe (el tramo, si se recorta)
            let total = match trim {
                Some((start, end)) => Some(end - start),
                None => duration,
            };
            let progress = match (progress, total) {
                (Some(callback), Some(total)) => Some((total, callback)),
                _ => None,
            };
            let args = ffmpeg_args(kind, &ext, tool, option, trim, &src_text, &out_text, progress.is_some())?;
            run_progress("ffmpeg", &as_refs(&args), None, progress)
        }
        // Kind::Pdf: tools_of solo deja pasar "compress"
        _ => compress_pdf(&src_text, &out),
    };
    let result = result.and_then(|_| check_saving(tool, &src, &out));
    if let Err(e) = result {
        // No dejar salidas a medias, tampoco si se canceló (la ruta era nueva, así que es nuestra)
        let _ = std::fs::remove_file(&out);
        return Err(if has_code(&e, CANCELLED) || has_code(&e, OPTIMIZED) { e } else { tr!("Could not apply the tool: {e}", "No se pudo aplicar la herramienta: {e}", e = e) });
    }
    Ok(out_text)
}

// Comprimir solo deja el resultado si ahorra algo claro; si no, error con código [optimized]
// y la salida se borra (así el original sigue siendo la mejor versión)
fn check_saving(tool: &str, src: &Path, out: &Path) -> Result<(), String> {
    if tool != "compress" {
        return Ok(());
    }
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    if worth_keeping(size(src), size(out)) {
        Ok(())
    } else {
        Err(optimized_error())
    }
}

// ---------- Información, vista previa y forma de onda ----------

// Interpreta la salida JSON de ffprobe. En vídeo, las medidas son las de la imagen que se ve: si el
// vídeo trae una rotación de 90 o 270 grados (metadato rotate o matriz de rotación), ancho y alto se
// intercambian, igual que al recodificar con ffmpeg (que aplica la rotación y la quita del metadato).
fn parse_probe(json: &str, kind: Kind) -> Result<MediaInfo, String> {
    let root: Value = serde_json::from_str(json).map_err(|_| cannot_read_file())?;
    let duration = root["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok()).filter(|d| d.is_finite() && *d >= 0.0);
    if kind == Kind::Audio {
        return Ok(MediaInfo { kind: "audio".into(), width: None, height: None, duration });
    }
    let stream = root["streams"]
        .as_array()
        .and_then(|list| list.iter().find(|s| s["codec_type"] == "video"))
        .ok_or_else(|| tr("The video has no picture", "El vídeo no tiene imagen"))?;
    let mut width = stream["width"].as_u64().map(|n| n as u32);
    let mut height = stream["height"].as_u64().map(|n| n as u32);
    let side_rotation = stream["side_data_list"].as_array().and_then(|list| list.iter().find_map(|d| d["rotation"].as_f64()));
    let tag_rotation = stream["tags"]["rotate"].as_str().and_then(|r| r.parse::<f64>().ok());
    let rotation = side_rotation.or(tag_rotation).unwrap_or(0.0);
    if (rotation.round() as i64).rem_euclid(180) == 90 {
        std::mem::swap(&mut width, &mut height);
    }
    Ok(MediaInfo { kind: "video".into(), width, height, duration })
}

fn media_info_blocking(path: &str) -> Result<MediaInfo, String> {
    let src = std::path::absolute(path).map_err(|_| tr("Invalid path", "Ruta no válida"))?;
    if !src.is_file() {
        return Err(tr("File does not exist", "El archivo no existe"));
    }
    let kind = category(&src).ok_or_else(unsupported_type)?;
    let src = st(&src);
    match kind {
        Kind::Image => {
            let (w, h) = image_size(&src)?;
            Ok(MediaInfo { kind: "imagen".into(), width: Some(w), height: Some(h), duration: None })
        }
        Kind::Audio | Kind::Video => {
            need(&["ffprobe"], "sudo apt install ffmpeg")?;
            let entries = "stream=codec_type,width,height:stream_tags=rotate:stream_side_data=rotation:format=duration";
            let args = ["-v", "error", "-print_format", "json", "-show_entries", entries, "--", src.as_str()];
            let bytes = capture("ffprobe", &args, Instant::now() + QUICK_WAIT).map_err(|_| cannot_read_file())?;
            parse_probe(&String::from_utf8_lossy(&bytes), kind)
        }
        _ => Err(unsupported_type()),
    }
}

// Lado máximo razonable para una vista previa
fn clamp_side(max_side: u32) -> u32 {
    max_side.clamp(16, 4096)
}

fn jpeg_data_url(bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() {
        return Err(cannot_make_image());
    }
    Ok(format!("data:image/jpeg;base64,{}", base64(bytes)))
}

// Traduce el error de capture: el mensaje de miniatura no sirve aquí
fn capture_error(e: String) -> String {
    if e == timeout_message() { took_too_long() } else { cannot_make_image() }
}

fn preview_image_blocking(path: &str, max_side: u32) -> Result<String, String> {
    let src = std::path::absolute(path).map_err(|_| tr("Invalid path", "Ruta no válida"))?;
    if !src.is_file() || category(&src) != Some(Kind::Image) {
        return Err(tr("Not an image", "No es una imagen"));
    }
    need(&["magick"], "sudo apt install imagemagick")?;
    let first_frame = format!("{}[0]", st(&src));
    let size = format!("{0}x{0}>", clamp_side(max_side));
    // Sobre fondo blanco: JPEG no tiene transparencia
    let args = [first_frame.as_str(), "-auto-orient", "-background", "white", "-alpha", "remove", "-alpha", "off", "-thumbnail", size.as_str(), "jpg:-"];
    let bytes = capture("magick", &args, Instant::now() + QUICK_WAIT).map_err(capture_error)?;
    jpeg_data_url(&bytes)
}

fn video_frame_blocking(path: &str, seconds: f64, max_side: u32) -> Result<String, String> {
    let src = std::path::absolute(path).map_err(|_| tr("Invalid path", "Ruta no válida"))?;
    if !src.is_file() || category(&src) != Some(Kind::Video) {
        return Err(tr("Not a video", "No es un vídeo"));
    }
    need(&["ffmpeg"], "sudo apt install ffmpeg")?;
    if !seconds.is_finite() {
        return Err(tr("The time is not valid", "El tiempo no es válido"));
    }
    let src = st(&src);
    let start = format!("{:.3}", seconds.max(0.0));
    // Cabe en un cuadrado de max_side sin ampliar (ffmpeg aplica la rotación del vídeo solo)
    let side = clamp_side(max_side);
    let scale = format!("scale=w=min(iw\\,{side}):h=min(ih\\,{side}):force_original_aspect_ratio=decrease");
    let deadline = Instant::now() + QUICK_WAIT;
    let rest = ["-frames:v", "1", "-vf", scale.as_str(), "-f", "image2pipe", "-vcodec", "mjpeg", "-"];
    let base = ["-nostdin", "-v", "error"];
    let mut with_seek: Vec<&str> = base.to_vec();
    with_seek.extend(["-ss", start.as_str(), "-i", src.as_str()]);
    with_seek.extend(rest);
    // Si el tiempo pasa del final no sale ningún fotograma: se prueba con el primero
    if let Ok(bytes) = capture("ffmpeg", &with_seek, deadline) {
        if !bytes.is_empty() {
            return jpeg_data_url(&bytes);
        }
    }
    let mut from_start: Vec<&str> = base.to_vec();
    from_start.extend(["-i", src.as_str()]);
    from_start.extend(rest);
    let bytes = capture("ffmpeg", &from_start, deadline).map_err(capture_error)?;
    jpeg_data_url(&bytes)
}

// Suma muestras (16 bits con signo) a los picos. `first` es el número de la primera muestra del
// trozo y `per_bucket` cuántas muestras caben en cada cubo; lo que sobra por redondeo va al último.
fn add_samples(peaks: &mut [f32], first: usize, per_bucket: usize, samples: &[i16]) {
    let last = peaks.len() - 1;
    for (i, sample) in samples.iter().enumerate() {
        let bucket = ((first + i) / per_bucket).min(last);
        let level = (*sample as i32).abs() as f32 / 32768.0;
        if level > peaks[bucket] {
            peaks[bucket] = level;
        }
    }
}

// Escala los picos para que el mayor sea 1.0 (un audio todo en silencio queda en ceros)
fn normalize(peaks: &mut [f32]) {
    let max = peaks.iter().cloned().fold(0.0, f32::max);
    if max > 0.0 {
        for peak in peaks.iter_mut() {
            *peak /= max;
        }
    }
}

// Lee PCM s16le por trozos y calcula el pico de cada cubo. Con `per_bucket` (se conoce la duración)
// no guarda las muestras: solo los picos. Sin él las guarda todas (8 kHz, unos 56 MB por hora).
fn read_peaks<R: Read>(mut stream: R, buckets: usize, per_bucket: Option<usize>) -> Vec<f32> {
    let mut peaks = vec![0.0f32; buckets];
    let mut all: Vec<i16> = Vec::new();
    let mut done = 0;
    // Un trozo puede acabar a mitad de una muestra: el byte suelto espera al siguiente
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 16384];
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        pending.extend_from_slice(&buf[..n]);
        let even = pending.len() / 2 * 2;
        let samples: Vec<i16> = pending[..even].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        pending.drain(..even);
        match per_bucket {
            Some(size) => add_samples(&mut peaks, done, size, &samples),
            None => all.extend_from_slice(&samples),
        }
        done += samples.len();
    }
    if per_bucket.is_none() {
        add_samples(&mut peaks, 0, (all.len() / buckets).max(1), &all);
    }
    normalize(&mut peaks);
    peaks
}

fn waveform_blocking(path: &str, buckets: u32) -> Result<Vec<f32>, String> {
    let src = std::path::absolute(path).map_err(|_| tr("Invalid path", "Ruta no válida"))?;
    if !src.is_file() || !matches!(category(&src), Some(Kind::Audio | Kind::Video)) {
        return Err(tr("Not an audio or video file", "No es un audio ni un vídeo"));
    }
    if buckets == 0 || buckets > 4000 {
        return Err(tr("Invalid number of bars", "Número de barras no válido"));
    }
    need(&["ffmpeg"], "sudo apt install ffmpeg")?;
    let src = st(&src);
    let buckets = buckets as usize;
    // Con la duración se sabe cuántas muestras tiene cada cubo y se procesa sin guardar el audio
    let per_bucket = probe_duration(&src).map(|d| ((d * WAVE_RATE) as usize / buckets).max(1));
    let args = ["-nostdin", "-v", "error", "-i", src.as_str(), "-vn", "-ac", "1", "-ar", "8000", "-f", "s16le", "-"];
    let peaks = capture_with("ffmpeg", &args, Instant::now() + WAVE_WAIT, move |stream| read_peaks(stream, buckets, per_bucket))
        .map_err(|e| if e == timeout_message() { took_too_long() } else { tr("Could not read the audio", "No se pudo leer el audio") })?;
    Ok(peaks)
}

// ---------- Comandos de Tauri ----------

#[tauri::command]
pub fn get_tools(path: String) -> Vec<ToolInfo> {
    tools_of(Path::new(&path))
}

// Evento convert-progress { percent } como en convert (el 100 lo marca la interfaz al terminar)
#[tauri::command]
pub async fn run_tool(app: AppHandle, path: String, tool: String, option: Option<String>, params: Option<Value>) -> Result<String, String> {
    let progress: Progress = Arc::new(move |percent| {
        let _ = app.emit("convert-progress", serde_json::json!({ "percent": percent }));
    });
    tauri::async_runtime::spawn_blocking(move || run_tool_with(&path, &tool, option.as_deref(), params.as_ref(), Some(progress)))
        .await
        .map_err(|e| tr!("Internal error: {e}", "Error interno: {e}", e = e))?
}

#[tauri::command]
pub async fn media_info(path: String) -> Result<MediaInfo, String> {
    tauri::async_runtime::spawn_blocking(move || media_info_blocking(&path))
        .await
        .map_err(|e| tr!("Internal error: {e}", "Error interno: {e}", e = e))?
}

#[tauri::command]
pub async fn preview_image(path: String, max_side: u32) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || preview_image_blocking(&path, max_side))
        .await
        .map_err(|e| tr!("Internal error: {e}", "Error interno: {e}", e = e))?
}

#[tauri::command]
pub async fn video_frame(path: String, seconds: f64, max_side: u32) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || video_frame_blocking(&path, seconds, max_side))
        .await
        .map_err(|e| tr!("Internal error: {e}", "Error interno: {e}", e = e))?
}

#[tauri::command]
pub async fn waveform(path: String, buckets: u32) -> Result<Vec<f32>, String> {
    tauri::async_runtime::spawn_blocking(move || waveform_blocking(&path, buckets))
        .await
        .map_err(|e| tr!("Internal error: {e}", "Error interno: {e}", e = e))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::{tests::lock, tool_exists};
    use serde_json::json;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::Mutex;

    // Carpeta temporal propia de cada prueba
    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mandarina_tools_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn run(program: &str, args: &[&str]) {
        let status = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).status().unwrap();
        assert!(status.success(), "{program} falló");
    }

    // Salida de un programa como texto (sin espacios sobrantes)
    fn output(program: &str, args: &[&str]) -> String {
        let out = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn have(names: &[&str]) -> bool {
        names.iter().all(|n| tool_exists(n))
    }

    fn size_of(path: &str) -> u64 {
        std::fs::metadata(path).unwrap().len()
    }

    // Medidas de una imagen tal como se ven (con la orientación aplicada)
    fn dims(path: &str) -> (u32, u32) {
        let text = output("magick", &["identify", "-auto-orient", "-format", "%w %h", &format!("{path}[0]")]);
        parse_size(&text).unwrap()
    }

    fn ids(list: &[ToolInfo]) -> Vec<&str> {
        list.iter().map(|t| t.id.as_str()).collect()
    }

    fn option_ids(info: &ToolInfo) -> Vec<&str> {
        info.options.iter().map(|o| o.id.as_str()).collect()
    }

    // JPEG de w x h con el EXIF de orientación 6 (se ve girado 90°) y un comentario "secreto"
    fn jpeg_with_exif(path: &Path, w: u32, h: u32) {
        run("magick", &["-size", &format!("{w}x{h}"), "plasma:fractal", "-quality", "90", &st(path)]);
        let bytes = std::fs::read(path).unwrap();
        let tiff = b"II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut exif = b"Exif\0\0".to_vec();
        exif.extend_from_slice(tiff);
        let mut data = bytes[..2].to_vec();
        data.extend_from_slice(&[0xFF, 0xE1]);
        data.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        data.extend_from_slice(&exif);
        let comment = b"secreto";
        data.extend_from_slice(&[0xFF, 0xFE]);
        data.extend_from_slice(&((comment.len() + 2) as u16).to_be_bytes());
        data.extend_from_slice(comment);
        data.extend_from_slice(&bytes[2..]);
        std::fs::write(path, data).unwrap();
    }

    // Vídeo de prueba con audio y el título "Secreto"
    fn make_video(path: &Path, secs: &str) {
        let args = [
            "-nostdin", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=320x240:rate=25", "-f", "lavfi", "-i", "sine=frequency=440",
            "-t", secs, "-c:v", "libx264", "-crf", "14", "-pix_fmt", "yuv420p", "-c:a", "aac", "-metadata", "title=Secreto",
        ];
        let mut all: Vec<&str> = args.to_vec();
        let out = st(path);
        all.push(&out);
        run("ffmpeg", &all);
    }

    // Duración en segundos según ffprobe
    fn duration_of(path: &str) -> f64 {
        output("ffprobe", &["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1", path]).parse().unwrap()
    }

    fn tool_run(path: &Path, tool: &str, option: Option<&str>, params: Option<Value>) -> Result<String, String> {
        run_tool_with(&st(path), tool, option, params.as_ref(), None)
    }

    // ----- Lógica pura -----

    #[test]
    fn nombre_elegido_por_el_usuario() {
        let n = |v: Value, ext: &str| custom_name(Some(&v), ext);
        // Sin campo o sin params: no hay nombre
        assert_eq!(custom_name(None, "jpg"), Ok(None));
        assert_eq!(n(json!({ "x": 1 }), "jpg"), Ok(None));
        // Se limpian espacios y la extensión del original (sin importar mayúsculas)
        assert_eq!(n(json!({ "nombre": "  mi recorte  " }), "jpg"), Ok(Some("mi recorte".into())));
        assert_eq!(n(json!({ "nombre": "mi recorte.JPG" }), "jpg"), Ok(Some("mi recorte".into())));
        assert_eq!(n(json!({ "nombre": "foto.v2" }), "jpg"), Ok(Some("foto.v2".into())));
        assert_eq!(n(json!({ "nombre": "ñandú – 日本" }), "png"), Ok(Some("ñandú – 日本".into())));
        // Rechazos
        for malo in ["", "   ", ".jpg", ".", "..", "a/b", "../x", "a\nb", "a\u{0}b"] {
            assert!(n(json!({ "nombre": malo }), "jpg").is_err(), "{malo:?}");
        }
        assert!(n(json!({ "nombre": "x".repeat(300) }), "jpg").is_err());
        // Un valor que no es texto se ignora
        assert_eq!(n(json!({ "nombre": 5 }), "jpg"), Ok(None));
    }

    #[test]
    fn salida_con_nombre_elegido() {
        let dir = std::env::temp_dir().join(format!("mandarina_tools_nombre_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("foto.JPG");
        std::fs::write(&src, b"x").unwrap();
        // Con nombre: misma carpeta y extensión del original
        assert_eq!(output_path_named(&src, "trim", Some("mi recorte")), dir.join("mi recorte.JPG"));
        // Sin nombre: el de siempre
        assert_eq!(output_path_named(&src, "trim", None), dir.join("foto (trimmed).JPG"));
        // Si existe, nunca se sobrescribe
        std::fs::write(dir.join("mi recorte.JPG"), b"x").unwrap();
        assert_eq!(output_path_named(&src, "trim", Some("mi recorte")), dir.join("mi recorte 2.JPG"));
        // Aunque se llame como el original
        assert_eq!(output_path_named(&src, "trim", Some("foto")), dir.join("foto 2.JPG"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recorta_con_nombre_elegido() {
        let _g = crate::converters::tests::lock();
        if !tool_exists("magick") {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mandarina_tools_recorte_nombre_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("-original.png");
        run("magick", &["-size", "200x100", "xc:red", &st(&src)]);
        let crop = |nombre: &str| json!({ "x": 10, "y": 10, "w": 50, "h": 40, "nombre": nombre });
        let out = tool_run(&src, "trim", None, Some(crop("-mi recorte"))).unwrap();
        assert_eq!(out, st(&dir.join("-mi recorte.png")));
        assert_eq!(image_size(&out).unwrap(), (50, 40));
        // El mismo nombre otra vez: no se pisa
        let otra = tool_run(&src, "trim", None, Some(crop("-mi recorte.png"))).unwrap();
        assert_eq!(otra, st(&dir.join("-mi recorte 2.png")));
        // Nombre no válido: error claro y no se crea nada
        let antes = std::fs::read_dir(&dir).unwrap().count();
        assert!(tool_run(&src, "trim", None, Some(crop("a/b"))).is_err());
        assert!(tool_run(&src, "trim", None, Some(crop("  "))).is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), antes);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn herramientas_de_imagen() {
        let list = tools_of(Path::new("a.JPG"));
        assert_eq!(ids(&list), ["compress", "strip_metadata", "trim", "rotate", "flip", "resize", "zip"]);
        let labels: Vec<&str> = list.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["REDUCE SIZE", "METADATA", "TRIM", "ROTATE", "FLIP", "RESIZE", "ZIP"]);
        assert_eq!(list[2].editor.as_deref(), Some("crop"));
        assert!(list.iter().filter(|t| t.id != "trim").all(|t| t.editor.is_none()));
        assert_eq!(option_ids(&list[3]), ["left", "right", "180"]);
        assert_eq!(list[3].options[0].label, "90° ↺");
        assert_eq!(option_ids(&list[4]), ["horizontal", "vertical"]);
        assert_eq!(option_ids(&list[5]), ["pct:75", "pct:50", "pct:25", "w:1920", "w:1280", "w:800"]);
        assert_eq!(list[5].options[3].label, "1920 px");
        assert!(list.iter().all(|t| t.options.len() <= 8));
    }

    #[test]
    fn herramientas_de_video() {
        let list = tools_of(Path::new("a.mkv"));
        assert_eq!(ids(&list), ["compress", "strip_metadata", "trim", "rotate", "flip", "resize", "mute", "zip"]);
        assert_eq!(list[2].editor.as_deref(), Some("trim"));
        assert_eq!(option_ids(&list[5]), ["pct:75", "pct:50", "pct:25", "h:1080", "h:720", "h:480"]);
        assert_eq!(list[5].options[3].label, "1080p");
        assert_eq!(list[6].label, "NO AUDIO");
        assert!(list.iter().all(|t| t.options.len() <= 8));
    }

    #[test]
    fn herramientas_de_audio_pdf_y_otros() {
        let audio = tools_of(Path::new("a.mp3"));
        assert_eq!(ids(&audio), ["compress", "strip_metadata", "trim", "zip"]);
        assert_eq!(audio[2].editor.as_deref(), Some("trim"));
        // Sin pérdida (wav, flac, aiff) y bmp: no se ofrece COMPRIMIR
        assert_eq!(ids(&tools_of(Path::new("a.flac"))), ["strip_metadata", "trim", "zip"]);
        assert_eq!(ids(&tools_of(Path::new("a.wav"))), ["strip_metadata", "trim", "zip"]);
        assert!(!ids(&tools_of(Path::new("a.bmp"))).contains(&"compress"));
        assert_eq!(ids(&tools_of(Path::new("a.pdf"))), ["compress", "zip"]);
        // Comprimidos y tipos desconocidos: ninguna. Documentos, SVG y HEIC: solo ZIP
        for name in ["a.zip", "a.xyz", "sinextension"] {
            assert!(tools_of(Path::new(name)).is_empty(), "{name}");
        }
        for name in ["a.docx", "a.srt", "a.svg", "a.heic", "a.heif"] {
            assert_eq!(ids(&tools_of(Path::new(name))), ["zip"], "{name}");
        }
        // Carpetas, aunque se llamen como un archivo con herramientas
        let dir = test_dir("carpeta").join("fotos.jpg");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(tools_of(&dir).is_empty());
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn json_del_contrato() {
        let list = tools_of(Path::new("a.mp3"));
        let json = serde_json::to_value(&list[2]).unwrap();
        assert_eq!(json, json!({ "id": "trim", "label": "TRIM", "options": [], "editor": "trim" }));
        let json = serde_json::to_value(&list[0]).unwrap();
        assert_eq!(json["editor"], Value::Null);
    }

    #[test]
    fn nombres_de_salida() {
        let dir = test_dir("nombres");
        let src = dir.join("foto.JPG");
        std::fs::write(&src, b"x").unwrap();
        assert_eq!(output_path(&src, "compress"), dir.join("foto (compressed).JPG"));
        // La regla de " 2" va al final del nombre completo
        std::fs::write(dir.join("foto (compressed).JPG"), b"x").unwrap();
        assert_eq!(output_path(&src, "compress"), dir.join("foto (compressed) 2.JPG"));
        std::fs::write(dir.join("foto (compressed) 2.JPG"), b"x").unwrap();
        assert_eq!(output_path(&src, "compress"), dir.join("foto (compressed) 3.JPG"));
        let names = [("strip_metadata", "no metadata"), ("trim", "trimmed"), ("rotate", "rotated"), ("flip", "flipped"), ("resize", "resized"), ("mute", "no audio")];
        for (tool, suffix) in names {
            assert_eq!(output_path(&src, tool), dir.join(format!("foto ({suffix}).JPG")));
        }
        // Nombre con puntos y con "-" delante
        let odd = dir.join("-mi.foto.v2.png");
        assert_eq!(output_path(&odd, "rotate"), dir.join("-mi.foto.v2 (rotated).png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn valida_el_recorte_de_tiempo() {
        let ok = json!({ "start": 1.0, "end": 3.5 });
        assert_eq!(parse_trim(Some(&ok), 10.0), Ok((1.0, 3.5)));
        // Hasta el final exacto, y un pequeño exceso por redondeo se ajusta
        assert_eq!(parse_trim(Some(&json!({ "start": 0, "end": 10 })), 10.0), Ok((0.0, 10.0)));
        assert_eq!(parse_trim(Some(&json!({ "start": 0, "end": 10.03 })), 10.0), Ok((0.0, 10.0)));
        let err = |v: Value| parse_trim(Some(&v), 10.0).unwrap_err();
        assert_eq!(err(json!({ "start": 5, "end": 5 })), "The start must be before the end");
        assert_eq!(err(json!({ "start": 6, "end": 5 })), "The start must be before the end");
        assert_eq!(err(json!({ "start": -1, "end": 5 })), "The start cannot be negative");
        assert_eq!(err(json!({ "start": 1, "end": 11 })), "The end is past the duration");
        assert_eq!(err(json!({ "start": 1, "end": 1.05 })), "The selection is too short");
        assert!(err(json!({ "start": 1 })).contains("end"));
        assert!(err(json!({ "start": "a", "end": 2 })).contains("start"));
        assert!(err(json!({ "start": null, "end": 2 })).contains("start"));
        assert!(parse_trim(None, 10.0).is_err());
    }

    #[test]
    fn valida_el_recuadro() {
        let ok = json!({ "x": 10, "y": 20, "w": 50, "h": 100 });
        assert_eq!(parse_crop(Some(&ok), 200, 400), Ok((10, 20, 50, 100)));
        // Justo hasta el borde
        assert_eq!(parse_crop(Some(&json!({ "x": 150, "y": 300, "w": 50, "h": 100 })), 200, 400), Ok((150, 300, 50, 100)));
        // Decimales: se redondean
        assert_eq!(parse_crop(Some(&json!({ "x": 9.6, "y": 0.2, "w": 49.7, "h": 10 })), 200, 400), Ok((10, 0, 50, 10)));
        let err = |v: Value| parse_crop(Some(&v), 200, 400).unwrap_err();
        assert_eq!(err(json!({ "x": 151, "y": 0, "w": 50, "h": 10 })), "The crop box goes outside the image");
        assert_eq!(err(json!({ "x": 0, "y": 301, "w": 50, "h": 100 })), "The crop box goes outside the image");
        assert_eq!(err(json!({ "x": 0, "y": 0, "w": 0, "h": 10 })), "The crop box is empty");
        assert_eq!(err(json!({ "x": -1, "y": 0, "w": 10, "h": 10 })), "The value \"x\" is not valid");
        assert!(err(json!({ "x": 0, "y": 0, "w": 10 })).contains("h"));
        assert!(err(json!({ "x": "0", "y": 0, "w": 10, "h": 10 })).contains("x"));
        assert!(parse_crop(None, 200, 400).is_err());
        // Un número enorme no desborda
        assert!(parse_crop(Some(&json!({ "x": 4294967295u64, "y": 0, "w": 4294967295u64, "h": 1 })), 200, 400).is_err());
    }

    #[test]
    fn valida_las_opciones() {
        let list = tools_of(Path::new("a.png"));
        let rotate = list.iter().find(|t| t.id == "rotate").unwrap();
        assert_eq!(check_option(rotate, Some("left")), Ok(Some("left".to_string())));
        assert!(check_option(rotate, Some("up")).is_err());
        assert!(check_option(rotate, None).is_err());
        // Las que no tienen opciones la ignoran
        let compress = list.iter().find(|t| t.id == "compress").unwrap();
        assert_eq!(check_option(compress, Some("loquesea")), Ok(None));
        assert_eq!(parse_resize("pct:50"), Ok(("pct", 50)));
        assert_eq!(parse_resize("w:1920"), Ok(("w", 1920)));
        assert!(parse_resize("pct:0").is_err());
        assert!(parse_resize("pct").is_err());
        assert!(parse_resize("w:-3").is_err());
    }

    #[test]
    fn regla_de_ahorro() {
        assert!(worth_keeping(1000, 500));
        assert!(worth_keeping(1000, 979));
        assert!(!worth_keeping(1000, 980));
        assert!(!worth_keeping(1000, 1000));
        assert!(!worth_keeping(1000, 1200));
        assert!(!worth_keeping(0, 0));
    }

    #[test]
    fn comandos_de_imagen() {
        let args = |ext: &str, tool: &str, option: Option<&str>, crop| image_args(ext, tool, option, crop, "/a/-x.png", "/a/y.png").unwrap();
        assert_eq!(args("png", "rotate", Some("left"), None), ["/a/-x.png", "-auto-orient", "-rotate", "270", "/a/y.png"]);
        assert_eq!(args("png", "rotate", Some("right"), None)[3], "90");
        assert_eq!(args("png", "flip", Some("horizontal"), None)[2], "-flop");
        assert_eq!(args("png", "flip", Some("vertical"), None)[2], "-flip");
        assert_eq!(args("png", "resize", Some("pct:50"), None)[3], "50%");
        assert_eq!(args("png", "resize", Some("w:800"), None)[3], "800x>");
        assert_eq!(args("png", "trim", None, Some((10, 20, 30, 40)))[2..5], ["-crop", "30x40+10+20", "+repage"]);
        assert_eq!(args("png", "strip_metadata", None, None)[2], "-strip");
        assert!(args("jpg", "compress", None, None).contains(&"75".to_string()));
        assert!(args("jpeg", "compress", None, None).contains(&"Plane".to_string()));
        assert!(args("png", "compress", None, None).contains(&"png:compression-level=9".to_string()));
        // GIF: fotogramas juntos antes y optimizados después
        let gif = args("gif", "flip", Some("vertical"), None);
        assert_eq!(gif[2], "-coalesce");
        assert_eq!(gif[gif.len() - 3..], ["-layers", "Optimize", "/a/y.png"]);
        // Formatos que no se pueden comprimir y casos que faltan datos
        assert_eq!(image_args("bmp", "compress", None, None, "a", "b").unwrap_err(), "The BMP format does not support compression");
        assert!(image_args("png", "trim", None, None, "a", "b").is_err());
        assert!(image_args("png", "rotate", None, None, "a", "b").is_err());
        assert!(image_args("png", "mute", None, None, "a", "b").is_err());
    }

    #[test]
    fn comandos_de_ffmpeg() {
        let build = |kind, ext: &str, tool: &str, option: Option<&str>, trim| ffmpeg_args(kind, ext, tool, option, trim, "/a/-v.mp4", "/a/o.mp4", false);
        let text = |args: Vec<String>| args.join(" ");
        let compress = text(build(Kind::Video, "mp4", "compress", None, None).unwrap());
        assert!(compress.contains("-i /a/-v.mp4"));
        assert!(compress.contains("libx264 -crf 28"));
        assert!(compress.contains("-c:a aac -b:a 128k"));
        assert!(compress.ends_with("/a/o.mp4"));
        assert!(text(build(Kind::Video, "webm", "compress", None, None).unwrap()).contains("libvpx-vp9 -crf 33"));
        assert!(text(build(Kind::Video, "avi", "compress", None, None).unwrap()).contains("libmp3lame"));
        let rotate = text(build(Kind::Video, "mp4", "rotate", Some("left"), None).unwrap());
        assert!(rotate.contains("-vf transpose=2") && rotate.contains("-c:a copy"));
        assert!(text(build(Kind::Video, "mp4", "rotate", Some("180"), None).unwrap()).contains("hflip,vflip"));
        assert!(text(build(Kind::Video, "mp4", "flip", Some("vertical"), None).unwrap()).contains("-vf vflip"));
        assert!(text(build(Kind::Video, "mp4", "resize", Some("pct:50"), None).unwrap()).contains("scale=trunc(iw*50/200)*2:-2"));
        assert!(text(build(Kind::Video, "mp4", "resize", Some("h:720"), None).unwrap()).contains("scale=-2:min(720\\,ih)"));
        let mute = text(build(Kind::Video, "mp4", "mute", None, None).unwrap());
        assert!(mute.contains("-c copy") && !mute.contains("-c:a"));
        let strip = text(build(Kind::Video, "mp4", "strip_metadata", None, None).unwrap());
        assert!(strip.contains("-map_metadata -1 -map_chapters -1 -c copy"));
        // Recortar: audio copiando, vídeo recodificando; -ss antes de -i y -t después
        let audio = text(build(Kind::Audio, "mp3", "trim", None, Some((1.5, 4.0))).unwrap());
        assert!(audio.contains("-ss 1.500 -i /a/-v.mp4 -t 2.500") && audio.contains("-c copy"));
        let video = text(build(Kind::Video, "mp4", "trim", None, Some((1.0, 2.0))).unwrap());
        assert!(video.contains("-ss 1.000 -i") && video.contains("libx264 -crf 20"));
        // Audio: comprimir según el formato; wav, flac y aiff no se pueden
        assert!(text(build(Kind::Audio, "ogg", "compress", None, None).unwrap()).contains("libvorbis -q:a 4"));
        assert!(text(build(Kind::Audio, "opus", "compress", None, None).unwrap()).contains("libopus -b:a 96k"));
        for ext in ["wav", "flac", "aiff"] {
            assert!(build(Kind::Audio, ext, "compress", None, None).is_err());
        }
        // Una herramienta de vídeo no vale para audio
        assert!(build(Kind::Audio, "mp3", "rotate", Some("left"), None).is_err());
        // Con progreso se pide a ffmpeg el avance por stdout
        let with = ffmpeg_args(Kind::Video, "mp4", "mute", None, None, "a", "b", true).unwrap();
        assert!(with.contains(&"pipe:1".to_string()));
    }

    #[test]
    fn calcula_los_picos() {
        // 4 muestras por cubo: picos 8192, 16384, 32767 (con signo) y 0
        let mut peaks = vec![0.0f32; 4];
        let samples: Vec<i16> = vec![0, 100, -8192, 5, 16384, 0, 0, -3, -32767, 0, 0, 0, 0, 0, 0, 0];
        add_samples(&mut peaks, 0, 4, &samples);
        assert_eq!(peaks[0], 8192.0 / 32768.0);
        assert_eq!(peaks[1], 16384.0 / 32768.0);
        assert_eq!(peaks[2], 32767.0 / 32768.0);
        assert_eq!(peaks[3], 0.0);
        // Lo que sobra por redondeo va al último cubo
        let mut peaks = vec![0.0f32; 2];
        add_samples(&mut peaks, 0, 2, &[0, 0, 0, 0, 0, 0, 1000]);
        assert_eq!(peaks[1], 1000.0 / 32768.0);
        // En trozos da lo mismo que de golpe
        let mut a = vec![0.0f32; 4];
        add_samples(&mut a, 0, 4, &samples[..5]);
        add_samples(&mut a, 5, 4, &samples[5..]);
        let mut b = vec![0.0f32; 4];
        add_samples(&mut b, 0, 4, &samples);
        assert_eq!(a, b);
        // -32768 no desborda
        let mut c = vec![0.0f32; 1];
        add_samples(&mut c, 0, 1, &[i16::MIN]);
        assert_eq!(c[0], 1.0);
    }

    #[test]
    fn normaliza_los_picos() {
        let mut peaks = vec![0.1f32, 0.2, 0.05];
        normalize(&mut peaks);
        assert_eq!(peaks, [0.5, 1.0, 0.25]);
        let mut silence = vec![0.0f32; 3];
        normalize(&mut silence);
        assert_eq!(silence, [0.0, 0.0, 0.0]);
    }

    // Convierte muestras a los bytes s16le que saca ffmpeg
    fn to_bytes(samples: &[i16]) -> Vec<u8> {
        samples.iter().flat_map(|s| s.to_le_bytes()).collect()
    }

    // Lector que entrega pocos bytes cada vez (y casi siempre parte alguna muestra por la mitad)
    struct Trickle(Vec<u8>, usize);

    impl Read for Trickle {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = 3.min(self.0.len() - self.1).min(buf.len());
            buf[..n].copy_from_slice(&self.0[self.1..self.1 + n]);
            self.1 += n;
            Ok(n)
        }
    }

    #[test]
    fn lee_picos_de_un_flujo() {
        let samples: Vec<i16> = (0..40).map(|i| if i % 10 == 3 { (i as i16 + 1) * 100 } else { 10 }).collect();
        let bytes = to_bytes(&samples);
        // Con la duración conocida (10 muestras por cubo) y sin ella (40 muestras / 4 cubos) sale igual
        let known = read_peaks(Trickle(bytes.clone(), 0), 4, Some(10));
        let unknown = read_peaks(Trickle(bytes.clone(), 0), 4, None);
        let whole = read_peaks(&bytes[..], 4, Some(10));
        assert_eq!(known, unknown);
        assert_eq!(known, whole);
        // Picos 400, 1400, 2400, 3400 normalizados
        assert_eq!(known.len(), 4);
        assert_eq!(known[3], 1.0);
        assert!((known[0] - 400.0 / 3400.0).abs() < 1e-6);
        // Más cubos que muestras y flujo vacío: no falla
        assert_eq!(read_peaks(&bytes[..4], 8, None).len(), 8);
        assert_eq!(read_peaks(&[][..], 3, Some(5)), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn lee_el_json_de_ffprobe() {
        let audio = r#"{"streams":[{"codec_type":"audio"}],"format":{"duration":"12.500000"}}"#;
        let info = parse_probe(audio, Kind::Audio).unwrap();
        assert_eq!(info, MediaInfo { kind: "audio".into(), width: None, height: None, duration: Some(12.5) });
        let video = r#"{"streams":[{"codec_type":"audio"},{"codec_type":"video","width":1280,"height":720}],"format":{"duration":"3.0"}}"#;
        let info = parse_probe(video, Kind::Video).unwrap();
        assert_eq!(info, MediaInfo { kind: "video".into(), width: Some(1280), height: Some(720), duration: Some(3.0) });
        // Rotación por matriz (también negativa) o por etiqueta: se intercambian; 180 no
        for rotation in ["90", "-90", "270"] {
            let json = format!(r#"{{"streams":[{{"codec_type":"video","width":1280,"height":720,"side_data_list":[{{"rotation":{rotation}}}]}}],"format":{{}}}}"#);
            let info = parse_probe(&json, Kind::Video).unwrap();
            assert_eq!((info.width, info.height, info.duration), (Some(720), Some(1280), None), "{rotation}");
        }
        let tagged = r#"{"streams":[{"codec_type":"video","width":1280,"height":720,"tags":{"rotate":"90"}}],"format":{}}"#;
        assert_eq!(parse_probe(tagged, Kind::Video).unwrap().width, Some(720));
        let upside = r#"{"streams":[{"codec_type":"video","width":1280,"height":720,"side_data_list":[{"rotation":180}]}],"format":{}}"#;
        assert_eq!(parse_probe(upside, Kind::Video).unwrap().width, Some(1280));
        // Sin pista de vídeo o JSON roto
        assert!(parse_probe(audio, Kind::Video).is_err());
        assert!(parse_probe("no es json", Kind::Audio).is_err());
    }

    #[test]
    fn rechaza_lo_que_no_se_puede() {
        let _guard = lock();
        let dir = test_dir("rechaza");
        let txt = dir.join("a.txt");
        std::fs::write(&txt, "hola").unwrap();
        assert_eq!(tool_run(&txt, "compress", None, None), Err("Tool not available for this file".to_string()));
        assert_eq!(tool_run(&dir.join("no_existe.png"), "compress", None, None), Err("File does not exist".to_string()));
        assert_eq!(tool_run(&dir, "compress", None, None), Err("File does not exist".to_string()));
        // Herramienta que ese tipo no tiene
        let png = dir.join("a.png");
        std::fs::write(&png, b"x").unwrap();
        assert!(tool_run(&png, "mute", None, None).is_err());
        let pdf = dir.join("a.pdf");
        std::fs::write(&pdf, b"x").unwrap();
        assert!(tool_run(&pdf, "rotate", Some("left"), None).is_err());
        // Sin opción o con una inventada
        assert_eq!(tool_run(&png, "rotate", None, None), Err("Choose an option".to_string()));
        assert_eq!(tool_run(&png, "rotate", Some("arriba"), None), Err("Invalid option".to_string()));
        // No se creó nada
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);
        assert!(media_info_blocking(&st(&txt)).is_err());
        assert!(preview_image_blocking(&st(&txt), 100).is_err());
        assert!(video_frame_blocking(&st(&png), 1.0, 100).is_err());
        assert!(waveform_blocking(&st(&png), 100).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ----- Imágenes -----

    #[test]
    fn comprime_jpg_y_png() {
        let _guard = lock();
        if !have(&["magick"]) {
            return;
        }
        let dir = test_dir("comprime_img");
        let jpg = dir.join("-foto [1].jpg");
        run("magick", &["-size", "800x600", "plasma:fractal", "-quality", "95", &st(&jpg)]);
        let out = tool_run(&jpg, "compress", None, None).unwrap();
        assert_eq!(out, st(&dir.join("-foto [1] (compressed).jpg")));
        assert!(size_of(&out) < size_of(&st(&jpg)) * 98 / 100);
        assert_eq!(dims(&out), (800, 600));
        // Otra vez: no se pisa el anterior
        let again = tool_run(&jpg, "compress", None, None).unwrap();
        assert_eq!(again, st(&dir.join("-foto [1] (compressed) 2.jpg")));

        // PNG guardado con poca compresión (captura de pantalla): se ahorra mucho sin perder nada
        let png = dir.join("-captura.png");
        run("magick", &["-size", "800x600", "gradient:white-blue", "-fill", "red", "-draw", "rectangle 50,50 300,300", "-define", "png:compression-level=1", &st(&png)]);
        let out = tool_run(&png, "compress", None, None).unwrap();
        assert!(size_of(&out) < size_of(&st(&png)) * 98 / 100);
        assert_eq!(dims(&out), (800, 600));

        // Ruido puro: no hay nada que ahorrar, se avisa y no queda salida
        let noise = dir.join("ruido.png");
        run("magick", &["-size", "300x300", "xc:gray50", "+noise", "Random", &st(&noise)]);
        let before = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(tool_run(&noise, "compress", None, None), Err("[optimized] Already optimized".to_string()));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), before);

        // BMP no admite compresión: la herramienta ni siquiera se ofrece
        let bmp = dir.join("a.bmp");
        run("magick", &["-size", "50x50", "xc:red", &st(&bmp)]);
        assert_eq!(tool_run(&bmp, "compress", None, None), Err("Tool not available for this file".to_string()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn comprime_webp_tiff_y_gif() {
        let _guard = lock();
        if !have(&["magick"]) {
            return;
        }
        let dir = test_dir("comprime_otros");
        run("magick", &["-size", "800x600", "plasma:fractal", &st(&dir.join("base.png"))]);
        for ext in ["webp", "tiff"] {
            let src = dir.join(format!("f.{ext}"));
            // Sin comprimir (tiff) o con calidad máxima (webp)
            run("magick", &[&st(&dir.join("base.png")), "-quality", "100", "-compress", "none", &st(&src)]);
            let out = tool_run(&src, "compress", None, None).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert!(size_of(&out) < size_of(&st(&src)), "{ext}");
        }
        // GIF animado: sigue teniendo los mismos fotogramas
        let gif = dir.join("a.gif");
        run("magick", &["-size", "200x100", "xc:red", "-size", "200x100", "xc:blue", "-size", "200x100", "xc:green", "-delay", "10", &st(&gif)]);
        let out = tool_run(&gif, "strip_metadata", None, None).unwrap();
        assert_eq!(output("magick", &["identify", "-format", "%n\n", &out]).lines().next(), Some("3"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn quita_metadatos_de_una_imagen_y_conserva_la_orientacion() {
        let _guard = lock();
        if !have(&["magick"]) {
            return;
        }
        let dir = test_dir("strip_img");
        let src = dir.join("-girada.jpg");
        // 400x200 con orientación 6: se ve de 200x400
        jpeg_with_exif(&src, 400, 200);
        assert_eq!(dims(&st(&src)), (200, 400));
        assert!(std::fs::read(&src).unwrap().windows(7).any(|w| w == b"secreto"));
        let out = tool_run(&src, "strip_metadata", None, None).unwrap();
        assert_eq!(out, st(&dir.join("-girada (no metadata).jpg")));
        assert!(!std::fs::read(&out).unwrap().windows(7).any(|w| w == b"secreto"));
        // Sin etiqueta de orientación y con la imagen ya girada
        assert_eq!(output("magick", &["identify", "-format", "%[orientation]", &format!("{out}[0]")]), "Undefined");
        assert_eq!(dims(&out), (200, 400));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recorta_una_imagen_orientada() {
        let _guard = lock();
        if !have(&["magick"]) {
            return;
        }
        let dir = test_dir("recorta_img");
        let src = dir.join("-r.jpg");
        jpeg_with_exif(&src, 400, 200);
        // Los límites son los de la imagen orientada (200 x 400)
        let crop = json!({ "x": 10, "y": 20, "w": 50, "h": 300 });
        let out = tool_run(&src, "trim", None, Some(crop)).unwrap();
        assert_eq!(out, st(&dir.join("-r (trimmed).jpg")));
        assert_eq!(dims(&out), (50, 300));
        // Un recuadro válido en la imagen sin orientar pero fuera de la orientada
        let too_wide = json!({ "x": 0, "y": 0, "w": 300, "h": 100 });
        assert_eq!(tool_run(&src, "trim", None, Some(too_wide)), Err("The crop box goes outside the image".to_string()));
        assert!(tool_run(&src, "trim", None, None).is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn gira_voltea_y_redimensiona_una_imagen() {
        let _guard = lock();
        if !have(&["magick"]) {
            return;
        }
        let dir = test_dir("gira_img");
        let src = dir.join("-g.png");
        // 400x200: mitad izquierda roja y mitad derecha azul
        run("magick", &["-size", "200x200", "xc:red", "-size", "200x200", "xc:blue", "+append", &st(&src)]);
        assert_eq!(dims(&st(&src)), (400, 200));
        let left = tool_run(&src, "rotate", Some("left"), None).unwrap();
        assert_eq!(left, st(&dir.join("-g (rotated).png")));
        assert_eq!(dims(&left), (200, 400));
        // Girar a la izquierda lleva el lado derecho (azul) arriba
        assert_eq!(output("magick", &[&left, "-format", "%[fx:p{0,0}.b]", "info:"]), "1");
        assert_eq!(dims(&tool_run(&src, "rotate", Some("right"), None).unwrap()), (200, 400));
        assert_eq!(dims(&tool_run(&src, "rotate", Some("180"), None).unwrap()), (400, 200));
        let flop = tool_run(&src, "flip", Some("horizontal"), None).unwrap();
        assert_eq!(output("magick", &[&flop, "-format", "%[fx:p{0,0}.b]", "info:"]), "1");
        // Voltear en vertical no cambia el lado rojo de arriba a la izquierda
        let flip = tool_run(&src, "flip", Some("vertical"), None).unwrap();
        assert_eq!(output("magick", &[&flip, "-format", "%[fx:p{0,0}.r]", "info:"]), "1");
        run("magick", &["-size", "100x100", "xc:red", "-size", "100x100", "xc:blue", "-append", &st(&dir.join("v.png"))]);
        let flip = tool_run(&dir.join("v.png"), "flip", Some("vertical"), None).unwrap();
        assert_eq!(output("magick", &[&flip, "-format", "%[fx:p{0,0}.b]", "info:"]), "1");
        // Redimensionar: porcentaje, ancho y sin ampliar
        assert_eq!(dims(&tool_run(&src, "resize", Some("pct:50"), None).unwrap()), (200, 100));
        assert_eq!(dims(&tool_run(&src, "resize", Some("pct:25"), None).unwrap()), (100, 50));
        assert_eq!(dims(&tool_run(&src, "resize", Some("w:800"), None).unwrap()), (400, 200));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn informacion_y_vista_previa_de_imagen() {
        let _guard = lock();
        if !have(&["magick"]) {
            return;
        }
        let dir = test_dir("info_img");
        let src = dir.join("-i[1].jpg");
        jpeg_with_exif(&src, 400, 200);
        let info = media_info_blocking(&st(&src)).unwrap();
        assert_eq!(info, MediaInfo { kind: "imagen".into(), width: Some(200), height: Some(400), duration: None });
        // GIF animado: el primer fotograma
        let gif = dir.join("a.gif");
        run("magick", &["-size", "30x20", "xc:red", "-size", "30x20", "xc:blue", "-delay", "5", &st(&gif)]);
        let info = media_info_blocking(&st(&gif)).unwrap();
        assert_eq!((info.width, info.height), (Some(30), Some(20)));
        // Vista previa: JPEG que cabe en 100 px, ya orientado (50 x 100)
        let url = preview_image_blocking(&st(&src), 100).unwrap();
        let data = url.strip_prefix("data:image/jpeg;base64,").expect("no es un data URL");
        assert!(data.starts_with("/9j/"));
        // PNG con transparencia
        let png = dir.join("t.png");
        run("magick", &["-size", "40x40", "xc:none", &st(&png)]);
        assert!(preview_image_blocking(&st(&png), 100).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ----- Vídeo y audio -----

    #[test]
    fn herramientas_de_video_con_ffmpeg() {
        let _guard = lock();
        if !have(&["ffmpeg", "ffprobe"]) {
            return;
        }
        let dir = test_dir("video");
        let src = dir.join("-v [1].mp4");
        make_video(&src, "2");
        let audio_streams = |p: &str| output("ffprobe", &["-v", "error", "-select_streams", "a", "-show_entries", "stream=codec_type", "-of", "csv=p=0", p]);
        let video_dims = |p: &str| output("ffprobe", &["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0", p]);
        let title = |p: &str| output("ffprobe", &["-v", "error", "-show_entries", "format_tags=title", "-of", "csv=p=0", p]);
        assert_eq!(title(&st(&src)), "Secreto");

        // Quitar metadatos: sin título y el vídeo y el audio siguen ahí
        let out = tool_run(&src, "strip_metadata", None, None).unwrap();
        assert_eq!(out, st(&dir.join("-v [1] (no metadata).mp4")));
        assert_eq!(title(&out), "");
        assert_eq!(video_dims(&out), "320,240");
        assert_eq!(audio_streams(&out), "audio");

        // Sin audio: ya no hay pista de audio y la imagen sigue igual
        let out = tool_run(&src, "mute", None, None).unwrap();
        assert_eq!(out, st(&dir.join("-v [1] (no audio).mp4")));
        assert_eq!(audio_streams(&out), "");
        assert_eq!(video_dims(&out), "320,240");

        // Girar: ancho y alto se intercambian (90°) o no (180°)
        assert_eq!(video_dims(&tool_run(&src, "rotate", Some("right"), None).unwrap()), "240,320");
        assert_eq!(video_dims(&tool_run(&src, "rotate", Some("left"), None).unwrap()), "240,320");
        assert_eq!(video_dims(&tool_run(&src, "rotate", Some("180"), None).unwrap()), "320,240");
        assert_eq!(video_dims(&tool_run(&src, "flip", Some("horizontal"), None).unwrap()), "320,240");
        assert_eq!(video_dims(&tool_run(&src, "flip", Some("vertical"), None).unwrap()), "320,240");
        // Redimensionar: porcentaje y alto, siempre pares, sin ampliar
        assert_eq!(video_dims(&tool_run(&src, "resize", Some("pct:50"), None).unwrap()), "160,120");
        assert_eq!(video_dims(&tool_run(&src, "resize", Some("pct:75"), None).unwrap()), "240,180");
        // h:480 es más que el original (240): no se amplía
        assert_eq!(video_dims(&tool_run(&src, "resize", Some("h:480"), None).unwrap()), "320,240");
        // Y uno más grande sí se reduce a 480 de alto (ancho par: 960x720 -> 640x480)
        let big = dir.join("grande.mp4");
        let args = ["-nostdin", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=960x720:rate=10", "-t", "1", "-pix_fmt", "yuv420p", &st(&big)];
        run("ffmpeg", &args);
        assert_eq!(video_dims(&tool_run(&big, "resize", Some("h:480"), None).unwrap()), "640,480");

        // Comprimir: sale más pequeño (el original está a calidad alta) o avisa de que ya está optimizado
        let out = tool_run(&src, "compress", None, None).unwrap();
        assert!(size_of(&out) < size_of(&st(&src)) * 98 / 100);
        assert_eq!(video_dims(&out), "320,240");
        assert_eq!(audio_streams(&out), "audio");

        // Recortar: el vídeo dura lo que el tramo y no se pasa del original
        let params = json!({ "start": 0.5, "end": 1.5 });
        let out = tool_run(&src, "trim", None, Some(params)).unwrap();
        assert_eq!(out, st(&dir.join("-v [1] (trimmed).mp4")));
        assert!((duration_of(&out) - 1.0).abs() < 0.15, "dura {}", duration_of(&out));
        // Parámetros inválidos: error y nada creado
        let before = std::fs::read_dir(&dir).unwrap().count();
        assert!(tool_run(&src, "trim", None, Some(json!({ "start": 1.0, "end": 5.0 }))).is_err());
        assert!(tool_run(&src, "trim", None, Some(json!({ "start": 1.5, "end": 1.0 }))).is_err());
        assert!(tool_run(&src, "trim", None, None).is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), before);

        // Información: medidas y duración
        let info = media_info_blocking(&st(&src)).unwrap();
        assert_eq!((info.kind.as_str(), info.width, info.height), ("video", Some(320), Some(240)));
        assert!((info.duration.unwrap() - 2.0).abs() < 0.1);
        // Un fotograma como JPEG; un tiempo pasado del final da el primero
        for seconds in [1.0, 99.0, 0.0] {
            let url = video_frame_blocking(&st(&src), seconds, 100).unwrap();
            assert!(url.starts_with("data:image/jpeg;base64,/9j/"));
        }
        assert!(video_frame_blocking(&st(&src), f64::NAN, 100).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn herramientas_de_video_en_cada_contenedor() {
        let _guard = lock();
        if !have(&["ffmpeg", "ffprobe"]) {
            return;
        }
        let dir = test_dir("contenedores");
        for ext in ["mov", "mkv", "avi", "webm", "wmv"] {
            let src = dir.join(format!("v.{ext}"));
            let args = ["-nostdin", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=160x120:rate=25", "-f", "lavfi", "-i", "sine=frequency=440", "-t", "2", &st(&src)];
            run("ffmpeg", &args);
            let dims = |p: &str| output("ffprobe", &["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0", p]);
            let rotated = tool_run(&src, "rotate", Some("left"), None).unwrap_or_else(|e| panic!("{ext} girar: {e}"));
            assert_eq!(dims(&rotated), "120,160", "{ext}");
            let trimmed = tool_run(&src, "trim", None, Some(json!({ "start": 0.5, "end": 1.5 }))).unwrap_or_else(|e| panic!("{ext} recortar: {e}"));
            assert!((duration_of(&trimmed) - 1.0).abs() < 0.2, "{ext}: dura {}", duration_of(&trimmed));
            tool_run(&src, "strip_metadata", None, None).unwrap_or_else(|e| panic!("{ext} metadatos: {e}"));
            tool_run(&src, "mute", None, None).unwrap_or_else(|e| panic!("{ext} sin audio: {e}"));
            // Comprimir un archivo tan pequeño puede no ahorrar nada: ese aviso también vale
            match tool_run(&src, "compress", None, None) {
                Ok(out) => assert!(size_of(&out) > 0, "{ext}"),
                Err(e) => assert_eq!(e, "[optimized] Already optimized", "{ext}"),
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn un_video_con_rotacion_se_mide_girado() {
        let _guard = lock();
        if !have(&["ffmpeg", "ffprobe"]) {
            return;
        }
        let dir = test_dir("rotado");
        let plain = dir.join("normal.mp4");
        make_video(&plain, "1");
        // Mismo vídeo con la matriz de rotación de 90° (como los de un móvil en vertical)
        let rotated = dir.join("rotado.mp4");
        run("ffmpeg", &["-nostdin", "-v", "error", "-display_rotation:v:0", "90", "-i", &st(&plain), "-c", "copy", &st(&rotated)]);
        let info = media_info_blocking(&st(&rotated)).unwrap();
        assert_eq!((info.width, info.height), (Some(240), Some(320)));
        // Al recodificar, ffmpeg aplica la rotación: la salida ya mide lo mismo que dice media_info
        let out = tool_run(&rotated, "flip", Some("horizontal"), None).unwrap();
        let info = media_info_blocking(&out).unwrap();
        assert_eq!((info.width, info.height), (Some(240), Some(320)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn herramientas_de_audio_con_ffmpeg() {
        let _guard = lock();
        if !have(&["ffmpeg", "ffprobe"]) {
            return;
        }
        let dir = test_dir("audio");
        let title = |p: &str| output("ffprobe", &["-v", "error", "-show_entries", "format_tags=title:stream_tags=title", "-of", "csv=p=0", p]);
        // Un archivo de cada formato con título, y ruido rosa para que comprimir ahorre
        for (ext, codec) in [("mp3", "libmp3lame"), ("m4a", "aac"), ("ogg", "libvorbis"), ("opus", "libopus"), ("wma", "wmav2"), ("wav", "pcm_s16le"), ("flac", "flac")] {
            let src = dir.join(format!("-a [1].{ext}"));
            let high = if ext == "ogg" { vec!["-q:a", "8"] } else if matches!(ext, "wav" | "flac") { vec![] } else { vec!["-b:a", "256k"] };
            let mut args = vec!["-nostdin", "-v", "error", "-f", "lavfi", "-i", "anoisesrc=d=6:c=pink:a=0.3", "-c:a", codec];
            args.extend(high);
            args.extend(["-metadata", "title=Secreto"]);
            let out = st(&src);
            args.push(&out);
            run("ffmpeg", &args);
            assert!(title(&out).contains("Secreto"), "{ext}: el original no trae título");

            // Quitar metadatos
            let stripped = tool_run(&src, "strip_metadata", None, None).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert_eq!(stripped, st(&dir.join(format!("-a [1] (no metadata).{ext}"))));
            assert_eq!(title(&stripped), "", "{ext}: queda el título");
            assert!((duration_of(&stripped) - duration_of(&out)).abs() < 0.1, "{ext}");

            // Recortar de 1 a 3 s: dura unos 2 s
            let trimmed = tool_run(&src, "trim", None, Some(json!({ "start": 1.0, "end": 3.0 }))).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert!((duration_of(&trimmed) - 2.0).abs() < 0.15, "{ext}: dura {}", duration_of(&trimmed));

            // Comprimir
            let compressed = tool_run(&src, "compress", None, None);
            if matches!(ext, "wav" | "flac") {
                assert_eq!(compressed, Err("Tool not available for this file".to_string()));
            } else {
                let compressed = compressed.unwrap_or_else(|e| panic!("{ext}: {e}"));
                assert!(size_of(&compressed) < size_of(&out) * 98 / 100, "{ext}");
                assert!((duration_of(&compressed) - 6.0).abs() < 0.3, "{ext}");
            }
        }
        // Información y forma de onda
        let info = media_info_blocking(&st(&dir.join("-a [1].mp3"))).unwrap();
        assert_eq!((info.kind.as_str(), info.width, info.height), ("audio", None, None));
        assert!((info.duration.unwrap() - 6.0).abs() < 0.2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn forma_de_onda_de_un_audio() {
        let _guard = lock();
        if !have(&["ffmpeg", "ffprobe"]) {
            return;
        }
        let dir = test_dir("onda");
        // 2 s de silencio y 2 s de tono: la primera mitad vale 0 y la segunda llega a 1
        let wav = dir.join("-onda [1].wav");
        let args = ["-nostdin", "-v", "error", "-f", "lavfi", "-i", "anullsrc=r=8000:cl=mono", "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=8000",
            "-filter_complex", "[0]atrim=0:2[a];[1]atrim=0:2[b];[a][b]concat=n=2:v=0:a=1", &st(&wav)];
        run("ffmpeg", &args);
        let peaks = waveform_blocking(&st(&wav), 100).unwrap();
        assert_eq!(peaks.len(), 100);
        assert!(peaks.iter().all(|p| (0.0..=1.0).contains(p)));
        assert!(peaks[..45].iter().all(|p| *p < 0.01), "el silencio no es silencio");
        assert!(peaks[55..].iter().all(|p| *p > 0.9), "el tono no suena");
        assert!(peaks.iter().cloned().fold(0.0, f32::max) == 1.0);
        // También de un vídeo
        let mp4 = dir.join("v.mp4");
        make_video(&mp4, "1");
        assert_eq!(waveform_blocking(&st(&mp4), 50).unwrap().len(), 50);
        assert!(waveform_blocking(&st(&wav), 0).is_err());
        // Un vídeo sin audio no tiene forma de onda
        let mute = tool_run(&mp4, "mute", None, None).unwrap();
        assert!(waveform_blocking(&mute, 50).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ----- PDF, progreso y cancelación -----

    #[test]
    fn comprime_un_pdf() {
        let _guard = lock();
        if !have(&["gs", "magick"]) {
            return;
        }
        let dir = test_dir("pdf");
        let pdf = dir.join("-doc %d [1].pdf");
        // ImageMagick interpreta el "%d" del nombre de salida: se crea con otro nombre y se renombra
        let plain = dir.join("tmp.pdf");
        run("magick", &["-size", "1000x800", "plasma:fractal", "-quality", "100", &st(&plain)]);
        std::fs::rename(&plain, &pdf).unwrap();
        let out = tool_run(&pdf, "compress", None, None).unwrap();
        assert_eq!(out, st(&dir.join("-doc %d [1] (compressed).pdf")));
        assert!(size_of(&out) < size_of(&st(&pdf)) * 98 / 100);
        // Sigue siendo un PDF de una página
        assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF"));
        // Un archivo que no es un PDF falla y no deja salida
        let fake = dir.join("falso.pdf");
        std::fs::write(&fake, "no soy un pdf").unwrap();
        let before = std::fs::read_dir(&dir).unwrap().count();
        assert!(tool_run(&fake, "compress", None, None).is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), before);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn el_progreso_llega_en_una_herramienta_de_ffmpeg() {
        let _guard = lock();
        if !have(&["ffmpeg", "ffprobe"]) {
            return;
        }
        let dir = test_dir("progreso");
        let src = dir.join("p.mp4");
        make_video(&src, "6");
        // En vez del AppHandle real se guarda cada porcentaje en una lista
        let seen = Arc::new(Mutex::new(Vec::<f64>::new()));
        let seen_in = Arc::clone(&seen);
        let progress: Progress = Arc::new(move |p| seen_in.lock().unwrap().push(p));
        run_tool_with(&st(&src), "compress", None, None, Some(progress)).unwrap();
        let seen = seen.lock().unwrap();
        assert!(!seen.is_empty(), "no se emitió ningún porcentaje");
        assert!(seen.windows(2).all(|w| w[0] < w[1]), "no es creciente: {seen:?}");
        assert!(seen.iter().all(|p| (0.0..=99.0).contains(p)), "fuera de rango: {seen:?}");
        // Recortar mide el avance sobre el tramo: llega cerca de 99 aunque el original sea más largo
        let seen = Arc::new(Mutex::new(Vec::<f64>::new()));
        let seen_in = Arc::clone(&seen);
        let progress: Progress = Arc::new(move |p| seen_in.lock().unwrap().push(p));
        let params = json!({ "start": 1.0, "end": 3.0 });
        run_tool_with(&st(&src), "trim", None, Some(&params), Some(progress)).unwrap();
        let last = seen.lock().unwrap().last().cloned().unwrap_or(0.0);
        assert!(last > 50.0, "el progreso del tramo no avanzó: {last}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cancelar_una_herramienta_no_deja_salida() {
        let _guard = lock();
        if !have(&["ffmpeg", "ffprobe"]) {
            return;
        }
        let dir = test_dir("cancela");
        // Un webm largo y comprimirlo con VP9 (lento): da tiempo de sobra a cancelar
        let src = dir.join("largo.webm");
        let args = ["-nostdin", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=25", "-t", "20", "-c:v", "libvpx", "-deadline", "realtime", "-cpu-used", "8", &st(&src)];
        run("ffmpeg", &args);
        let canceller = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(700));
            CANCEL.store(true, Ordering::SeqCst);
        });
        let start = Instant::now();
        let progress: Progress = Arc::new(|_| {});
        let result = run_tool_with(&st(&src), "compress", None, None, Some(progress));
        canceller.join().unwrap();
        CANCEL.store(false, Ordering::SeqCst);
        assert_eq!(result, Err("[cancelled] Cancelled".to_string()));
        assert!(start.elapsed() < Duration::from_secs(8));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "solo debe quedar el original");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ----- Idiomas -----

    #[test]
    fn etiquetas_en_los_dos_idiomas() {
        use crate::lang::{set_lang, Lang};
        let labels = |name: &str| -> Vec<String> { tools_of(Path::new(name)).iter().map(|t| t.label.clone()).collect() };
        set_lang(Lang::En);
        assert_eq!(labels("a.mp4"), ["REDUCE SIZE", "METADATA", "TRIM", "ROTATE", "FLIP", "RESIZE", "NO AUDIO", "ZIP"]);
        let video = tools_of(Path::new("a.mp4"));
        assert_eq!(video[3].options[0].label, "90° ↺");
        assert_eq!(video[5].options[3].label, "1080p");
        set_lang(Lang::Es);
        assert_eq!(labels("a.mp4"), ["REDUCIR TAMAÑO", "METADATOS", "RECORTAR", "GIRAR", "VOLTEAR", "REDIMENSIONAR", "SIN AUDIO", "ZIP"]);
        // Los ids no cambian con el idioma
        assert_eq!(ids(&tools_of(Path::new("a.mp4"))), ["compress", "strip_metadata", "trim", "rotate", "flip", "resize", "mute", "zip"]);
        let image = tools_of(Path::new("a.png"));
        assert_eq!(image[4].options[0].label, "Horizontal");
        assert_eq!(image[5].options[3].label, "1920 px");
        set_lang(Lang::En);
    }

    #[test]
    fn sufijos_de_salida_en_los_dos_idiomas() {
        use crate::lang::{set_lang, Lang};
        let src = Path::new("/x/foto.JPG");
        let tools = ["compress", "strip_metadata", "trim", "rotate", "flip", "resize", "mute"];
        let en = ["compressed", "no metadata", "trimmed", "rotated", "flipped", "resized", "no audio"];
        let es = ["comprimido", "sin metadatos", "recortado", "girado", "volteado", "redimensionado", "sin audio"];
        for (lang, names) in [(Lang::En, en), (Lang::Es, es)] {
            set_lang(lang);
            for (tool, name) in tools.iter().zip(names) {
                assert_eq!(output_path(src, tool), Path::new(&format!("/x/foto ({name}).JPG")));
            }
        }
        set_lang(Lang::En);
    }

    #[test]
    fn errores_en_espanol() {
        use crate::lang::{set_lang, Lang};
        set_lang(Lang::Es);
        let err = |v: Value| parse_trim(Some(&v), 10.0).unwrap_err();
        assert_eq!(err(json!({ "start": 5, "end": 5 })), "El inicio debe ser menor que el final");
        assert_eq!(err(json!({ "start": -1, "end": 5 })), "El inicio no puede ser negativo");
        assert_eq!(err(json!({ "start": 1, "end": 11 })), "El final pasa de la duración");
        assert_eq!(err(json!({ "start": 1, "end": 1.05 })), "El tramo es demasiado corto");
        assert_eq!(err(json!({ "start": 1 })), "Falta «end» o no es un número");
        let crop = |v: Value| parse_crop(Some(&v), 200, 400).unwrap_err();
        assert_eq!(crop(json!({ "x": 151, "y": 0, "w": 50, "h": 10 })), "El recuadro se sale de la imagen");
        assert_eq!(crop(json!({ "x": 0, "y": 0, "w": 0, "h": 10 })), "El recuadro está vacío");
        assert_eq!(crop(json!({ "x": -1, "y": 0, "w": 10, "h": 10 })), "El valor «x» no es válido");
        assert_eq!(image_args("bmp", "compress", None, None, "a", "b").unwrap_err(), "El formato BMP no admite compresión");
        let name = |n: &str| custom_name(Some(&json!({ "nombre": n })), "jpg").unwrap_err();
        assert_eq!(name("  "), "Escribe un nombre para el archivo nuevo");
        assert_eq!(name("a/b"), "El nombre no es válido: no puede llevar «/» ni ser «.» o «..»");
        assert_eq!(name(&"x".repeat(300)), "El nombre es demasiado largo");
        let list = tools_of(Path::new("a.png"));
        let rotate = list.iter().find(|t| t.id == "rotate").unwrap();
        assert_eq!(check_option(rotate, None), Err("Falta elegir una opción".to_string()));
        assert_eq!(check_option(rotate, Some("up")), Err("Opción no válida".to_string()));
        set_lang(Lang::En);
        assert_eq!(err(json!({ "start": 1 })), "\"end\" is missing or is not a number");
        assert_eq!(name("a/b"), "The name is not valid: it cannot contain \"/\" or be \".\" or \"..\"");
        assert_eq!(name("  "), "Type a name for the new file");
        assert_eq!(name(&"x".repeat(300)), "The name is too long");
    }

    #[test]
    fn errores_de_run_tool_en_los_dos_idiomas() {
        use crate::lang::{set_lang, Lang};
        let _guard = lock();
        let dir = test_dir("idiomas");
        let txt = dir.join("a.txt");
        std::fs::write(&txt, b"x").unwrap();
        set_lang(Lang::En);
        assert_eq!(tool_run(&txt, "compress", None, None), Err("Tool not available for this file".to_string()));
        assert_eq!(tool_run(&dir.join("no.png"), "compress", None, None), Err("File does not exist".to_string()));
        set_lang(Lang::Es);
        assert_eq!(tool_run(&txt, "compress", None, None), Err("Herramienta no disponible para este archivo".to_string()));
        assert_eq!(tool_run(&dir.join("no.png"), "compress", None, None), Err("El archivo no existe".to_string()));
        set_lang(Lang::En);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn codigos_cancelado_y_optimizado() {
        use crate::lang::{set_lang, Lang};
        let dir = test_dir("codigos");
        let (src, out) = (dir.join("a.jpg"), dir.join("a (x).jpg"));
        std::fs::write(&src, vec![0u8; 1000]).unwrap();
        std::fs::write(&out, vec![0u8; 1000]).unwrap();
        for (lang, text) in [(Lang::En, "[optimized] Already optimized"), (Lang::Es, "[optimized] Ya está optimizado")] {
            set_lang(lang);
            let e = check_saving("compress", &src, &out).unwrap_err();
            assert_eq!(e, text);
            assert!(has_code(&e, OPTIMIZED) && !has_code(&e, CANCELLED));
        }
        // Otras herramientas no comprueban el ahorro
        assert_eq!(check_saving("rotate", &src, &out), Ok(()));
        // Si ahorra, no hay error
        std::fs::write(&out, vec![0u8; 500]).unwrap();
        assert_eq!(check_saving("compress", &src, &out), Ok(()));
        set_lang(Lang::En);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
