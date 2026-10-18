use crate::converters::{kill_group, tool_exists, TempDir};
use crate::lang::tr;
use crate::tr;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const IMAGE_EXT: [&str; 11] = ["jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "tif", "avif", "bmp", "gif"];
const VIDEO_EXT: [&str; 6] = ["mp4", "mov", "mkv", "webm", "avi", "wmv"];

// Lado máximo de la miniatura y tiempo máximo de espera
const SIZE: &str = "200";
const MAX_WAIT: Duration = Duration::from_secs(3);

const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

// Mensajes de error de las miniaturas
fn no_thumbnail() -> String {
    tr("No thumbnail", "Sin miniatura")
}

// tools.rs lo compara para distinguir "tardó demasiado" de los demás fallos
pub(crate) fn timeout_message() -> String {
    tr("The thumbnail took too long", "La miniatura tardó demasiado")
}

// Base64 estándar (RFC 4648) con relleno "="
pub(crate) fn base64(bytes: &[u8]) -> String {
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        // Se juntan hasta 3 bytes en un número de 24 bits y se sacan 4 grupos de 6 bits
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        for i in 0..4 {
            // Con 1 byte hay 2 letras útiles y con 2 bytes hay 3: el resto es relleno
            if i <= chunk.len() {
                text.push(TABLE[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                text.push('=');
            }
        }
    }
    text
}

// Ejecuta el programa y deja que `read` procese su stdout (en un hilo, para que el proceso no se bloquee
// al llenarse la tubería). Devuelve lo que devuelva `read` si el programa termina bien. Si pasa `deadline`,
// mata el proceso (y sus hijos) y devuelve Err. No usa CANCEL ni run_in: una miniatura (o la vista previa
// o la forma de onda de las herramientas) nunca cancela ni se cancela con la conversión.
pub(crate) fn capture_with<T: Send + 'static>(
    program: &str,
    args: &[&str],
    deadline: Instant,
    read: impl FnOnce(std::process::ChildStdout) -> T + Send + 'static,
) -> Result<T, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|_| no_thumbnail())?;
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || stdout.map(read));
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                kill_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(timeout_message());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return Err(no_thumbnail()),
        }
    };
    let result = reader.join().ok().flatten();
    match result {
        Some(value) if status.success() => Ok(value),
        _ => Err(no_thumbnail()),
    }
}

// Igual, pero devolviendo el stdout entero (puede venir vacío; lo comprueba quien llama)
pub(crate) fn capture(program: &str, args: &[&str], deadline: Instant) -> Result<Vec<u8>, String> {
    capture_with(program, args, deadline, |mut stream| {
        let mut bytes = Vec::new();
        let _ = stream.read_to_end(&mut bytes);
        bytes
    })
}

// Miniatura JPEG de una imagen con ImageMagick.
// La ruta es absoluta, así que nunca empieza por "-". El "[0]" del final es la selección de
// fotograma (solo el primero, para GIF animados); ImageMagick prueba antes el nombre literal,
// por lo que un archivo con corchetes, ":" o "*" ("a[1].png") se abre bien. No se usa el prefijo
// "file:" porque con él ImageMagick falla con estos nombres (probado).
fn image_thumb(path: &str, deadline: Instant) -> Result<Vec<u8>, String> {
    if !tool_exists("magick") {
        return Err(no_thumbnail());
    }
    let first_frame = format!("{path}[0]");
    let size = format!("{SIZE}x{SIZE}");
    capture("magick", &[&first_frame, "-auto-orient", "-thumbnail", &size, "jpg:-"], deadline)
}

// SVG: rsvg-convert a un PNG pequeño en una carpeta temporal y luego ImageMagick a JPEG
fn svg_thumb(path: &str, deadline: Instant) -> Result<Vec<u8>, String> {
    if !tool_exists("rsvg-convert") {
        return Err(no_thumbnail());
    }
    let tmp = TempDir::new()?;
    let png = tmp.path().join("mini.png").to_string_lossy().to_string();
    // Ni -w ni -h pueden pasar de 200; -a mantiene la proporción
    capture("rsvg-convert", &["-a", "-w", SIZE, "-h", SIZE, "-o", &png, "--", path], deadline)?;
    image_thumb(&png, deadline)
}

// Vídeo: un fotograma en el segundo 1 y, si el vídeo es más corto, el primero
fn video_thumb(path: &str, deadline: Instant) -> Result<Vec<u8>, String> {
    if !tool_exists("ffmpeg") {
        return Err(no_thumbnail());
    }
    let scale = format!("scale={SIZE}:-2");
    let base = ["-nostdin", "-v", "error"];
    let rest = ["-frames:v", "1", "-vf", &scale, "-f", "image2pipe", "-vcodec", "mjpeg", "-"];
    let mut with_seek: Vec<&str> = base.to_vec();
    with_seek.extend(["-ss", "1", "-i", path]);
    with_seek.extend(rest);
    match capture("ffmpeg", &with_seek, deadline) {
        Ok(bytes) if !bytes.is_empty() => return Ok(bytes),
        _ => {}
    }
    let mut from_start: Vec<&str> = base.to_vec();
    from_start.extend(["-i", path]);
    from_start.extend(rest);
    capture("ffmpeg", &from_start, deadline)
}

// PDF: primera página con pdftoppm. Sin nombre de salida escribe la imagen en stdout.
fn pdf_thumb(path: &str, deadline: Instant) -> Result<Vec<u8>, String> {
    if !tool_exists("pdftoppm") {
        return Err(no_thumbnail());
    }
    capture("pdftoppm", &["-f", "1", "-l", "1", "-scale-to", SIZE, "-jpeg", path], deadline)
}

fn thumbnail_blocking(path: &str) -> Result<String, String> {
    let src = std::path::absolute(path).map_err(|_| no_thumbnail())?;
    if !src.is_file() {
        return Err(no_thumbnail());
    }
    let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let src = src.to_string_lossy().to_string();
    let deadline = Instant::now() + MAX_WAIT;
    let jpeg = if IMAGE_EXT.contains(&ext.as_str()) {
        image_thumb(&src, deadline)?
    } else if ext == "svg" {
        svg_thumb(&src, deadline)?
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        video_thumb(&src, deadline)?
    } else if ext == "pdf" {
        pdf_thumb(&src, deadline)?
    } else {
        return Err(no_thumbnail());
    };
    if jpeg.is_empty() {
        return Err(no_thumbnail());
    }
    Ok(format!("data:image/jpeg;base64,{}", base64(&jpeg)))
}

#[tauri::command]
pub async fn thumbnail(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || thumbnail_blocking(&path))
        .await
        .map_err(|e| tr!("Internal error: {e}", "Error interno: {e}", e = e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    // Carpeta temporal propia de cada prueba
    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mandarina_th_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn run(program: &str, args: &[&str]) {
        let status = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).status().unwrap();
        assert!(status.success(), "{program} falló");
    }

    // Decodifica el data URL y comprueba que sea un JPEG
    fn assert_jpeg(url: &str) {
        let data = url.strip_prefix("data:image/jpeg;base64,").expect("no es un data URL");
        assert!(data.starts_with("/9j/"), "no empieza como un JPEG");
    }

    #[test]
    fn base64_de_la_rfc_4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        // Bytes altos: 0xFF 0xD8 0xFF es el inicio de un JPEG
        assert_eq!(base64(&[0xFF, 0xD8, 0xFF]), "/9j/");
    }

    #[test]
    fn otros_tipos_no_tienen_miniatura() {
        let dir = test_dir("otros");
        let txt = dir.join("a.txt");
        std::fs::write(&txt, "hola").unwrap();
        assert_eq!(thumbnail_blocking(txt.to_str().unwrap()), Err("No thumbnail".to_string()));
        // Ni una carpeta ni un archivo que no existe
        assert_eq!(thumbnail_blocking(dir.to_str().unwrap()), Err("No thumbnail".to_string()));
        assert_eq!(thumbnail_blocking("/no/existe.png"), Err("No thumbnail".to_string()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn miniatura_de_imagenes_con_nombres_raros() {
        if !tool_exists("magick") {
            return;
        }
        let dir = test_dir("imagen");
        for name in ["-x.png", "a[1].png", "[0].png", "a [0] b.png", "a:b.png", "a*.png", "x[2x2].png"] {
            let png = dir.join(name);
            run("magick", &["-size", "600x300", "xc:red", png.to_str().unwrap()]);
            let url = thumbnail_blocking(png.to_str().unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_jpeg(&url);
        }
        // GIF animado: solo el primer fotograma, y la miniatura es un único JPEG
        let gif = dir.join("anim.gif");
        run("magick", &["-size", "300x300", "xc:red", "-size", "300x300", "xc:blue", "-delay", "5", gif.to_str().unwrap()]);
        assert_jpeg(&thumbnail_blocking(gif.to_str().unwrap()).unwrap());
        // No se escribe nada junto al original
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 8);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn miniatura_de_svg_pdf_y_video() {
        let dir = test_dir("otros_tipos");
        if tool_exists("rsvg-convert") && tool_exists("magick") {
            let svg = dir.join("-s[0].svg");
            std::fs::write(
                &svg,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"400\" height=\"100\"><rect width=\"400\" height=\"100\" fill=\"orange\"/></svg>",
            )
            .unwrap();
            assert_jpeg(&thumbnail_blocking(svg.to_str().unwrap()).unwrap());
        }
        if tool_exists("pdftoppm") && tool_exists("magick") {
            let pdf = dir.join("-p[0].pdf");
            run("magick", &["-size", "300x400", "xc:white", pdf.to_str().unwrap()]);
            assert_jpeg(&thumbnail_blocking(pdf.to_str().unwrap()).unwrap());
        }
        if tool_exists("ffmpeg") {
            // Uno de 3 s (fotograma en el segundo 1) y otro de medio segundo (primer fotograma)
            for (name, secs) in [("largo.mp4", "3"), ("corto.mp4", "0.5")] {
                let video = dir.join(name);
                let args = ["-nostdin", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=10", "-t", secs, "-pix_fmt", "yuv420p", video.to_str().unwrap()];
                run("ffmpeg", &args);
                assert_jpeg(&thumbnail_blocking(video.to_str().unwrap()).unwrap_or_else(|e| panic!("{name}: {e}")));
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn se_corta_si_tarda_demasiado() {
        // sh lanza un "sleep" nieto: también debe morir
        let start = Instant::now();
        let result = capture("sh", &["-c", "sleep 31.78; true"], Instant::now() + Duration::from_millis(300));
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
        if tool_exists("pgrep") {
            let found = Command::new("pgrep").args(["-f", "^sleep 31[.]78$"]).output().unwrap();
            assert!(found.stdout.is_empty(), "queda un proceso hijo");
        }
    }

    #[test]
    fn mensajes_en_espanol() {
        crate::lang::set_lang(crate::lang::Lang::Es);
        assert_eq!(thumbnail_blocking("/no/existe.png"), Err("Sin miniatura".to_string()));
        assert_eq!(timeout_message(), "La miniatura tardó demasiado");
        crate::lang::set_lang(crate::lang::Lang::En);
        assert_eq!(timeout_message(), "The thumbnail took too long");
    }
}
