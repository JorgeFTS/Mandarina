use crate::converters::{kill_group, tool_exists, TempDir};
use crate::lang::tr;
use crate::tr;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const IMAGE_EXT: [&str; 11] = ["jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "tif", "avif", "bmp", "gif"];
const VIDEO_EXT: [&str; 6] = ["mp4", "mov", "mkv", "webm", "avi", "wmv"];

// Maximum side of the thumbnail and maximum wait time
const SIZE: &str = "200";
const MAX_WAIT: Duration = Duration::from_secs(3);

const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

// Error messages of the thumbnails
fn no_thumbnail() -> String {
    tr("No thumbnail", "Sin miniatura")
}

// tools.rs compares it to tell "it took too long" apart from the other failures
pub(crate) fn timeout_message() -> String {
    tr("The thumbnail took too long", "La miniatura tardó demasiado")
}

// Standard Base64 (RFC 4648) with "=" padding
pub(crate) fn base64(bytes: &[u8]) -> String {
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        // Up to 3 bytes are gathered into a 24-bit number and 4 groups of 6 bits are taken out
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        for i in 0..4 {
            // With 1 byte there are 2 useful letters and with 2 bytes there are 3: the rest is padding
            if i <= chunk.len() {
                text.push(TABLE[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                text.push('=');
            }
        }
    }
    text
}

// Runs the program and lets `read` process its stdout (in a thread, so the process does not block
// when the pipe fills up). Returns whatever `read` returns if the program ends well. If `deadline` is passed,
// it kills the process (and its children) and returns Err. It uses neither CANCEL nor run_in: a thumbnail (or the preview
// or the waveform of the tools) never cancels, and is never cancelled by, the conversion.
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

// Same, but returning the whole stdout (it may come back empty; the caller checks it)
pub(crate) fn capture(program: &str, args: &[&str], deadline: Instant) -> Result<Vec<u8>, String> {
    capture_with(program, args, deadline, |mut stream| {
        let mut bytes = Vec::new();
        let _ = stream.read_to_end(&mut bytes);
        bytes
    })
}

// JPEG thumbnail of an image with ImageMagick.
// The path is absolute, so it never starts with "-". The trailing "[0]" is the frame
// selection (only the first one, for animated GIFs); ImageMagick tries the literal name first,
// so a file with brackets, ":" or "*" ("a[1].png") opens fine. The "file:" prefix is not used
// because with it ImageMagick fails with these names (tested).
fn image_thumb(path: &str, deadline: Instant) -> Result<Vec<u8>, String> {
    if !tool_exists("magick") {
        return Err(no_thumbnail());
    }
    let first_frame = format!("{path}[0]");
    let size = format!("{SIZE}x{SIZE}");
    capture("magick", &[&first_frame, "-auto-orient", "-thumbnail", &size, "jpg:-"], deadline)
}

// SVG: rsvg-convert to a small PNG in a temporary folder and then ImageMagick to JPEG
fn svg_thumb(path: &str, deadline: Instant) -> Result<Vec<u8>, String> {
    if !tool_exists("rsvg-convert") {
        return Err(no_thumbnail());
    }
    let tmp = TempDir::new()?;
    let png = tmp.path().join("mini.png").to_string_lossy().to_string();
    // Neither -w nor -h may exceed 200; -a keeps the aspect ratio
    capture("rsvg-convert", &["-a", "-w", SIZE, "-h", SIZE, "-o", &png, "--", path], deadline)?;
    image_thumb(&png, deadline)
}

// Video: one frame at second 1 and, if the video is shorter, the first one
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

// PDF: first page with pdftoppm. With no output name it writes the image to stdout.
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

    // Temporary folder of each test
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

    // Decodes the data URL and checks that it is a JPEG
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
        // High bytes: 0xFF 0xD8 0xFF is the start of a JPEG
        assert_eq!(base64(&[0xFF, 0xD8, 0xFF]), "/9j/");
    }

    #[test]
    fn otros_tipos_no_tienen_miniatura() {
        let dir = test_dir("otros");
        let txt = dir.join("a.txt");
        std::fs::write(&txt, "hola").unwrap();
        assert_eq!(thumbnail_blocking(txt.to_str().unwrap()), Err("No thumbnail".to_string()));
        // Neither a folder nor a file that does not exist
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
        // Animated GIF: only the first frame, and the thumbnail is a single JPEG
        let gif = dir.join("anim.gif");
        run("magick", &["-size", "300x300", "xc:red", "-size", "300x300", "xc:blue", "-delay", "5", gif.to_str().unwrap()]);
        assert_jpeg(&thumbnail_blocking(gif.to_str().unwrap()).unwrap());
        // Nothing is written next to the original
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
            // One of 3 s (frame at second 1) and another of half a second (first frame)
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
        // sh launches a grandchild "sleep": it must die too
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
