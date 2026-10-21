use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::config::{self, Config};
use crate::lang::{cancelled_error, has_code, tr, CANCELLED};
use crate::tr;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

const IMAGE_EXT: [&str; 12] = [
    "jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "tif", "avif", "bmp", "gif", "svg",
];
const AUDIO_EXT: [&str; 8] = ["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"];
const VIDEO_EXT: [&str; 6] = ["mp4", "mov", "mkv", "webm", "avi", "wmv"];
const PDF_EXT: [&str; 1] = ["pdf"];
const DOCUMENT_EXT: [&str; 9] = ["docx", "doc", "odt", "rtf", "md", "txt", "epub", "html", "htm"];
const SLIDES_EXT: [&str; 3] = ["pptx", "ppt", "odp"];
const SPREADSHEET_EXT: [&str; 5] = ["xlsx", "xls", "ods", "csv", "tsv"];
const SUBTITLE_EXT: [&str; 2] = ["srt", "vtt"];
// rar can only be read, not created
const ARCHIVE_EXT: [&str; 5] = ["zip", "tar", "tar.gz", "tgz", "rar"];

// heic is input only: ImageMagick reads it but does not write it
const IMAGE_TARGETS: [&str; 7] = ["jpg", "png", "webp", "tiff", "avif", "bmp", "pdf"];
const AUDIO_TARGETS: [&str; 8] = ["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"];
const VIDEO_TARGETS: [&str; 8] = ["mp4", "mov", "mkv", "webm", "avi", "wmv", "gif", "mp3"];
// pptx and xlsx from the earlier Python version are not ported (Python needs openpyxl and LibreOffice does not do it well)
const PDF_TARGETS: [&str; 6] = ["docx", "jpg", "png", "epub", "txt", "md"];
const DOCUMENT_TARGETS: [&str; 8] = ["docx", "md", "txt", "epub", "html", "pdf", "jpg", "png"];
const SLIDES_TARGETS: [&str; 3] = ["pdf", "jpg", "png"];
const SPREADSHEET_TARGETS: [&str; 5] = ["xlsx", "ods", "csv", "tsv", "pdf"];
// LibreOffice filter for TSV: tab separator (9), no quotes (34), UTF-8 (76), from row 1
const TSV_FILTER: &str = "Text - txt - csv (StarCalc):9,34,76,1";
const SUBTITLE_TARGETS: [&str; 3] = ["srt", "vtt", "txt"];
const ARCHIVE_TARGETS: [&str; 3] = ["zip", "tar", "tar.gz"];
const FOLDER_TARGETS: [&str; 3] = ["zip", "tar", "tar.gz"];

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum Kind {
    Image,
    Audio,
    Video,
    Pdf,
    Document,
    Slides,
    Spreadsheet,
    Subtitle,
    Archive,
    Folder,
}

// Function that is told the percentage (0 to 99) during an ffmpeg conversion.
// In the app it emits the event; in tests it stores the values, without needing a window.
pub type Progress = Arc<dyn Fn(f64) + Send + Sync>;

// It is set to true from the cancel command; run_in looks at it every 50 ms and kills the process
pub static CANCEL: AtomicBool = AtomicBool::new(false);

// Images that a .md links to and that do not exist, found while converting. The interface asks for them
// with `take_warnings` when the conversion ends, so the result can say that the document came out without them.
static MISSING_IMAGES: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub(crate) fn get_ext(path: &Path) -> String {
    // "tar.gz" has two dots: it is treated as a single extension
    if path.to_string_lossy().to_lowercase().ends_with(".tar.gz") {
        return "tar.gz".to_string();
    }
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn alias(ext: &str) -> &str {
    match ext {
        "jpeg" => "jpg",
        "tif" => "tiff",
        "heif" => "heic",
        "htm" => "html",
        "tgz" => "tar.gz",
        _ => ext,
    }
}

pub(crate) fn category(path: &Path) -> Option<Kind> {
    if path.is_dir() {
        return Some(Kind::Folder);
    }
    let ext = get_ext(path);
    if IMAGE_EXT.contains(&ext.as_str()) {
        Some(Kind::Image)
    } else if AUDIO_EXT.contains(&ext.as_str()) {
        Some(Kind::Audio)
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some(Kind::Video)
    } else if PDF_EXT.contains(&ext.as_str()) {
        Some(Kind::Pdf)
    } else if DOCUMENT_EXT.contains(&ext.as_str()) {
        Some(Kind::Document)
    } else if SLIDES_EXT.contains(&ext.as_str()) {
        Some(Kind::Slides)
    } else if SPREADSHEET_EXT.contains(&ext.as_str()) {
        Some(Kind::Spreadsheet)
    } else if SUBTITLE_EXT.contains(&ext.as_str()) {
        Some(Kind::Subtitle)
    } else if ARCHIVE_EXT.contains(&ext.as_str()) {
        Some(Kind::Archive)
    } else {
        None
    }
}

// List of targets of a type, as it is in the tables (it includes the format itself)
pub(crate) fn type_targets(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Image => &IMAGE_TARGETS,
        Kind::Audio => &AUDIO_TARGETS,
        Kind::Video => &VIDEO_TARGETS,
        Kind::Pdf => &PDF_TARGETS,
        Kind::Document => &DOCUMENT_TARGETS,
        Kind::Slides => &SLIDES_TARGETS,
        Kind::Spreadsheet => &SPREADSHEET_TARGETS,
        Kind::Subtitle => &SUBTITLE_TARGETS,
        Kind::Archive => &ARCHIVE_TARGETS,
        Kind::Folder => &FOLDER_TARGETS,
    }
}

// true if the file is already in that format (jpeg counts as jpg). A folder never is:
// "data.zip" is still a folder that can be compressed to zip.
fn is_already(path: &Path, format: &str) -> bool {
    category(path).is_some_and(|k| k != Kind::Folder) && alias(&get_ext(path)) == format
}

fn targets_of(path: &Path) -> Vec<String> {
    let Some(kind) = category(path) else {
        return Vec::new();
    };
    type_targets(kind)
        .iter()
        .filter(|t| !is_already(path, t))
        .map(|t| t.to_string())
        .collect()
}

// Targets the wheel shows for a file: those of targets_of, limited and ordered according to the
// user's settings (see config.rs). targets_of is still the full list that convert validates against.
fn shown_targets_with(path: &Path, config: &Config) -> Vec<String> {
    let all = targets_of(path);
    let Some(kind) = category(path) else {
        return all;
    };
    config.visible(kind).into_iter().filter(|t| all.iter().any(|a| a == t)).map(|t| t.to_string()).collect()
}

// Returns folder/stem.ext, or folder/stem 2.ext, stem 3.ext... if it already exists (as on the Mac)
pub(crate) fn unique_path(folder: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut candidate = folder.join(format!("{stem}.{ext}"));
    let mut i = 2;
    while candidate.exists() {
        candidate = folder.join(format!("{stem} {i}.{ext}"));
        i += 1;
    }
    candidate
}

// New folder for the images of a result: name_media, name_media_2... With no spaces, because
// the name goes inside Markdown and HTML links.
fn unique_media_folder(folder: &Path, stem: &str) -> PathBuf {
    let base = format!("{}_media", stem.replace(' ', "_"));
    let mut candidate = folder.join(&base);
    let mut i = 2;
    while candidate.exists() {
        candidate = folder.join(format!("{base}_{i}"));
        i += 1;
    }
    candidate
}

// Same as unique_path but for folders (no extension): name, name 2...
fn unique_folder(folder: &Path, name: &str) -> PathBuf {
    let mut candidate = folder.join(name);
    let mut i = 2;
    while candidate.exists() {
        candidate = folder.join(format!("{name} {i}"));
        i += 1;
    }
    candidate
}

// Looks for the program in the PATH without running it (so LibreOffice is not launched just to check)
pub(crate) fn tool_exists(name: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

pub(crate) fn need(names: &[&str], hint: &str) -> Result<String, String> {
    for name in names {
        if tool_exists(name) {
            return Ok(name.to_string());
        }
    }
    #[cfg(target_os = "macos")]
    let hint = &brew_hint(hint);
    Err(tr!("{name} not found. Install it with: {hint}", "No se encontró {name}. Instálalo con: {hint}", name = names[0], hint = hint))
}

// The hints are written for apt; on macOS they become the Homebrew equivalent
#[cfg(target_os = "macos")]
fn brew_hint(hint: &str) -> String {
    let Some(packages) = hint.strip_prefix("sudo apt install ") else {
        return hint.to_string();
    };
    let mapped: Vec<&str> = packages
        .split_whitespace()
        .map(|p| match p {
            // Homebrew's ffmpeg has no libvorbis, which OGG needs
            "ffmpeg" => "ffmpeg-full",
            "libreoffice" => "--cask libreoffice",
            "poppler-utils" => "poppler",
            "librsvg2-bin" => "librsvg",
            "7zip" | "p7zip-full" => "sevenzip",
            other => other,
        })
        .collect();
    format!("brew install {}", mapped.join(" "))
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    run_in(program, args, None)
}

// Reads a whole stream (stdout or stderr) in a thread, so that the process never blocks when the pipe fills up
fn read_in_thread<R: Read + Send + 'static>(stream: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            let _ = stream.read_to_end(&mut bytes);
        }
        bytes
    })
}

// Seconds indicated by a line of ffmpeg's `-progress`, or None if the line carries no time.
// out_time_us and out_time_ms are both in microseconds (out_time_ms is a misnomer
// in ffmpeg); out_time comes as HH:MM:SS.micros. "N/A" and negative values are ignored.
fn progress_seconds(line: &str) -> Option<f64> {
    let (key, value) = line.trim().split_once('=')?;
    let value = value.trim();
    let seconds = match key {
        "out_time_us" | "out_time_ms" => value.parse::<f64>().ok()? / 1_000_000.0,
        "out_time" => {
            let mut parts = value.split(':');
            let (h, m, s) = (parts.next()?, parts.next()?, parts.next()?);
            if parts.next().is_some() {
                return None;
            }
            h.parse::<f64>().ok()? * 3600.0 + m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?
        }
        _ => return None,
    };
    if seconds.is_finite() && seconds >= 0.0 {
        Some(seconds)
    } else {
        None
    }
}

// New percentage for a progress line. Returns None if the line is no use, if the
// duration is not known (0 or negative) or if the value does not go up compared to `last` (so it never goes backwards).
// It is capped at 99: 100 is marked by the end of convert.
fn next_percent(line: &str, total: f64, last: f64) -> Option<f64> {
    if !(total > 0.0) {
        return None;
    }
    let percent = (progress_seconds(line)? / total * 100.0).min(99.0);
    if percent > last { Some(percent) } else { None }
}

// Duration in seconds from the output of ffprobe ("12.345\n"). "N/A" or 0 -> None.
fn parse_duration(text: &str) -> Option<f64> {
    let seconds = text.trim().parse::<f64>().ok()?;
    if seconds.is_finite() && seconds > 0.0 { Some(seconds) } else { None }
}

// Asks ffprobe for the duration. If it is not installed or it fails, None (there will be no real progress).
pub(crate) fn probe_duration(src: &str) -> Option<f64> {
    if !tool_exists("ffprobe") {
        return None;
    }
    let output = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1", "--", src])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    parse_duration(&String::from_utf8_lossy(&output.stdout))
}

// Reads the stdout of ffmpeg line by line and reports the percentage at most every 100 ms.
// When the stream ends it reports the last percentage that was skipped, so a fast run still ends high.
fn read_progress_in_thread<R: Read + Send + 'static>(
    stream: Option<R>,
    total: f64,
    callback: Progress,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut last = -1.0;
        let mut last_time: Option<Instant> = None;
        let mut skipped: Option<f64> = None;
        if let Some(stream) = stream {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let Some(percent) = next_percent(&line, total, skipped.unwrap_or(last)) else {
                    continue;
                };
                let too_soon = last_time.is_some_and(|t| t.elapsed() < Duration::from_millis(100));
                if too_soon {
                    skipped = Some(percent);
                    continue;
                }
                last = percent;
                skipped = None;
                last_time = Some(Instant::now());
                callback(percent);
            }
        }
        if let Some(percent) = skipped {
            callback(percent);
        }
        Vec::new()
    })
}

// Same as run, but able to choose the working folder (zip uses it).
// If CANCEL is set it kills the process (and its children) and returns the error with code [cancelled].
pub(crate) fn run_in(program: &str, args: &[&str], dir: Option<&Path>) -> Result<(), String> {
    run_progress(program, args, dir, None)
}

// Same as run_in; if it receives (total duration in seconds, progress function), it reads the stdout
// of ffmpeg (-progress pipe:1) line by line and reports the percentage.
pub(crate) fn run_progress(
    program: &str,
    args: &[&str],
    dir: Option<&Path>,
    progress: Option<(f64, Progress)>,
) -> Result<(), String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Own process group: that way what the program launches (soffice) can be killed too
        .process_group(0);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let mut child = command
        .spawn()
        .map_err(|e| tr!("Could not run {program}: {e}", "No se pudo ejecutar {program}: {e}", program = program, e = e))?;
    let stdout_reader = match progress {
        Some((total, callback)) => read_progress_in_thread(child.stdout.take(), total, callback),
        None => read_in_thread(child.stdout.take()),
    };
    let stderr_reader = read_in_thread(child.stderr.take());

    let status = loop {
        if CANCEL.load(Ordering::SeqCst) {
            kill_group(child.id());
            let _ = child.kill();
            let _ = child.wait();
            // When all the processes die the pipes close and the threads finish
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(cancelled_error());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(tr!("Error waiting for {program}: {e}", "Error esperando a {program}: {e}", program = program, e = e)),
        }
    };
    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let mut text = String::from_utf8_lossy(&stderr).trim().to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&stdout).trim().to_string();
    }
    match text.lines().last() {
        Some(line) => Err(line.to_string()),
        None => Err(tr!("{program} failed", "{program} falló", program = program)),
    }
}

// Kills the whole process group of the child (its pgid is its pid because we use process_group(0))
pub(crate) fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stderr(Stdio::null())
        .status();
}

// Text path to pass it to a program (the paths are already absolute, they never start with "-")
pub(crate) fn st(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

// Temporary folder that deletes itself when it goes out of scope, also if there was an error
pub(crate) struct TempDir(PathBuf);

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl TempDir {
    pub(crate) fn new() -> Result<TempDir, String> {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("mandarina_tmp_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).map_err(|e| tr!("Could not create temporary folder: {e}", "No se pudo crear carpeta temporal: {e}", e = e))?;
        Ok(TempDir(dir))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// Creates a new, empty subfolder inside parent (out, out 2...)
fn new_subdir(parent: &Path) -> Result<PathBuf, String> {
    let dir = unique_folder(parent, "out");
    std::fs::create_dir(&dir).map_err(|e| tr!("Could not create temporary folder: {e}", "No se pudo crear carpeta temporal: {e}", e = e))?;
    Ok(dir)
}

// Moves a temporary file to its final destination without overwriting anything (rename or, if it changes disk, copy)
pub(crate) fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        return Err(tr("Output file already exists", "El archivo de salida ya existe"));
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to).map_err(|e| tr!("Could not save the result: {e}", "No se pudo guardar el resultado: {e}", e = e))?;
    Ok(())
}

fn convert_image(src: &str, out: &str) -> Result<(), String> {
    if get_ext(Path::new(src)) == "svg" {
        return convert_svg(src, out);
    }
    let tool = need(&["magick", "convert"], "sudo apt install imagemagick")?;
    run(&tool, &[src, "-auto-orient", out])
}

fn convert_svg(src: &str, out: &str) -> Result<(), String> {
    if !tool_exists("rsvg-convert") {
        return match need(&["magick", "convert"], "") {
            Ok(tool) => run(&tool, &[src, out]),
            Err(_) => Err(tr("Cannot read SVG files. Install: sudo apt install librsvg2-bin", "No se pueden leer SVG. Instala: sudo apt install librsvg2-bin")),
        };
    }
    let fmt = get_ext(Path::new(out));
    if fmt == "png" || fmt == "pdf" {
        return run("rsvg-convert", &["-f", &fmt, "-o", out, src]);
    }
    // First to an intermediate PNG and then ImageMagick does the rest
    let mid = std::env::temp_dir().join(format!("mandarina_{}.png", std::process::id()));
    let mid_str = mid.to_string_lossy().to_string();
    let result = run("rsvg-convert", &["-f", "png", "-o", &mid_str, src])
        .and_then(|_| convert_image(&mid_str, out));
    let _ = std::fs::remove_file(&mid);
    result
}

// ffmpeg arguments (without the program) for a conversion. The only options beyond the defaults are
// the ones that change the result or the speed of a target:
// - gif: fewer frames per second and a small size;
// - audio targets: -vn drops the picture (and the cover art);
// - webm: with its defaults VP9 is very slow (a 3-minute 2560x1600 video took 7.5 minutes);
//   "-deadline good -cpu-used 4 -row-mt 1" is about 1.8 times faster with the same size and almost the
//   same quality (53.2 dB against 52.6 dB of PSNR measured on 20 s of that video).
fn ffmpeg_convert_args(src: &str, out: &str, target: &str, real_progress: bool) -> Vec<String> {
    let mut args: Vec<&str> = vec!["-nostdin", "-n"];
    if real_progress {
        args.extend(["-progress", "pipe:1", "-nostats"]);
    }
    args.extend(["-i", src]);
    if target == "gif" {
        args.extend(["-vf", "fps=12,scale=480:-1:flags=lanczos"]);
    } else if target == "webm" {
        args.extend(["-c:v", "libvpx-vp9", "-deadline", "good", "-cpu-used", "4", "-row-mt", "1"]);
    } else if AUDIO_TARGETS.contains(&target) {
        args.push("-vn");
    }
    args.push(out);
    args.into_iter().map(String::from).collect()
}

fn convert_ffmpeg(src: &str, out: &str, target: &str, progress: Option<Progress>) -> Result<(), String> {
    need(&["ffmpeg"], "sudo apt install ffmpeg")?;
    // There is only real progress if someone is listening and the duration is known
    let progress = match progress {
        Some(callback) => probe_duration(src).map(|total| (total, callback)),
        None => None,
    };
    let args = ffmpeg_convert_args(src, out, target, progress.is_some());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run_progress("ffmpeg", &args, None, progress)
}

// Reference document of the docx files: A4 with 2.5 cm margins and the "Centered" style.
// It was made from pandoc's one (`pandoc -o x.docx --print-default-data-file reference.docx`).
const REFERENCE_DOCX: &[u8] = include_bytes!("../assets/reference-a4.docx");
const DOCX_FILTER: &str = include_str!("../assets/docx-filter.lua");

// <p align="center">…</p> -> <div align="center"><p>…</p></div>. Only the form with double quotes.
fn wrap_centered_paragraphs(html: &str) -> String {
    const OPEN: &str = "<p align=\"center\">";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after = &rest[start + OPEN.len()..];
        let Some(end) = after.find("</p>") else {
            // No closing tag: it is left as is
            out.push_str(&rest[start..]);
            return out;
        };
        out.push_str("<div align=\"center\"><p>");
        out.push_str(&after[..end]);
        out.push_str("</p></div>");
        rest = &after[end + "</p>".len()..];
    }
    out.push_str(rest);
    out
}

// pandoc shares the widths of a pipe table by the dashes of its separator line when a row is longer than
// --columns (72 by default), which leaves tiny columns. With a huge value it leaves the widths to the reader.
const COLUMNS: &str = "--columns=100000";

fn convert_pandoc(src: &Path, out: &Path, target: &str) -> Result<(), String> {
    need(&["pandoc"], "sudo apt install pandoc")?;
    let folder = out.parent().unwrap_or(Path::new("."));
    let stem = get_stem(out);
    let src_dir = src.parent().unwrap_or(Path::new("."));
    // The images of a .md are looked up next to the .md, even if the result goes to another folder
    let resource = format!("--resource-path={}", st(src_dir));

    // A .md with HTML inside (<table>, <img>, <br>, <b>: normal in a GitHub README) loses
    // that HTML when going straight to docx or txt: HTML tables come out as loose paragraphs and
    // <img> images disappear. Going through HTML first, pandoc understands them as tables and images.
    let tmp = TempDir::new()?;
    let mut src_text = st(src);
    let via_html = get_ext(src) == "md" && matches!(target, "docx" | "txt");
    if via_html {
        let mid = st(&tmp.path().join("mid.html"));
        run_in("pandoc", &[src_text.as_str(), "-f", "markdown", "-t", "html", COLUMNS, "-o", mid.as_str()], Some(folder))?;
        // pandoc loses the align of <p>; wrapped in a <div align="center"> it does keep it
        if let Ok(html) = std::fs::read_to_string(&mid) {
            std::fs::write(&mid, wrap_centered_paragraphs(&html))
                .map_err(|e| tr!("Could not write the file: {e}", "No se pudo escribir el archivo: {e}", e = e))?;
        }
        src_text = mid;
    }
    // The docx files go in A4 (pandoc uses Letter by default) and with the "Centered" style defined; the filter
    // applies it to the <div align="center">. Both files are inside the program (assets/).
    let reference = st(&tmp.path().join("reference.docx"));
    let filter = st(&tmp.path().join("docx-filter.lua"));
    let reference_arg = format!("--reference-doc={reference}");
    let filter_arg = format!("--lua-filter={filter}");
    if target == "docx" {
        std::fs::write(&reference, REFERENCE_DOCX).and_then(|_| std::fs::write(&filter, DOCX_FILTER))
            .map_err(|e| tr!("Could not write the file: {e}", "No se pudo escribir el archivo: {e}", e = e))?;
    }

    let out_text = st(out);
    let mut args = vec![src_text.as_str(), "-o", out_text.as_str(), resource.as_str()];
    if via_html {
        args.extend(["-f", "html"]);
    }
    // Without a wrap width, the dashes of a pipe table's separator line do not decide the column widths
    if get_ext(src) == "md" && matches!(target, "html" | "epub") {
        args.push(COLUMNS);
    }
    if target == "docx" {
        args.extend([reference_arg.as_str(), filter_arg.as_str()]);
    }
    if target == "txt" {
        args.extend(["-t", "plain"]);
    }
    // md and html link the images of the document: without extracting them, the links (media/rId1.jpg)
    // end up broken. They go to a new "<name>_media" folder next to the result (pandoc only creates it if
    // there are images) and, with cwd = that folder, the links come out relative and the result can be moved
    // together with it. The html also goes complete (-s): as a fragment it carries no <meta charset> and the
    // accents would look wrong when opened in a browser.
    let media_dir = unique_media_folder(folder, &stem);
    let media_arg = format!("--extract-media={}", media_dir.file_name().unwrap_or_default().to_string_lossy());
    let title = format!("pagetitle={stem}");
    if target == "html" {
        args.extend(["-s", "--metadata", title.as_str()]);
    }
    if matches!(target, "md" | "html") {
        args.push(media_arg.as_str());
    }
    let result = run_in("pandoc", &args, Some(folder));
    if result.is_err() {
        // The folder was new, so if it exists it is ours
        let _ = std::fs::remove_dir_all(&media_dir);
    }
    result
}

// Runs LibreOffice without a window and returns the file produced (inside tmp).
// It uses its own profile inside tmp so it does not clash with an open LibreOffice.
fn libreoffice(src: &Path, tmp: &Path, convert_to: &str, infilter: Option<&str>) -> Result<PathBuf, String> {
    let tool = need(&["soffice", "libreoffice"], "sudo apt install libreoffice")?;
    let outdir = new_subdir(tmp)?;
    let profile = format!("-env:UserInstallation=file://{}", st(&tmp.join("profile")));
    let infilter = infilter.map(|f| format!("--infilter={f}"));
    let (outdir_str, src_str) = (st(&outdir), st(src));
    let mut args = vec![profile.as_str(), "--headless"];
    if let Some(f) = &infilter {
        args.push(f);
    }
    args.extend(["--convert-to", convert_to, "--outdir", &outdir_str, &src_str]);
    run(&tool, &args)?;
    let first = std::fs::read_dir(&outdir)
        .map_err(|e| tr!("Could not read the LibreOffice output: {e}", "No se pudo leer la salida de LibreOffice: {e}", e = e))?
        .flatten()
        .next();
    match first {
        Some(entry) => Ok(entry.path()),
        None => Err(tr("LibreOffice produced no file", "LibreOffice no produjo ningún archivo")),
    }
}

// One image per page at 300 DPI inside out_folder (the folder is created here)
fn pdf_to_images(pdf: &Path, out_folder: &Path, target: &str) -> Result<(), String> {
    need(&["pdftoppm"], "sudo apt install poppler-utils")?;
    std::fs::create_dir(out_folder).map_err(|e| tr!("Could not create the folder: {e}", "No se pudo crear la carpeta: {e}", e = e))?;
    let flag = if target == "png" { "-png" } else { "-jpeg" };
    let prefix = st(&out_folder.join("page"));
    run("pdftoppm", &["-r", "300", flag, &st(pdf), &prefix])
}

// We already have a PDF: we leave it as a PDF or split it into images (one per page)
fn finish_pdf(pdf: &Path, out: &Path, target: &str) -> Result<(), String> {
    if target == "pdf" {
        move_file(pdf, out)
    } else {
        pdf_to_images(pdf, out, target)
    }
}

// The filled shapes of a PDF (colored banners, backgrounds of a table) come out of LibreOffice only as
// old VML shapes, with no stacking order. Word and LibreOffice paint them above the text boxes, which
// hides what goes on them: the white name on a colored banner of a CV disappeared. A very negative
// z-index sends them behind the text. Only the shapes with a fill are touched.
const BEHIND_TEXT: &str = "z-index:-251658240;";

fn send_shapes_behind(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() + 256);
    let mut rest = xml;
    while let Some(start) = rest.find("<v:shape ") {
        out.push_str(&rest[..start]);
        let tag_and_rest = &rest[start..];
        let Some(end) = tag_and_rest.find('>') else {
            // A tag that never closes: the rest is left as it is
            out.push_str(tag_and_rest);
            return out;
        };
        let tag = &tag_and_rest[..=end];
        match tag.find("style=\"") {
            Some(i) if tag.contains("fillcolor=") && !tag.contains("z-index") => {
                let at = i + "style=\"".len();
                out.push_str(&tag[..at]);
                out.push_str(BEHIND_TEXT);
                out.push_str(&tag[at..]);
            }
            _ => out.push_str(tag),
        }
        rest = &tag_and_rest[end + 1..];
    }
    out.push_str(rest);
    out
}

// Applies send_shapes_behind to the document.xml of a docx made from a PDF. It is best effort: without
// unzip and zip, or if anything fails, the docx is left as LibreOffice made it.
fn send_pdf_shapes_behind(docx: &Path, tmp: &Path) {
    if !tool_exists("unzip") || !tool_exists("zip") {
        return;
    }
    let Ok(output) = Command::new("unzip").args(["-p", &st(docx), "word/document.xml"]).stderr(Stdio::null()).output() else {
        return;
    };
    let Ok(xml) = String::from_utf8(output.stdout) else {
        return;
    };
    let fixed = send_shapes_behind(&xml);
    if fixed == xml {
        return;
    }
    let work = tmp.join("shapes");
    if std::fs::create_dir_all(work.join("word")).is_err() || std::fs::write(work.join("word/document.xml"), fixed).is_err() {
        return;
    }
    let _ = run_in("zip", &["-q", &st(docx), "word/document.xml"], Some(&work));
}

fn convert_pdf(src: &Path, out: &Path, target: &str) -> Result<(), String> {
    let tmp = TempDir::new()?;
    match target {
        "txt" => {
            need(&["pdftotext"], "sudo apt install poppler-utils")?;
            run("pdftotext", &["-layout", &st(src), &st(out)])
        }
        "jpg" | "png" => pdf_to_images(src, out, target),
        "docx" => {
            let docx = libreoffice(src, tmp.path(), "docx:MS Word 2007 XML", Some("writer_pdf_import"))?;
            send_pdf_shapes_behind(&docx, tmp.path());
            move_file(&docx, out)
        }
        // epub and md: first to DOCX with LibreOffice and then pandoc
        _ => {
            let docx = libreoffice(src, tmp.path(), "docx:MS Word 2007 XML", Some("writer_pdf_import"))?;
            convert_pandoc(&docx, out, target)
        }
    }
}

fn convert_document(src: &Path, out: &Path, target: &str) -> Result<(), String> {
    let ext = get_ext(src);
    let tmp = TempDir::new()?;
    if matches!(target, "pdf" | "jpg" | "png") {
        let pdf = if matches!(ext.as_str(), "md" | "txt" | "html" | "htm" | "epub") {
            // pandoc does not make PDF without LaTeX: it goes through DOCX and LibreOffice
            let mid = tmp.path().join("mid.docx");
            convert_pandoc(src, &mid, "docx")?;
            libreoffice(&mid, tmp.path(), "pdf", None)?
        } else {
            libreoffice(src, tmp.path(), "pdf", None)?
        };
        return finish_pdf(&pdf, out, target);
    }
    if ext == "doc" {
        // pandoc does not read old .doc files: they go to DOCX first
        let docx = libreoffice(src, tmp.path(), "docx", None)?;
        return convert_pandoc(&docx, out, target);
    }
    convert_pandoc(src, out, target)
}

fn convert_slides(src: &Path, out: &Path, target: &str) -> Result<(), String> {
    let tmp = TempDir::new()?;
    let pdf = libreoffice(src, tmp.path(), "pdf", None)?;
    finish_pdf(&pdf, out, target)
}

fn convert_spreadsheet(src: &Path, out: &Path, target: &str) -> Result<(), String> {
    let convert_to = if target == "tsv" {
        format!("csv:{TSV_FILTER}")
    } else {
        target.to_string()
    };
    // An input .tsv also has to be read with the tab filter
    let infilter = if get_ext(src) == "tsv" { Some(TSV_FILTER) } else { None };
    let tmp = TempDir::new()?;
    let produced = libreoffice(src, tmp.path(), &convert_to, infilter)?;
    move_file(&produced, out)
}

// Removes the <i>, <b>... tags from a line
fn strip_tags(line: &str) -> String {
    let mut text = String::new();
    let mut inside = false;
    for c in line.chars() {
        match c {
            '<' => inside = true,
            '>' if inside => inside = false,
            _ if !inside => text.push(c),
            _ => {}
        }
    }
    text
}

// Leaves only the spoken text: no numbers, times, WEBVTT header, notes or tags
fn subtitle_to_text(content: &str) -> String {
    let mut lines = Vec::new();
    for line in content.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        let is_number = !line.is_empty() && line.chars().all(|c| c.is_ascii_digit());
        if line.is_empty()
            || is_number
            || line.contains("-->")
            || line.starts_with("WEBVTT")
            || line.starts_with("NOTE")
        {
            continue;
        }
        lines.push(strip_tags(line));
    }
    lines.join("\n") + "\n"
}

fn convert_subtitle(src: &Path, out: &Path, target: &str, progress: Option<Progress>) -> Result<(), String> {
    if target != "txt" {
        return convert_ffmpeg(&st(src), &st(out), target, progress);
    }
    let bytes = std::fs::read(src).map_err(|e| tr!("Could not read the file: {e}", "No se pudo leer el archivo: {e}", e = e))?;
    let text = subtitle_to_text(&String::from_utf8_lossy(&bytes));
    // create_new: it fails if the file already exists instead of overwriting it
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(|e| tr!("Could not create the file: {e}", "No se pudo crear el archivo: {e}", e = e))?;
    std::io::Write::write_all(&mut file, text.as_bytes())
        .map_err(|e| tr!("Could not write the file: {e}", "No se pudo escribir el archivo: {e}", e = e))
}

// Extracts src into dest with the system tools
fn extract_archive(src: &Path, dest: &Path) -> Result<(), String> {
    let (src, dest) = (st(src), st(dest));
    match get_ext(Path::new(&src)).as_str() {
        "zip" => {
            if tool_exists("unzip") {
                run("unzip", &["-q", &src, "-d", &dest])
            } else {
                let sz = need(&["7z", "7zz", "7za"], "sudo apt install unzip")?;
                run(&sz, &["x", "-y", &format!("-o{dest}"), "--", &src])
            }
        }
        "rar" => {
            if let Some(sz) = ["7z", "7zz", "7za"].into_iter().find(|t| tool_exists(t)) {
                run(sz, &["x", "-y", &format!("-o{dest}"), "--", &src])
            } else if tool_exists("unrar") {
                run("unrar", &["x", "-y", &src, &format!("{dest}/")])
            } else if tool_exists("bsdtar") {
                run("bsdtar", &["-xf", &src, "-C", &dest])
            } else {
                Err(tr("Cannot open RAR files. Install: sudo apt install 7zip", "No se pueden abrir RAR. Instala: sudo apt install 7zip"))
            }
        }
        // tar, tar.gz and tgz: tar detects the compression by itself
        _ => {
            need(&["tar"], "sudo apt install tar")?;
            run("tar", &["-xf", &src, "-C", &dest])
        }
    }
}

// Extracts into a temporary folder and compresses again in the requested format
fn convert_archive(src: &Path, out: &Path, target: &str) -> Result<(), String> {
    let tmp = TempDir::new()?;
    let files = tmp.path().join("files");
    std::fs::create_dir(&files).map_err(|e| tr!("Could not create temporary folder: {e}", "No se pudo crear carpeta temporal: {e}", e = e))?;
    extract_archive(src, &files)?;
    let out = st(out);
    match target {
        // -y stores symbolic links as links, without following them outside the folder
        "zip" => {
            need(&["zip"], "sudo apt install zip")?;
            run_in("zip", &["-r", "-q", "-y", &out, "."], Some(&files))
        }
        _ => {
            need(&["tar"], "sudo apt install tar")?;
            let flags = if target == "tar.gz" { "-czf" } else { "-cf" };
            run("tar", &[flags, &out, "-C", &st(&files), "."])
        }
    }
}

// Compresses the original folder (with its name inside the archive, as the Mac does).
// It runs from the parent folder; the output goes next to the folder, never inside it.
fn convert_folder(src: &Path, out: &Path, target: &str) -> Result<(), String> {
    let parent = src.parent().ok_or_else(|| tr("Invalid path", "Ruta no válida"))?;
    let name = src.file_name().unwrap_or_default().to_string_lossy().to_string();
    let out = st(out);
    match target {
        // -y stores symbolic links as links, without following them outside the folder
        "zip" => {
            need(&["zip"], "sudo apt install zip")?;
            run_in("zip", &["-r", "-q", "-y", &out, "--", &name], Some(parent))
        }
        _ => {
            need(&["tar"], "sudo apt install tar")?;
            let flags = if target == "tar.gz" { "-czf" } else { "-cf" };
            run("tar", &[flags, &out, "-C", &st(parent), "--", &name])
        }
    }
}

// Packs one or several files or folders (of any type) into a single zip, tar or tar.gz next
// to the first one. They are linked in a temporary folder so that all of them end up at the root of the archive,
// wherever they come from; zip follows the links and tar does it with -h.
pub(crate) fn pack(paths: &[String], format: &str) -> Result<String, String> {
    let mut sources = Vec::new();
    for path in paths {
        let src = std::path::absolute(path).map_err(|e| tr!("Invalid path: {e}", "Ruta no válida: {e}", e = e))?;
        if !src.exists() {
            return Err(tr("File does not exist", "El archivo no existe"));
        }
        sources.push(src);
    }
    let first = sources.first().ok_or_else(|| tr("No files", "No hay archivos"))?;
    let folder = first.parent().ok_or_else(|| tr("Invalid path", "Ruta no válida"))?;
    let stem = if sources.len() > 1 {
        tr("Archive", "Archivo")
    } else if first.is_dir() {
        first.file_name().unwrap_or_default().to_string_lossy().to_string()
    } else {
        get_stem(first)
    };
    let out = unique_path(folder, &stem, format);

    let tmp = TempDir::new()?;
    let mut names = Vec::new();
    for src in &sources {
        let mut name = src.file_name().unwrap_or_default().to_string_lossy().to_string();
        let mut i = 2;
        while tmp.path().join(&name).exists() {
            name = format!("{i} {}", src.file_name().unwrap_or_default().to_string_lossy());
            i += 1;
        }
        std::os::unix::fs::symlink(src, tmp.path().join(&name))
            .map_err(|e| tr!("Could not prepare the files: {e}", "No se pudieron preparar los archivos: {e}", e = e))?;
        names.push(name);
    }
    let out_text = st(&out);
    let result = match format {
        "zip" => {
            need(&["zip"], "sudo apt install zip").and_then(|_| {
                let mut args = vec!["-r", "-q", out_text.as_str(), "--"];
                args.extend(names.iter().map(String::as_str));
                run_in("zip", &args, Some(tmp.path()))
            })
        }
        _ => need(&["tar"], "sudo apt install tar").and_then(|_| {
            let flags = if format == "tar.gz" { "-chzf" } else { "-chf" };
            let dir = st(tmp.path());
            let mut args = vec![flags, out_text.as_str(), "-C", dir.as_str(), "--"];
            args.extend(names.iter().map(String::as_str));
            run("tar", &args)
        }),
    };
    if let Err(e) = result {
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    Ok(out_text)
}

// Name without extension (it removes the whole ".tar.gz" if there is one)
pub(crate) fn get_stem(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    match name.to_lowercase().strip_suffix(".tar.gz") {
        Some(rest) => name[..rest.len()].to_string(),
        None => path.file_stem().unwrap_or_default().to_string_lossy().to_string(),
    }
}

// Converts and returns the path of the result.
// pdf/document/presentation -> jpg/png: with several pages the output is a FOLDER
// "name_pages" next to the original with one image per page; with a single page it is a file.
#[cfg(test)]
fn convert_blocking(path: &str, format: &str) -> Result<String, String> {
    convert_with(path, format, None)
}

// Same as convert_blocking, reporting the real progress in ffmpeg conversions
fn convert_with(path: &str, format: &str, progress: Option<Progress>) -> Result<String, String> {
    // Every new conversion starts with no pending cancellation
    CANCEL.store(false, Ordering::SeqCst);
    clear_missing_images();
    convert_one(path, format, progress)
}

// Converts a file without touching CANCEL: convert_many uses it so as not to lose a cancellation
// that arrives between one file and the next
fn convert_one(path: &str, format: &str, progress: Option<Progress>) -> Result<String, String> {
    let src = std::path::absolute(path).map_err(|e| tr!("Invalid path: {e}", "Ruta no válida: {e}", e = e))?;
    if !src.exists() {
        return Err(tr("File does not exist", "El archivo no existe"));
    }
    let kind = category(&src).ok_or_else(|| tr("Unsupported file type", "Tipo de archivo no soportado"))?;
    if !targets_of(&src).iter().any(|t| t == format) {
        let name = src.file_name().unwrap_or_default().to_string_lossy();
        return Err(tr!("Cannot convert {name} to {format}", "No se puede convertir {name} a {format}", name = name, format = format));
    }

    let folder = src.parent().ok_or_else(|| tr("Invalid path", "Ruta no válida"))?;
    if kind == Kind::Document && get_ext(&src) == "md" {
        note_missing_images(&src, folder);
    }
    // A folder uses its full name ("data.v2" is not trimmed)
    let stem = if kind == Kind::Folder {
        src.file_name().unwrap_or_default().to_string_lossy().to_string()
    } else {
        get_stem(&src)
    };
    let pages = matches!(kind, Kind::Pdf | Kind::Document | Kind::Slides) && matches!(format, "jpg" | "png");
    let out = if pages {
        unique_folder(folder, &format!("{stem}_pages"))
    } else {
        unique_path(folder, &stem, format)
    };

    let result = match kind {
        Kind::Image => convert_image(&st(&src), &st(&out)),
        Kind::Audio | Kind::Video => convert_ffmpeg(&st(&src), &st(&out), format, progress),
        Kind::Pdf => convert_pdf(&src, &out, format),
        Kind::Document => convert_document(&src, &out, format),
        Kind::Slides => convert_slides(&src, &out, format),
        Kind::Spreadsheet => convert_spreadsheet(&src, &out, format),
        Kind::Subtitle => convert_subtitle(&src, &out, format, progress),
        Kind::Archive => convert_archive(&src, &out, format),
        Kind::Folder => convert_folder(&src, &out, format),
    };
    if let Err(e) = result {
        // Do not leave half-written outputs, not even if it was cancelled (the path was new, so it is ours)
        let _ = std::fs::remove_dir_all(&out);
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    if pages {
        return single_page_to_file(&out, folder, &stem, format);
    }
    Ok(st(&out))
}

fn clear_missing_images() {
    MISSING_IMAGES.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

// Reads the .md and keeps the images it links to that are not next to it (pandoc leaves them out without failing)
fn note_missing_images(md: &Path, folder: &Path) {
    let Ok(text) = std::fs::read_to_string(md) else {
        return;
    };
    let mut list = MISSING_IMAGES.lock().unwrap_or_else(|e| e.into_inner());
    for image in missing_images(&text, folder) {
        if !list.contains(&image) {
            list.push(image);
        }
    }
}

// The links of the images of a markdown text: ![alt](path "title"), ![alt](<path>) and <img src="path">
fn image_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    // Markdown: after "![" comes the alt text, then "(" and the path up to the first space or the closing ")"
    let mut rest = text;
    while let Some(at) = rest.find("![") {
        rest = &rest[at + 2..];
        let Some(close) = rest.find("](") else {
            break;
        };
        let after = &rest[close + 2..];
        let target = match after.strip_prefix('<') {
            Some(inside) => inside.split('>').next().unwrap_or(""),
            None => after.split(|c: char| c.is_whitespace() || c == ')').next().unwrap_or(""),
        };
        links.push(target.to_string());
    }
    // HTML: the src of every <img>. The lowercase copy has the same offsets (only ASCII changes)
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("<img") {
        let start = from + at;
        let end = lower[start..].find('>').map_or(text.len(), |e| start + e);
        let tag = &text[start..end];
        if let Some(src) = tag.to_ascii_lowercase().find("src=") {
            let value = &tag[src + 4..];
            let link = match value.chars().next() {
                Some(q @ ('"' | '\'')) => value[1..].split(q).next().unwrap_or(""),
                _ => value.split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or(""),
            };
            links.push(link.to_string());
        }
        from = end;
    }
    links
}

// "my%20photo.png?raw=1#x" -> "my photo.png"
fn link_to_path(link: &str) -> String {
    let link = link.split(['?', '#']).next().unwrap_or("");
    let bytes = link.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

// The local images of the text that do not exist (relative ones are looked up in `folder`), without repeats
fn missing_images(text: &str, folder: &Path) -> Vec<String> {
    let mut missing: Vec<String> = Vec::new();
    for link in image_links(text) {
        let link = link.trim();
        let remote = ["http://", "https://", "data:", "//", "ftp://"].iter().any(|p| link.starts_with(p));
        if link.is_empty() || remote {
            continue;
        }
        let path = link_to_path(link);
        if !folder.join(&path).exists() && !missing.contains(&link.to_string()) {
            missing.push(link.to_string());
        }
    }
    missing
}

// The images left out by the last conversion (the interface asks for them when it ends)
#[tauri::command]
pub fn take_warnings() -> Vec<String> {
    std::mem::take(&mut *MISSING_IMAGES.lock().unwrap_or_else(|e| e.into_inner()))
}

// With a single page no folder is needed: the image is taken out and the folder is deleted.
// With several pages the folder is left as it is.
fn single_page_to_file(pages_folder: &Path, folder: &Path, stem: &str, format: &str) -> Result<String, String> {
    let files: Vec<PathBuf> = std::fs::read_dir(pages_folder)
        .map_err(|e| tr!("Could not read the folder: {e}", "No se pudo leer la carpeta: {e}", e = e))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    if files.len() != 1 {
        return Ok(st(pages_folder));
    }
    let final_path = unique_path(folder, stem, format);
    move_file(&files[0], &final_path)?;
    let _ = std::fs::remove_dir_all(pages_folder);
    Ok(st(&final_path))
}

// Formats for the wheel with several files. Those of an unsupported type are ignored. A format
// is offered if all the supported ones accept it according to the list of their type (the format itself included)
// and not all of them are already in that format. The order is that of the list of the first supported file.
fn targets_of_many(paths: &[String]) -> Vec<String> {
    targets_of_many_with(paths, &config::current())
}

fn targets_of_many_with(paths: &[String], config: &Config) -> Vec<String> {
    let files: Vec<(&Path, Kind)> = paths
        .iter()
        .filter_map(|p| category(Path::new(p)).map(|k| (Path::new(p), k)))
        .collect();
    let Some((_, first_kind)) = files.first() else {
        return Vec::new();
    };
    let mut list: Vec<String> = config
        .visible(*first_kind)
        .iter()
        .filter(|format| files.iter().all(|(_, kind)| config.visible(*kind).contains(format)))
        .filter(|format| !files.iter().all(|(path, _)| is_already(path, format)))
        .map(|format| format.to_string())
        .collect();
    // Several files that are not archives can always be packed into a zip. The wheel allows
    // 8 segments, so if needed the last format is dropped to make room for it.
    if files.len() > 1 && files.iter().all(|(_, kind)| *kind != Kind::Archive) && !list.iter().any(|f| f == "zip") {
        list.truncate(7);
        list.push("zip".to_string());
    }
    list
}

// Result of convert_many: paths created, names skipped and failures with their reason
#[derive(Serialize, Debug, Default, PartialEq)]
pub struct BatchResult {
    outputs: Vec<String>,
    skipped: Vec<String>,
    failed: Vec<Failure>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Failure {
    name: String,
    error: String,
}

// Global percentage (0 to 99) when `done` files out of `total` have finished and the current one is at
// `current` (0 to 100, or None if it does not report its progress)
fn global_percent(done: usize, total: usize, current: Option<f64>) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let current = current.unwrap_or(0.0).clamp(0.0, 100.0);
    ((done as f64 + current / 100.0) / total as f64 * 100.0).clamp(0.0, 99.0)
}

// Sends the global percentage without going backwards and at most every 100 ms
struct GlobalProgress {
    sink: Progress,
    state: std::sync::Mutex<(f64, Option<Instant>)>,
}

impl GlobalProgress {
    fn send(&self, percent: f64) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let too_soon = state.1.is_some_and(|t| t.elapsed() < Duration::from_millis(100));
        if percent <= state.0 || too_soon {
            return;
        }
        *state = (percent, Some(Instant::now()));
        (self.sink)(percent);
    }
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().unwrap_or_default().to_string_lossy().to_string()
}

// Core of convert_many. on_file(index, total, name) reports when each file to convert starts
// and percent receives the global percentage; in the app they emit events and in tests they store values.
fn convert_many_with(
    paths: &[String],
    format: &str,
    on_file: &dyn Fn(usize, usize, &str),
    percent: Progress,
) -> Result<BatchResult, String> {
    if paths.is_empty() {
        return Err(tr("No files", "No hay archivos"));
    }
    CANCEL.store(false, Ordering::SeqCst);
    clear_missing_images();
    // Those already in that format are skipped and the unsupported ones do not count in the total
    let skip = |p: &String| is_already(Path::new(p), format);
    let total = paths
        .iter()
        .filter(|p| category(Path::new(p.as_str())).is_some() && !skip(p))
        .count();
    // Several files (none an archive) to zip, tar or tar.gz: all together in a single archive
    let supported: Vec<&String> = paths.iter().filter(|p| category(Path::new(p.as_str())).is_some()).collect();
    let pack_all = paths.len() > 1
        && ARCHIVE_TARGETS.contains(&format)
        && !supported.is_empty()
        && supported.iter().all(|p| category(Path::new(p.as_str())) != Some(Kind::Archive));
    if pack_all {
        let mut result = BatchResult::default();
        for path in paths.iter().filter(|p| category(Path::new(p.as_str())).is_none()) {
            result.failed.push(Failure { name: file_name(path), error: tr("Unsupported file type", "Tipo de archivo no soportado") });
        }
        let names: Vec<String> = supported.into_iter().cloned().collect();
        match pack(&names, format) {
            Ok(out) => result.outputs.push(out),
            Err(e) if has_code(&e, CANCELLED) => return Err(e),
            Err(error) => result.failed.push(Failure { name: tr("Archive", "Archivo"), error }),
        }
        return Ok(result);
    }
    let global = Arc::new(GlobalProgress { sink: percent, state: std::sync::Mutex::new((-1.0, None)) });
    let mut result = BatchResult::default();
    let mut done = 0;
    for path in paths {
        if CANCEL.load(Ordering::SeqCst) {
            return Err(cancelled_error());
        }
        let name = file_name(path);
        if category(Path::new(path)).is_none() {
            result.failed.push(Failure { name, error: tr("Unsupported file type", "Tipo de archivo no soportado") });
            continue;
        }
        if skip(path) {
            result.skipped.push(name);
            continue;
        }
        on_file(done + 1, total, &name);
        let global_in = Arc::clone(&global);
        let (done_now, total_now) = (done, total);
        let progress: Progress =
            Arc::new(move |p| global_in.send(global_percent(done_now, total_now, Some(p))));
        match convert_one(path, format, Some(progress)) {
            Ok(out) => result.outputs.push(out),
            Err(e) if has_code(&e, CANCELLED) => return Err(e),
            Err(error) => result.failed.push(Failure { name, error }),
        }
        done += 1;
        global.send(global_percent(done, total, None));
    }
    Ok(result)
}

#[tauri::command]
pub fn get_targets(path: String) -> Vec<String> {
    shown_targets_with(Path::new(&path), &config::current())
}

#[tauri::command]
pub fn get_targets_many(paths: Vec<String>) -> Vec<String> {
    targets_of_many(&paths)
}

#[tauri::command]
pub async fn convert(app: AppHandle, path: String, format: String) -> Result<String, String> {
    // convert-progress event with { "percent": number }; 100 is marked by the interface when it ends
    let progress: Progress = Arc::new(move |percent| {
        let _ = app.emit("convert-progress", serde_json::json!({ "percent": percent }));
    });
    tauri::async_runtime::spawn_blocking(move || convert_with(&path, &format, Some(progress)))
        .await
        .map_err(|e| tr!("Internal error: {e}", "Error interno: {e}", e = e))?
}

// Events: convert-file { index, total, name } when each file starts and
// global convert-progress { percent } (100 is marked by the interface when it ends)
#[tauri::command]
pub async fn convert_many(app: AppHandle, paths: Vec<String>, format: String) -> Result<BatchResult, String> {
    let app_progress = app.clone();
    let percent: Progress = Arc::new(move |percent| {
        let _ = app_progress.emit("convert-progress", serde_json::json!({ "percent": percent }));
    });
    tauri::async_runtime::spawn_blocking(move || {
        let on_file = |index: usize, total: usize, name: &str| {
            let _ = app.emit("convert-file", serde_json::json!({ "index": index, "total": total, "name": name }));
        };
        convert_many_with(&paths, &format, &on_file, percent)
    })
    .await
    .map_err(|e| tr!("Internal error: {e}", "Error interno: {e}", e = e))?
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    // CANCEL is global: the tests that convert or cancel cannot run at the same time
    static LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn lock() -> std::sync::MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn encuentra_las_imagenes_que_faltan() {
        let dir = test_dir("faltan");
        std::fs::write(dir.join("esta.png"), "x").unwrap();
        std::fs::write(dir.join("con espacio.png"), "x").unwrap();
        let text = "![a](esta.png) ![b](nada.png \"titulo\") ![c](<otra falta.png>) ![d](con%20espacio.png)\n\
            ![e](https://x.org/a.png) ![f](data:image/png;base64,AAA) <img src=\"html.png\" width=3> <IMG SRC='esta.png'> ![g](nada.png)";
        let missing = missing_images(text, &dir);
        assert_eq!(missing, vec!["nada.png", "otra falta.png", "html.png"]);
        assert!(missing_images("sin imagenes [enlace](x.png)", &dir).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn convertir_un_md_apunta_las_imagenes_que_faltan() {
        let _guard = lock();
        if !tool_exists("pandoc") {
            return;
        }
        let dir = test_dir("md-faltan");
        std::fs::write(dir.join("doc.md"), "# Hola\n\n![x](no-existe.png)\n").unwrap();
        convert_blocking(&st(&dir.join("doc.md")), "docx").unwrap();
        assert_eq!(take_warnings(), vec!["no-existe.png"]);
        assert!(take_warnings().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detecta_el_tipo() {
        assert_eq!(category(Path::new("/a/foto.JPG")), Some(Kind::Image));
        assert_eq!(category(Path::new("a.svg")), Some(Kind::Image));
        assert_eq!(category(Path::new("a.flac")), Some(Kind::Audio));
        assert_eq!(category(Path::new("a.mkv")), Some(Kind::Video));
        assert_eq!(category(Path::new("a.pdf")), Some(Kind::Pdf));
        assert_eq!(category(Path::new("a.HTM")), Some(Kind::Document));
        assert_eq!(category(Path::new("a.xyz")), None);
        assert_eq!(category(Path::new("sinextension")), None);
    }

    #[test]
    fn targets_excluyen_el_propio_formato() {
        let t = targets_of(Path::new("a.png"));
        assert_eq!(t, ["jpg", "webp", "tiff", "avif", "bmp", "pdf"]);
        assert!(!targets_of(Path::new("a.mp3")).contains(&"mp3".to_string()));
        assert_eq!(targets_of(Path::new("a.mp4")).len(), 7);
        assert!(targets_of(Path::new("a.xyz")).is_empty());
    }

    #[test]
    fn alias_de_extensiones() {
        assert!(!targets_of(Path::new("a.jpeg")).contains(&"jpg".to_string()));
        assert!(!targets_of(Path::new("a.tif")).contains(&"tiff".to_string()));
        // heic/heif are read, but they are no longer a target
        assert!(!targets_of(Path::new("a.heif")).contains(&"heic".to_string()));
        assert!(!targets_of(Path::new("a.png")).contains(&"heic".to_string()));
        assert!(targets_of(Path::new("a.heic")).contains(&"jpg".to_string()));
        assert!(targets_of(Path::new("a.jpeg")).contains(&"png".to_string()));
    }

    #[test]
    fn nombre_unico() {
        let dir = std::env::temp_dir().join(format!("mandarina_u_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique_path(&dir, "a", "png"), dir.join("a.png"));
        std::fs::write(dir.join("a.png"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "a", "png"), dir.join("a 2.png"));
        std::fs::write(dir.join("a 2.png"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "a", "png"), dir.join("a 3.png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rechaza_formato_no_permitido() {
        let _guard = lock();
        let dir = std::env::temp_dir().join(format!("mandarina_r_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.png");
        std::fs::write(&f, b"x").unwrap();
        assert!(convert_blocking(f.to_str().unwrap(), "png").is_err());
        assert!(convert_blocking(f.to_str().unwrap(), "docx").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn convierte_audio_con_ffmpeg() {
        let _guard = lock();
        if !tool_exists("ffmpeg") {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mandarina_i_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("-tono.wav");
        let wav_str = wav.to_str().unwrap();
        run(
            "ffmpeg",
            &["-nostdin", "-f", "lavfi", "-i", "sine=duration=0.2", wav_str],
        )
        .unwrap();
        let out = convert_blocking(wav_str, "flac").unwrap();
        assert!(out.ends_with("-tono.flac"));
        let again = convert_blocking(wav_str, "flac").unwrap();
        assert!(again.ends_with("-tono 2.flac"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn convierte_imagen_con_magick() {
        let _guard = lock();
        if !tool_exists("magick") {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mandarina_m_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        let png_str = png.to_str().unwrap();
        run("magick", &["-size", "4x4", "xc:red", png_str]).unwrap();
        let out = convert_blocking(png_str, "bmp").unwrap();
        assert!(Path::new(&out).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // Temporary folder of each test
    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mandarina_t_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // Minimal one-page PDF with the text "Hola mundo"
    fn write_test_pdf(path: &Path) {
        let pdf = "%PDF-1.4\n\
1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 100]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj\n\
4 0 obj<</Length 44>>stream\nBT /F1 18 Tf 20 50 Td (Hola mundo) Tj ET\nendstream endobj\n\
5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\n\
trailer<</Root 1 0 R/Size 6>>\n%%EOF\n";
        std::fs::write(path, pdf).unwrap();
    }

    #[test]
    fn destinos_de_pdf_y_documento() {
        assert_eq!(targets_of(Path::new("a.pdf")), ["docx", "jpg", "png", "epub", "txt", "md"]);
        let t = targets_of(Path::new("a.docx"));
        assert_eq!(t, ["md", "txt", "epub", "html", "pdf", "jpg", "png"]);
        assert!(!targets_of(Path::new("a.htm")).contains(&"html".to_string()));
        assert!(targets_of(Path::new("a.doc")).contains(&"docx".to_string()));
        assert!(targets_of(Path::new("a.rtf")).len() <= 8);
    }

    #[test]
    fn nombre_de_carpeta_unico() {
        let dir = test_dir("carpeta");
        assert_eq!(unique_folder(&dir, "a_pages"), dir.join("a_pages"));
        std::fs::create_dir(dir.join("a_pages")).unwrap();
        assert_eq!(unique_folder(&dir, "a_pages"), dir.join("a_pages 2"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn convierte_markdown_con_pandoc() {
        let _guard = lock();
        if !tool_exists("pandoc") {
            return;
        }
        let dir = test_dir("pandoc");
        let md = dir.join("-nota.md");
        std::fs::write(&md, "# Titulo\n\nHola *mundo*\n").unwrap();
        let html = convert_blocking(md.to_str().unwrap(), "html").unwrap();
        assert!(std::fs::read_to_string(&html).unwrap().contains("<em>mundo</em>"));
        let txt = convert_blocking(md.to_str().unwrap(), "txt").unwrap();
        assert!(std::fs::read_to_string(&txt).unwrap().contains("Hola mundo"));
        let docx = convert_blocking(md.to_str().unwrap(), "docx").unwrap();
        assert!(std::fs::metadata(&docx).unwrap().len() > 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tabla_markdown_con_separador_largo_reparte_anchos_por_contenido() {
        let _guard = lock();
        if !tool_exists("pandoc") || !tool_exists("unzip") {
            return;
        }
        let dir = test_dir("tabla-md");
        let md = dir.join("tabla.md");
        let rule = "-".repeat(150);
        let text = format!(
            "| Ver | Fecha | Autor | Descripcion |\n|{r}|{r}|{r}|{r}|\n| 1.0 | 2026-01-01 | Ana | Primera version del documento con un texto bastante largo para que la fila pase de 72 columnas |\n",
            r = rule
        );
        std::fs::write(&md, text).unwrap();
        let html = convert_blocking(md.to_str().unwrap(), "html").unwrap();
        assert!(!std::fs::read_to_string(&html).unwrap().contains("<colgroup>"));
        let docx = convert_blocking(md.to_str().unwrap(), "docx").unwrap();
        let xml = Command::new("unzip").args(["-p", &docx, "word/document.xml"]).output().unwrap();
        let xml = String::from_utf8_lossy(&xml.stdout).to_string();
        let widths: Vec<u32> = xml
            .split("<w:gridCol w:w=\"")
            .skip(1)
            .map(|piece| piece.split('"').next().unwrap().parse().unwrap())
            .collect();
        assert_eq!(widths.len(), 4);
        let total: u32 = widths.iter().sum();
        assert!(widths[0] * 100 / total >= 5, "first column too narrow: {widths:?}");
        assert!(widths[3] > widths[0] && widths[3] > widths[1], "last column should be the widest: {widths:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn convierte_pdf_a_texto_e_imagenes() {
        let _guard = lock();
        if !tool_exists("pdftotext") || !tool_exists("pdftoppm") {
            return;
        }
        let dir = test_dir("pdf");
        let pdf = dir.join("-hoja.pdf");
        write_test_pdf(&pdf);
        let txt = convert_blocking(pdf.to_str().unwrap(), "txt").unwrap();
        assert!(std::fs::read_to_string(&txt).unwrap().contains("Hola mundo"));
        // A single page: a loose file and no leftover folder
        let png = convert_blocking(pdf.to_str().unwrap(), "png").unwrap();
        assert!(png.ends_with("-hoja.png"));
        assert!(Path::new(&png).is_file());
        assert!(!dir.join("-hoja_pages").exists());
        let again = convert_blocking(pdf.to_str().unwrap(), "png").unwrap();
        assert!(again.ends_with("-hoja 2.png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn varias_paginas_dan_una_carpeta() {
        let _guard = lock();
        if !tool_exists("pdftoppm") || !tool_exists("magick") {
            return;
        }
        let dir = test_dir("varias");
        let pdf = dir.join("-libro.pdf");
        run("magick", &["-size", "50x50", "xc:red", "xc:blue", &st(&pdf)]).unwrap();
        let folder = convert_blocking(pdf.to_str().unwrap(), "jpg").unwrap();
        assert!(folder.ends_with("-libro_pages"));
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pdf_roto_no_deja_salida() {
        let _guard = lock();
        if !tool_exists("pdftoppm") {
            return;
        }
        let dir = test_dir("roto");
        let pdf = dir.join("roto.pdf");
        std::fs::write(&pdf, b"esto no es un pdf").unwrap();
        assert!(convert_blocking(pdf.to_str().unwrap(), "png").is_err());
        assert!(!dir.join("roto_pages").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn destinos_de_presentaciones_y_hojas() {
        assert_eq!(targets_of(Path::new("a.pptx")), ["pdf", "jpg", "png"]);
        assert_eq!(targets_of(Path::new("a.odp")), ["pdf", "jpg", "png"]);
        assert_eq!(targets_of(Path::new("a.xlsx")), ["ods", "csv", "tsv", "pdf"]);
        assert_eq!(targets_of(Path::new("a.tsv")), ["xlsx", "ods", "csv", "pdf"]);
        assert!(!targets_of(Path::new("a.csv")).contains(&"csv".to_string()));
    }

    #[test]
    fn convierte_csv_a_xlsx_y_tsv_con_libreoffice() {
        let _guard = lock();
        if !tool_exists("soffice") && !tool_exists("libreoffice") {
            return;
        }
        let dir = test_dir("calc");
        let csv = dir.join("datos.csv");
        std::fs::write(&csv, "a,b\n1,2\n").unwrap();
        let xlsx = convert_blocking(csv.to_str().unwrap(), "xlsx").unwrap();
        assert!(std::fs::metadata(&xlsx).unwrap().len() > 0);
        let tsv = convert_blocking(csv.to_str().unwrap(), "tsv").unwrap();
        // LibreOffice quotes the texts with this filter (same as in the earlier Python version)
        let text = std::fs::read_to_string(&tsv).unwrap().replace('"', "");
        assert_eq!(text.trim(), "a\tb\n1\t2");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn destinos_de_subtitulos_y_comprimidos() {
        assert_eq!(targets_of(Path::new("a.srt")), ["vtt", "txt"]);
        assert_eq!(targets_of(Path::new("a.vtt")), ["srt", "txt"]);
        assert_eq!(targets_of(Path::new("a.zip")), ["tar", "tar.gz"]);
        assert_eq!(targets_of(Path::new("a.tar")), ["zip", "tar.gz"]);
        assert_eq!(targets_of(Path::new("a.TAR.GZ")), ["zip", "tar"]);
        assert_eq!(targets_of(Path::new("a.tgz")), ["zip", "tar"]);
        // rar is input only
        assert_eq!(targets_of(Path::new("a.rar")), ["zip", "tar", "tar.gz"]);
        assert!(!targets_of(Path::new("a.zip")).contains(&"rar".to_string()));
    }

    #[test]
    fn extension_y_nombre_de_tar_gz() {
        assert_eq!(get_ext(Path::new("/x/a.b.tar.gz")), "tar.gz");
        assert_eq!(get_stem(Path::new("/x/a.b.tar.gz")), "a.b");
        assert_eq!(get_stem(Path::new("/x/a.zip")), "a");
        assert_eq!(get_stem(Path::new("/x/a.tgz")), "a");
    }

    #[test]
    fn subtitulo_a_texto() {
        let srt = "\u{feff}1\n00:00:01,000 --> 00:00:02,000\n<i>Hola</i>\n\n2\n00:00:03,000 --> 00:00:04,000\nAdios <b>amigo</b>\n";
        assert_eq!(subtitle_to_text(srt), "Hola\nAdios amigo\n");
        let vtt = "WEBVTT\n\nNOTE nota\n\n00:01.000 --> 00:02.000\nLinea\n";
        assert_eq!(subtitle_to_text(vtt), "Linea\n");
    }

    #[test]
    fn convierte_subtitulos() {
        let _guard = lock();
        let dir = test_dir("sub");
        let srt = dir.join("-peli.srt");
        std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHola\n").unwrap();
        let txt = convert_blocking(srt.to_str().unwrap(), "txt").unwrap();
        assert_eq!(std::fs::read_to_string(&txt).unwrap(), "Hola\n");
        if tool_exists("ffmpeg") {
            let vtt = convert_blocking(srt.to_str().unwrap(), "vtt").unwrap();
            assert!(std::fs::read_to_string(&vtt).unwrap().starts_with("WEBVTT"));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn convierte_zip_a_tar_gz() {
        let _guard = lock();
        if !tool_exists("zip") || !tool_exists("tar") || !tool_exists("unzip") {
            return;
        }
        let dir = test_dir("zip");
        std::fs::write(dir.join("-hola.txt"), "contenido").unwrap();
        let zip = dir.join("-pack.zip");
        run_in("zip", &["-q", &st(&zip), "--", "-hola.txt"], Some(&dir)).unwrap();
        let tgz = convert_blocking(zip.to_str().unwrap(), "tar.gz").unwrap();
        assert!(tgz.ends_with("-pack.tar.gz"));
        // Back to zip from the tar.gz: it follows the name without ".tar"
        let back = convert_blocking(&tgz, "zip").unwrap();
        assert!(back.ends_with("-pack 2.zip"));
        let list = Command::new("tar").args(["-tzf", &tgz]).output().unwrap();
        assert!(String::from_utf8_lossy(&list.stdout).contains("-hola.txt"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn destinos_de_carpeta() {
        let dir = test_dir("destinos_carpeta");
        // A folder named "data.zip" can still go to zip
        let folder = dir.join("datos.zip");
        std::fs::create_dir(&folder).unwrap();
        assert_eq!(targets_of(&folder), ["zip", "tar", "tar.gz"]);
        assert_eq!(category(&folder), Some(Kind::Folder));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn comprime_una_carpeta() {
        let _guard = lock();
        if !tool_exists("zip") || !tool_exists("tar") {
            return;
        }
        let dir = test_dir("comprime_carpeta");
        let folder = dir.join("Mi Carpeta");
        std::fs::create_dir_all(folder.join("sub")).unwrap();
        std::fs::write(folder.join("sub").join("-a.txt"), "hola").unwrap();
        let zip = convert_blocking(folder.to_str().unwrap(), "zip").unwrap();
        assert_eq!(zip, st(&dir.join("Mi Carpeta.zip")));
        let again = convert_blocking(folder.to_str().unwrap(), "zip").unwrap();
        assert_eq!(again, st(&dir.join("Mi Carpeta 2.zip")));
        let tgz = convert_blocking(folder.to_str().unwrap(), "tar.gz").unwrap();
        let list = Command::new("tar").args(["-tzf", &tgz]).output().unwrap();
        assert!(String::from_utf8_lossy(&list.stdout).contains("Mi Carpeta/sub/-a.txt"));
        let tar = convert_blocking(folder.to_str().unwrap(), "tar").unwrap();
        assert!(tar.ends_with("Mi Carpeta.tar"));
        // The original folder is still intact
        assert!(folder.join("sub").join("-a.txt").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // Cancelling a LibreOffice conversion must kill soffice and everything it launched (on Linux and
    // macOS kill_group runs "kill -KILL -- -<pgid>"; BSD kill accepts "--" too) and leave no output
    #[test]
    fn cancelar_libreoffice_no_deja_procesos() {
        let _guard = lock();
        if !tool_exists("soffice") && !tool_exists("libreoffice") {
            return;
        }
        let dir = test_dir("cancela_libreoffice");
        let csv = dir.join("datos.csv");
        let rows: String = (0..20000).map(|i| format!("{i},fila {i},{}\n", i * 7)).collect();
        std::fs::write(&csv, rows).unwrap();
        CANCEL.store(false, Ordering::SeqCst);
        let canceller = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(700));
            CANCEL.store(true, Ordering::SeqCst);
        });
        let start = std::time::Instant::now();
        let result = convert_blocking(csv.to_str().unwrap(), "pdf");
        canceller.join().unwrap();
        CANCEL.store(false, Ordering::SeqCst);
        assert_eq!(result, Err("[cancelled] Cancelled".to_string()));
        assert!(start.elapsed() < Duration::from_secs(10));
        assert!(!dir.join("datos.pdf").exists(), "queda una salida a medias");
        if tool_exists("pgrep") {
            // soffice runs with a profile inside our temporary folder: mandarina_tmp_<pid>_<n>
            std::thread::sleep(Duration::from_millis(300));
            let pattern = format!("mandarina_tmp_{}_", std::process::id());
            let found = Command::new("pgrep").args(["-f", &pattern]).output().unwrap();
            assert!(found.stdout.is_empty(), "queda un proceso de LibreOffice");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn run_se_cancela_y_mata_al_hijo() {
        let _guard = lock();
        CANCEL.store(false, Ordering::SeqCst);
        let canceller = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(300));
            CANCEL.store(true, Ordering::SeqCst);
        });
        let start = std::time::Instant::now();
        // sh launches a grandchild "sleep": it must die too
        let result = run("sh", &["-c", "sleep 31.77; true"]);
        canceller.join().unwrap();
        CANCEL.store(false, Ordering::SeqCst);
        assert_eq!(result, Err("[cancelled] Cancelled".to_string()));
        assert!(start.elapsed() < Duration::from_secs(5));
        if tool_exists("pgrep") {
            let found = Command::new("pgrep").args(["-f", "^sleep 31[.]77$"]).output().unwrap();
            assert!(found.stdout.is_empty(), "queda un proceso hijo");
        }
    }

    #[test]
    fn cancelar_una_conversion_no_deja_salida() {
        let _guard = lock();
        if !tool_exists("ffmpeg") {
            return;
        }
        let dir = test_dir("cancela");
        let src = dir.join("largo.mkv");
        run(
            "ffmpeg",
            &["-nostdin", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=25", "-t", "20", "-c:v", "mpeg4", &st(&src)],
        )
        .unwrap();
        // convert_blocking sets CANCEL to false at the start, so it is cancelled a little later
        let canceller = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(700));
            CANCEL.store(true, Ordering::SeqCst);
        });
        let result = convert_blocking(src.to_str().unwrap(), "webm");
        canceller.join().unwrap();
        CANCEL.store(false, Ordering::SeqCst);
        assert_eq!(result, Err("[cancelled] Cancelled".to_string()));
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(left.len(), 1, "solo debe quedar el original");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn lee_el_tiempo_de_las_lineas_de_progreso() {
        // out_time_us and out_time_ms come in microseconds
        assert_eq!(progress_seconds("out_time_us=1500000"), Some(1.5));
        assert_eq!(progress_seconds("out_time_ms=2000000"), Some(2.0));
        assert_eq!(progress_seconds("out_time=00:01:02.500000"), Some(62.5));
        assert_eq!(progress_seconds("  out_time_us=500000  "), Some(0.5));
        // With no known time or with other keys
        assert_eq!(progress_seconds("out_time_us=N/A"), None);
        assert_eq!(progress_seconds("out_time=N/A"), None);
        assert_eq!(progress_seconds("out_time_us=-9223372036854775807"), None);
        assert_eq!(progress_seconds("progress=continue"), None);
        assert_eq!(progress_seconds("bitrate=118.2kbits/s"), None);
        assert_eq!(progress_seconds("basura"), None);
    }

    #[test]
    fn calcula_el_porcentaje() {
        assert_eq!(next_percent("out_time_us=1000000", 4.0, -1.0), Some(25.0));
        assert_eq!(next_percent("out_time=00:00:02.000000", 4.0, 25.0), Some(50.0));
        // Unknown or zero duration: nothing
        assert_eq!(next_percent("out_time_us=1000000", 0.0, -1.0), None);
        assert_eq!(next_percent("out_time_us=1000000", -3.0, -1.0), None);
        assert_eq!(next_percent("out_time_us=1000000", f64::NAN, -1.0), None);
        assert_eq!(next_percent("out_time_us=N/A", 4.0, -1.0), None);
        // It never goes backwards or repeats
        assert_eq!(next_percent("out_time_us=1000000", 4.0, 50.0), None);
        assert_eq!(next_percent("out_time_us=2000000", 4.0, 50.0), None);
        // Cap at 99 (and once at 99 no more is emitted)
        assert_eq!(next_percent("out_time_us=4000000", 4.0, 50.0), Some(99.0));
        assert_eq!(next_percent("out_time_us=9000000", 4.0, 50.0), Some(99.0));
        assert_eq!(next_percent("out_time_us=9000000", 4.0, 99.0), None);
    }

    #[test]
    fn lee_la_duracion_de_ffprobe() {
        assert_eq!(parse_duration("12.345\n"), Some(12.345));
        assert_eq!(parse_duration("N/A\n"), None);
        assert_eq!(parse_duration("0.000000\n"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("inf"), None);
    }

    #[test]
    fn el_progreso_real_llega_en_una_conversion() {
        let _guard = lock();
        if !tool_exists("ffmpeg") || !tool_exists("ffprobe") {
            return;
        }
        let dir = test_dir("progreso");
        let wav = dir.join("tres.wav");
        run("ffmpeg", &["-nostdin", "-f", "lavfi", "-i", "sine=duration=3", &st(&wav)]).unwrap();
        // Instead of the real AppHandle, each percentage is stored in a list
        let seen = Arc::new(std::sync::Mutex::new(Vec::<f64>::new()));
        let seen_in = Arc::clone(&seen);
        let progress: Progress = Arc::new(move |p| seen_in.lock().unwrap().push(p));
        let out = convert_with(wav.to_str().unwrap(), "flac", Some(progress)).unwrap();
        assert!(Path::new(&out).is_file());
        let seen = seen.lock().unwrap();
        assert!(!seen.is_empty(), "no se emitió ningún porcentaje");
        assert!(seen.windows(2).all(|w| w[0] < w[1]), "no es creciente: {seen:?}");
        assert!(seen.iter().all(|p| (0.0..=99.0).contains(p)), "fuera de rango: {seen:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sin_duracion_no_emite_nada() {
        let _guard = lock();
        if !tool_exists("ffmpeg") {
            return;
        }
        let dir = test_dir("sin_duracion");
        let srt = dir.join("a.srt");
        std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHola\n").unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let count_in = Arc::clone(&count);
        let progress: Progress = Arc::new(move |_| {
            count_in.fetch_add(1, Ordering::SeqCst);
        });
        // A subtitle has no useful duration: the conversion works the same, with no events
        let out = convert_with(srt.to_str().unwrap(), "vtt", Some(progress)).unwrap();
        assert!(Path::new(&out).is_file());
        assert_eq!(count.load(Ordering::SeqCst), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cancelar_con_progreso_no_deja_salida() {
        let _guard = lock();
        if !tool_exists("ffmpeg") || !tool_exists("ffprobe") {
            return;
        }
        let dir = test_dir("cancela_progreso");
        let src = dir.join("largo.mkv");
        run(
            "ffmpeg",
            &["-nostdin", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=25", "-t", "20", "-c:v", "mpeg4", &st(&src)],
        )
        .unwrap();
        let canceller = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(700));
            CANCEL.store(true, Ordering::SeqCst);
        });
        let progress: Progress = Arc::new(|_| {});
        let result = convert_with(src.to_str().unwrap(), "webm", Some(progress));
        canceller.join().unwrap();
        CANCEL.store(false, Ordering::SeqCst);
        assert_eq!(result, Err("[cancelled] Cancelled".to_string()));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "solo debe quedar el original");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(target_os = "macos")]
    const HINT_X: &str = "brew install x";
    #[cfg(not(target_os = "macos"))]
    const HINT_X: &str = "sudo apt install x";

    #[test]
    fn avisa_si_falta_una_herramienta() {
        assert_eq!(
            need(&["herramienta_que_no_existe"], "sudo apt install x"),
            Err(format!("herramienta_que_no_existe not found. Install it with: {}", HINT_X))
        );
        // With several options it is enough that one exists
        assert_eq!(need(&["herramienta_que_no_existe", "sh"], "x"), Ok("sh".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn pistas_de_homebrew() {
        assert_eq!(brew_hint("sudo apt install ffmpeg"), "brew install ffmpeg-full");
        assert_eq!(brew_hint("sudo apt install poppler-utils librsvg2-bin"), "brew install poppler librsvg");
        assert_eq!(brew_hint("sudo apt install libreoffice"), "brew install --cask libreoffice");
        assert_eq!(brew_hint("pip install x"), "pip install x");
    }

    #[test]
    fn convierte_svg_a_png_pdf_y_jpg() {
        let _guard = lock();
        if !tool_exists("rsvg-convert") || !tool_exists("magick") {
            return;
        }
        let dir = test_dir("svg");
        let svg = dir.join("-logo.svg");
        std::fs::write(
            &svg,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"120\" height=\"60\">\
<rect width=\"120\" height=\"60\" fill=\"orange\"/><text x=\"10\" y=\"35\">Hola</text></svg>",
        )
        .unwrap();
        let png = convert_blocking(svg.to_str().unwrap(), "png").unwrap();
        assert!(std::fs::read(&png).unwrap().starts_with(b"\x89PNG"));
        let pdf = convert_blocking(svg.to_str().unwrap(), "pdf").unwrap();
        assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
        let jpg = convert_blocking(svg.to_str().unwrap(), "jpg").unwrap();
        assert!(std::fs::read(&jpg).unwrap().starts_with(&[0xFF, 0xD8]));
        // The intermediate PNG of the step to jpg must not be left in the temp folder
        let left = dir.read_dir().unwrap().count();
        assert_eq!(left, 4, "svg + png + pdf + jpg");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn convierte_video_a_gif() {
        let _guard = lock();
        if !tool_exists("ffmpeg") {
            return;
        }
        let dir = test_dir("gif");
        let video = dir.join("-clip.mp4");
        run(
            "ffmpeg",
            &["-nostdin", "-f", "lavfi", "-i", "testsrc2=size=160x90:rate=10", "-t", "1", "-pix_fmt", "yuv420p", &st(&video)],
        )
        .unwrap();
        let gif = convert_blocking(video.to_str().unwrap(), "gif").unwrap();
        assert!(gif.ends_with("-clip.gif"));
        assert!(std::fs::read(&gif).unwrap().starts_with(b"GIF8"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // There is no way to create a .rar without the "rar" program (7z only reads it), so this test
    // is skipped if it is not installed
    #[test]
    fn convierte_rar_a_zip() {
        let _guard = lock();
        if !tool_exists("rar") || !tool_exists("zip") {
            return;
        }
        let dir = test_dir("rar");
        std::fs::write(dir.join("-hola.txt"), "contenido").unwrap();
        let rar = dir.join("-pack.rar");
        run_in("rar", &["a", "-inul", "--", &st(&rar), "-hola.txt"], Some(&dir)).unwrap();
        let zip = convert_blocking(rar.to_str().unwrap(), "zip").unwrap();
        assert!(zip.ends_with("-pack.zip"));
        assert!(std::fs::metadata(&zip).unwrap().len() > 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn varios_con_un_archivo_igual_que_uno() {
        for name in [
            "a.png", "a.jpeg", "a.mp3", "a.mp4", "a.pdf", "a.docx", "a.pptx", "a.csv", "a.srt",
            "a.zip", "a.tgz", "a.xyz",
        ] {
            assert_eq!(targets_of_many(&list(&[name])), targets_of(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn empaqueta_archivos_sueltos_y_varios_en_zip() {
        let _g = lock();
        if !tool_exists("zip") || !tool_exists("unzip") || !tool_exists("tar") {
            return;
        }
        let dir = test_dir("empaquetar");
        std::fs::write(dir.join("a.txt"), "uno").unwrap();
        std::fs::write(dir.join("b.png"), "dos").unwrap();
        let (a, b) = (st(&dir.join("a.txt")), st(&dir.join("b.png")));
        // A single file: name.zip with the file at the root
        let one = pack(&[a.clone()], "zip").unwrap();
        assert_eq!(one, st(&dir.join("a.zip")));
        let listing = capture_list(&one);
        assert!(listing.contains("a.txt"), "{listing}");
        // Several, of different types: a single "Archive.zip" (or "Archivo.zip") with all of them
        let both = convert_many_with(&[a.clone(), b.clone()], "zip", &|_, _, _| {}, Arc::new(|_| {})).unwrap();
        assert_eq!(both.outputs.len(), 1);
        assert!(both.failed.is_empty());
        let listing = capture_list(&both.outputs[0]);
        assert!(listing.contains("a.txt") && listing.contains("b.png"), "{listing}");
        // tar.gz keeps the content and does not overwrite
        let tgz = pack(&[a, b], "tar.gz").unwrap();
        assert!(std::fs::metadata(&tgz).unwrap().len() > 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn capture_list(zip: &str) -> String {
        let out = std::process::Command::new("unzip").args(["-Z1", zip]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn config_with(json: &str) -> Config {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn la_configuracion_limita_y_ordena_los_destinos() {
        let config = config_with(r#"{"formats": {"image": ["webp", "pdf", "png", "jpg"]}}"#);
        // A jpg sees the chosen ones without its own, in the chosen order
        assert_eq!(shown_targets_with(Path::new("a.jpg"), &config), ["webp", "pdf", "png"]);
        // A type with nothing configured does not change
        assert_eq!(shown_targets_with(Path::new("a.mp3"), &config), targets_of(Path::new("a.mp3")));
        // The validation of convert still uses the full list
        assert!(targets_of(Path::new("a.jpg")).contains(&"bmp".to_string()));
        // Several: what was chosen is used, and ZIP always
        let list = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<String>>();
        assert_eq!(targets_of_many_with(&list(&["a.png", "b.png"]), &config), ["webp", "pdf", "jpg", "zip"]);
        // Image + document: what was chosen for the image that the document also shows, in the chosen order
        assert_eq!(targets_of_many_with(&list(&["a.png", "b.docx"]), &config), ["pdf", "png", "jpg", "zip"]);
    }

    #[test]
    fn docx_a_html_y_md_extrae_las_imagenes_y_el_html_es_completo() {
        let _g = lock();
        if !tool_exists("pandoc") || !tool_exists("magick") {
            return;
        }
        let dir = test_dir("docx_media");
        run("magick", &["-size", "40x30", "xc:orange", &st(&dir.join("foto.png"))]).unwrap();
        std::fs::write(dir.join("doc.md"), "# Título ñandú\n\nTexto con acción.\n\n![Una foto](foto.png)\n").unwrap();
        run_in("pandoc", &["doc.md", "-o", "con.docx"], Some(&dir)).unwrap();
        // Without images: no extra folder must be left
        std::fs::write(dir.join("solo.md"), "# Hola\n\ntexto\n").unwrap();
        run_in("pandoc", &["solo.md", "-o", "solo.docx"], Some(&dir)).unwrap();

        let html = convert_blocking(&st(&dir.join("con.docx")), "html").unwrap();
        let text = std::fs::read_to_string(&html).unwrap();
        assert!(text.contains("<meta charset=\"utf-8\""), "el html debe ser completo");
        assert!(text.contains("<title>con</title>"), "{text}");
        assert!(text.contains("ñandú"));
        // The link is relative to the folder of the result and the image exists
        let link = text.split("<img src=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
        assert!(link.starts_with("con_media/"), "{link}");
        assert!(dir.join(&link).is_file(), "falta la imagen {link}");

        // The md goes to another folder (con_media already exists): con_media_2, no spaces in the link
        let md = convert_blocking(&st(&dir.join("con.docx")), "md").unwrap();
        let md_text = std::fs::read_to_string(&md).unwrap();
        let md_link = md_text.split("](").nth(1).unwrap().split(')').next().unwrap().to_string();
        assert!(md_link.starts_with("con_media_2/"), "{md_link}");
        assert!(!md_link.contains(' '), "{md_link}");
        assert!(dir.join(&md_link).is_file());

        let plain = convert_blocking(&st(&dir.join("solo.docx")), "html").unwrap();
        assert!(!dir.join("solo_media").exists(), "sin imágenes no debe quedar carpeta");
        assert!(std::fs::read_to_string(plain).unwrap().contains("<meta charset"));
        // txt extracts nothing
        convert_blocking(&st(&dir.join("con.docx")), "txt").unwrap();
        assert!(!dir.join("con_media_3").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn envuelve_los_parrafos_centrados() {
        assert_eq!(
            wrap_centered_paragraphs("a<p align=\"center\"><img src=\"x.png\"></p>b<p>normal</p><p align=\"center\">dos</p>"),
            "a<div align=\"center\"><p><img src=\"x.png\"></p></div>b<p>normal</p><div align=\"center\"><p>dos</p></div>"
        );
        // With no closing tag it is left the same; with nothing to change, too
        assert_eq!(wrap_centered_paragraphs("x<p align=\"center\">sin cierre"), "x<p align=\"center\">sin cierre");
        assert_eq!(wrap_centered_paragraphs("<p>nada</p>"), "<p>nada</p>");
    }

    #[test]
    fn md_con_html_a_docx_conserva_tablas_imagenes_centrado_y_a4() {
        let _g = lock();
        if !tool_exists("pandoc") || !tool_exists("magick") || !tool_exists("unzip") {
            return;
        }
        let dir = test_dir("md_html");
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        run("magick", &["-size", "40x30", "xc:orange", &st(&dir.join("assets/logo.png"))]).unwrap();
        run("magick", &["-size", "30x30", "xc:blue", &st(&dir.join("assets/otra.png"))]).unwrap();
        let md = "<p align=\"center\">\n  <img src=\"assets/logo.png\" alt=\"Logo\" width=\"110\">\n</p>\n\n\
                  <div align=\"center\">\n\n<p><strong>Portada</strong></p>\n\n</div>\n\n<div style=\"page-break-before: always;\"></div>\n\n\
                  # Título\n\n| a | b |\n|---|---|\n| uno | dos |\n\n\
                  <table>\n<tr><th>Campo</th><th>Valor</th></tr>\n<tr><td>nombre</td><td>Ana<br>López</td></tr>\n</table>\n\n\
                  Texto con <b>negrita</b> y ![otra](assets/otra.png).\n";
        std::fs::write(dir.join("informe.md"), md).unwrap();
        let docx = convert_blocking(&st(&dir.join("informe.md")), "docx").unwrap();
        let output = std::process::Command::new("unzip").args(["-p", docx.as_str(), "word/document.xml"]).output().unwrap();
        let xml = String::from_utf8_lossy(&output.stdout).to_string();
        // The Markdown table and the HTML one are two real tables (the HTML one used to come out as paragraphs)
        assert_eq!(xml.matches("<w:tbl>").count(), 2, "{xml}");
        // The logo (<img> HTML) and the normal image
        assert_eq!(xml.matches("<w:drawing>").count(), 2);
        assert!(xml.contains("Ana") && xml.contains("López") && xml.contains("negrita"));
        // A4 page (11906 x 16838 twips) and the logo, which was in a <p align="center">, centered
        assert!(xml.contains("11906") && xml.contains("16838"), "debe ser A4");
        // Centering: the logo (<p align="center">) and the text of the <div align="center">
        assert_eq!(xml.matches("w:val=\"Centered\"").count(), 2, "{xml}");
        // The empty <div style="page-break-before: always"> becomes a page break
        assert_eq!(xml.matches("w:type=\"page\"").count(), 1);
        // Also to txt: the HTML table is not lost
        let txt = convert_blocking(&st(&dir.join("informe.md")), "txt").unwrap();
        let text = std::fs::read_to_string(txt).unwrap();
        assert!(text.contains("Campo") && text.contains("nombre") && text.contains("López"), "{text}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // Manual test with a real file, to see how a specific conversion turns out without opening the app:
    //   MANDARINA_REAL=/ruta/archivo.md MANDARINA_FORMAT=docx cargo test archivo_real -- --ignored --nocapture
    // The result is created next to the file, as in the app.
    #[test]
    #[ignore]
    fn archivo_real() {
        let _g = lock();
        let (Ok(path), Ok(format)) = (std::env::var("MANDARINA_REAL"), std::env::var("MANDARINA_FORMAT")) else {
            println!("Define MANDARINA_REAL y MANDARINA_FORMAT");
            return;
        };
        let start = Instant::now();
        let result = convert_blocking(&path, &format);
        println!("RESULTADO {format}: {:?} en {:.1}s", result, start.elapsed().as_secs_f32());
        assert!(result.is_ok());
    }

    #[test]
    fn ffmpeg_args_por_destino() {
        let args = |target: &str, progress: bool| ffmpeg_convert_args("in.mp4", "out.x", target, progress);
        // webm: the fast VP9 options go before the output file
        let webm = args("webm", false);
        let text = webm.join(" ");
        assert!(text.contains("-c:v libvpx-vp9 -deadline good -cpu-used 4 -row-mt 1"), "{text}");
        assert_eq!(webm.last().unwrap(), "out.x");
        assert!(webm.iter().position(|a| a == "-i").unwrap() < webm.iter().position(|a| a == "-c:v").unwrap());
        // the other video targets keep the plain conversion
        for target in ["mp4", "mov", "mkv", "avi", "wmv"] {
            assert_eq!(args(target, false), ["-nostdin", "-n", "-i", "in.mp4", "out.x"], "{target}");
        }
        // gif and audio keep their own options
        assert!(args("gif", false).join(" ").contains("fps=12,scale=480"));
        assert!(args("mp3", false).contains(&"-vn".to_string()));
        assert!(!args("webm", false).contains(&"-vn".to_string()));
        // real progress comes before the input
        let with = args("webm", true);
        assert!(with.join(" ").starts_with("-nostdin -n -progress pipe:1 -nostats -i in.mp4"));
    }

    #[test]
    fn envia_al_fondo_las_formas_vml_con_relleno() {
        let xml = "<w:p><w:pict><v:shape id=\"a\" fillcolor=\"#5c2d91\" stroked=\"f\" style=\"position:absolute;margin-left:54pt\" path=\"m0,0\"><v:fill/></v:shape></w:pict>\
                   <w:pict><v:shape id=\"b\" style=\"position:absolute;margin-left:1pt\"><v:textbox/></v:shape></w:pict>\
                   <w:pict><v:shape id=\"c\" fillcolor=\"#fff\" style=\"z-index:5;position:absolute\"/></w:pict>texto</w:p>";
        let fixed = send_shapes_behind(xml);
        // The filled shape without order goes behind; the one with no fill and the one that already has a z-index do not change
        assert!(fixed.contains("fillcolor=\"#5c2d91\" stroked=\"f\" style=\"z-index:-251658240;position:absolute;margin-left:54pt\""), "{fixed}");
        assert!(fixed.contains("<v:shape id=\"b\" style=\"position:absolute;margin-left:1pt\">"));
        assert!(fixed.contains("style=\"z-index:5;position:absolute\""));
        assert_eq!(fixed.matches("z-index:-251658240").count(), 1);
        assert!(fixed.ends_with("texto</w:p>"));
        // Nothing to change: the same text; and an unterminated tag does not lose or repeat text
        assert_eq!(send_shapes_behind("<w:p>sin formas</w:p>"), "<w:p>sin formas</w:p>");
        assert_eq!(send_shapes_behind("<w:p><v:shape fillcolor=\"x\""), "<w:p><v:shape fillcolor=\"x\"");
    }

    #[test]
    fn pdf_a_docx_deja_visible_el_texto_sobre_una_banda_de_color() {
        let _g = lock();
        if !tool_exists("gs") || !tool_exists("magick") || !tool_exists("unzip") || !tool_exists("zip") || !(tool_exists("soffice") || tool_exists("libreoffice")) {
            return;
        }
        let dir = test_dir("pdf_banda");
        // A PDF with a purple banner and white text on it, made with ghostscript
        let ps = "0.36 0.18 0.57 setrgbcolor 40 640 520 90 rectfill 1 setgray /Helvetica-Bold findfont 34 scalefont setfont 60 675 moveto (WHITE ON BANNER) show 0 setgray /Helvetica findfont 14 scalefont setfont 60 560 moveto (Normal text below the banner) show showpage";
        std::fs::write(dir.join("banda.ps"), ps).unwrap();
        run("gs", &["-q", "-dNOPAUSE", "-dBATCH", "-sDEVICE=pdfwrite", &format!("-sOutputFile={}", st(&dir.join("banda.pdf"))), &st(&dir.join("banda.ps"))]).unwrap();
        let docx = convert_blocking(&st(&dir.join("banda.pdf")), "docx").unwrap();
        let xml = String::from_utf8_lossy(&std::process::Command::new("unzip").args(["-p", docx.as_str(), "word/document.xml"]).output().unwrap().stdout).to_string();
        assert!(xml.contains("z-index:-251658240"), "the banner must go behind the text");
        // Rendered, the banner is not an empty color block: there are white pixels (the text) inside it
        run("soffice", &["-env:UserInstallation=file:///tmp/mandarina_banda_profile", "--headless", "--convert-to", "pdf", "--outdir", &st(&dir.join("render")), &docx]).ok();
        let rendered = dir.join("render/banda.pdf");
        if rendered.exists() {
            run("gs", &["-q", "-dNOPAUSE", "-dBATCH", "-sDEVICE=png16m", "-r50", "-dFirstPage=1", "-dLastPage=1", &format!("-sOutputFile={}", st(&dir.join("pagina.png"))), &st(&rendered)]).unwrap();
            // The banner is at the top of the page: look at the band between 6% and 14% of the height
            let output = std::process::Command::new("magick")
                .args([&st(&dir.join("pagina.png")), "-gravity", "North", "-crop", "100%x9%+0+55", "+repage", "-colorspace", "gray", "-threshold", "85%", "-format", "%[fx:mean]", "info:"])
                .output()
                .unwrap();
            let white: f64 = String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(1.0);
            assert!(white < 0.97, "the band should have dark pixels (the banner), mean {white}");
            let output = std::process::Command::new("magick")
                .args([&st(&dir.join("pagina.png")), "-gravity", "North", "-crop", "70%x6%+0+60", "+repage", "-colorspace", "gray", "-threshold", "85%", "-format", "%[fx:mean]", "info:"])
                .output()
                .unwrap();
            let inner: f64 = String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0.0);
            assert!(inner > 0.0, "white text pixels must be visible inside the banner");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn varios_del_mismo_tipo() {
        // Three png: everything but png is offered (they all already are)
        assert_eq!(
            targets_of_many(&list(&["a.png", "b.png", "c.png"])),
            ["jpg", "webp", "tiff", "avif", "bmp", "pdf", "zip"]
        );
        // png and jpg mixed: jpg and png too, because not all of them are already in that format
        assert_eq!(
            targets_of_many(&list(&["a.png", "b.jpeg"])),
            ["jpg", "png", "webp", "tiff", "avif", "bmp", "pdf", "zip"]
        );
    }

    #[test]
    fn varios_de_tipos_mezclados() {
        // Image + document + presentation: only what they have in common, in the order of the image
        assert_eq!(targets_of_many(&list(&["a.png", "b.docx", "c.pptx"])), ["jpg", "png", "pdf", "zip"]);
        // With a presentation first the order changes
        assert_eq!(targets_of_many(&list(&["c.pptx", "a.png"])), ["pdf", "jpg", "png", "zip"]);
        // Image + document to pdf: both support pdf and neither is pdf
        assert!(targets_of_many(&list(&["a.png", "b.docx"])).contains(&"pdf".to_string()));
        // Audio + video: only mp3 is common
        assert_eq!(targets_of_many(&list(&["a.wav", "b.mp4"])), ["mp3", "zip"]);
        // Nothing in common except packing into zip
        assert_eq!(targets_of_many(&list(&["a.png", "b.mp3"])), ["zip"]);
        // A pdf and a document: the pdf has no "pdf" in its list, so it is not common
        assert!(!targets_of_many(&list(&["a.pdf", "b.docx"])).contains(&"pdf".to_string()));
    }

    #[test]
    fn varios_con_archivos_no_soportados() {
        assert!(targets_of_many(&[]).is_empty());
        assert!(targets_of_many(&list(&["a.xyz", "b"])).is_empty());
        // One supported and another not: the unsupported one is ignored
        assert_eq!(targets_of_many(&list(&["a.xyz", "b.png"])), targets_of(Path::new("b.png")));
        assert_eq!(targets_of_many(&list(&["b.png", "a.xyz"])), targets_of(Path::new("b.png")));
    }

    #[test]
    fn porcentaje_global() {
        // A single file: it matches its own progress
        assert_eq!(global_percent(0, 1, Some(40.0)), 40.0);
        assert_eq!(global_percent(0, 1, None), 0.0);
        // Cap at 99, also when the last one finishes
        assert_eq!(global_percent(1, 1, None), 99.0);
        assert_eq!(global_percent(0, 1, Some(100.0)), 99.0);
        // Several files with no progress of their own: it jumps when each one finishes
        assert_eq!(global_percent(0, 4, None), 0.0);
        assert_eq!(global_percent(1, 4, None), 25.0);
        assert_eq!(global_percent(3, 4, None), 75.0);
        // Several with progress of their own: (1 + 0.5) / 3
        assert_eq!(global_percent(1, 3, Some(50.0)), 50.0);
        // With no files or with odd values it does not go out of range
        assert_eq!(global_percent(0, 0, None), 0.0);
        assert_eq!(global_percent(0, 2, Some(500.0)), 50.0);
        // Monotonic: each step (progress of the current one, then finishing) never goes down
        let mut last = 0.0;
        for done in 0..5 {
            for current in [None, Some(0.0), Some(30.0), Some(99.0), Some(100.0)] {
                let p = global_percent(done, 5, current);
                assert!(p >= last && p <= 99.0, "{done} {current:?} {p}");
                last = p;
            }
        }
    }

    #[test]
    fn el_progreso_global_no_retrocede_ni_se_repite() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::<f64>::new()));
        let seen_in = Arc::clone(&seen);
        let global = GlobalProgress {
            sink: Arc::new(move |p| seen_in.lock().unwrap().push(p)),
            state: std::sync::Mutex::new((-1.0, None)),
        };
        global.send(10.0);
        // Too soon (less than 100 ms): it is discarded
        global.send(20.0);
        std::thread::sleep(Duration::from_millis(120));
        // It goes backwards or repeats: it is discarded
        global.send(5.0);
        global.send(10.0);
        global.send(30.0);
        assert_eq!(*seen.lock().unwrap(), [10.0, 30.0]);
    }

    // Folder with n wav audios of `secs` seconds, named -tono 1.wav, -tono 2.wav...
    fn make_wavs(dir: &Path, n: usize, secs: &str) -> Vec<String> {
        (1..=n)
            .map(|i| {
                let wav = dir.join(format!("-tono {i}.wav"));
                let duration = format!("sine=duration={secs}");
                run("ffmpeg", &["-nostdin", "-f", "lavfi", "-i", &duration, &st(&wav)]).unwrap();
                st(&wav)
            })
            .collect()
    }

    // Event log for the convert_many tests
    type Log = Arc<std::sync::Mutex<Vec<(usize, usize, String)>>>;
    type Percents = Arc<std::sync::Mutex<Vec<f64>>>;

    fn convert_many_logged(paths: &[String], format: &str) -> (Result<BatchResult, String>, Log, Percents) {
        let files: Log = Arc::new(std::sync::Mutex::new(Vec::new()));
        let percents: Percents = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (files_in, percents_in) = (Arc::clone(&files), Arc::clone(&percents));
        let on_file = move |i: usize, t: usize, n: &str| files_in.lock().unwrap().push((i, t, n.to_string()));
        let percent: Progress = Arc::new(move |p| percents_in.lock().unwrap().push(p));
        let result = convert_many_with(paths, format, &on_file, percent);
        (result, files, percents)
    }

    #[test]
    fn convierte_varios_saltando_y_fallando() {
        let _guard = lock();
        if !tool_exists("ffmpeg") {
            return;
        }
        let dir = test_dir("varios");
        let mut paths = make_wavs(&dir, 3, "0.3");
        // One that is already flac, one corrupt and one of an unsupported type
        let flac = dir.join("-ya.flac");
        run("ffmpeg", &["-nostdin", "-f", "lavfi", "-i", "sine=duration=0.2", &st(&flac)]).unwrap();
        let broken = dir.join("-roto.wav");
        std::fs::write(&broken, b"esto no es audio").unwrap();
        let unknown = dir.join("-raro.xyz");
        std::fs::write(&unknown, b"x").unwrap();
        paths.insert(1, st(&flac));
        paths.insert(2, st(&broken));
        paths.push(st(&unknown));

        let (result, files, percents) = convert_many_logged(&paths, "flac");
        let result = result.unwrap();
        assert_eq!(result.outputs.len(), 3);
        for i in 1..=3 {
            assert!(dir.join(format!("-tono {i}.flac")).is_file());
        }
        assert_eq!(result.skipped, ["-ya.flac"]);
        assert_eq!(result.failed.len(), 2);
        assert_eq!(result.failed[0].name, "-roto.wav");
        assert!(!result.failed[0].error.is_empty());
        assert_eq!(result.failed[1], Failure { name: "-raro.xyz".into(), error: "Unsupported file type".into() });
        // The corrupt one does not leave a half-written output
        assert!(!dir.join("-roto.flac").exists());
        // Events: 4 to convert (3 good and the corrupt one), indices from 1; the .flac and the .xyz do not count
        let files = files.lock().unwrap();
        let expected: Vec<(usize, usize, String)> = ["-tono 1.wav", "-roto.wav", "-tono 2.wav", "-tono 3.wav"]
            .iter()
            .enumerate()
            .map(|(i, n)| (i + 1, 4, n.to_string()))
            .collect();
        assert_eq!(*files, expected);
        let percents = percents.lock().unwrap();
        assert!(percents.windows(2).all(|w| w[0] < w[1]), "no es creciente: {percents:?}");
        assert!(percents.iter().all(|p| (0.0..=99.0).contains(p)), "fuera de rango: {percents:?}");
        // With the name already existing it is numbered, the same as convert
        let again = convert_many_with(&paths[..1], "flac", &|_, _, _| {}, Arc::new(|_| {})).unwrap();
        assert!(again.outputs[0].ends_with("-tono 1 2.flac"));
        // Empty list
        let empty = convert_many_with(&[], "flac", &|_, _, _| {}, Arc::new(|_| {}));
        assert!(empty.is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn convertir_varios_se_cancela_y_conserva_lo_hecho() {
        let _guard = lock();
        if !tool_exists("ffmpeg") {
            return;
        }
        let dir = test_dir("varios_cancela");
        let make_video = |name: &str, secs: &str| {
            let path = dir.join(name);
            let args = ["-nostdin", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=25", "-t", secs, "-c:v", "mpeg4"];
            run("ffmpeg", &[&args[..], &[&st(&path)]].concat()).unwrap();
            path
        };
        let short = make_video("-corto.mkv", "1");
        let long = make_video("-largo.mkv", "20");
        let last = dir.join("-ultimo.mkv");
        std::fs::copy(&long, &last).unwrap();
        let paths = vec![st(&short), st(&long), st(&last)];
        // When the second file starts, another thread cancels half a second later
        let on_file = |index: usize, _: usize, _: &str| {
            if index == 2 {
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_millis(500));
                    CANCEL.store(true, Ordering::SeqCst);
                });
            }
        };
        let result = convert_many_with(&paths, "webm", &on_file, Arc::new(|_| {}));
        // Waits for the cancellation thread to finish before clearing the flag
        std::thread::sleep(Duration::from_millis(100));
        CANCEL.store(false, Ordering::SeqCst);
        assert_eq!(result.unwrap_err(), "[cancelled] Cancelled");
        // The first one is kept; the cancelled one leaves no half-written output and the third is not even touched
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["-corto.mkv", "-corto.webm", "-largo.mkv", "-ultimo.mkv"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ----- Messages in both languages -----

    // Launches a long process with CANCEL already set: run_in kills it right away
    fn cancelled_run() -> Result<(), String> {
        CANCEL.store(true, Ordering::SeqCst);
        let result = run_in("sleep", &["5"], None);
        CANCEL.store(false, Ordering::SeqCst);
        result
    }

    #[test]
    fn mensajes_en_ingles_y_espanol() {
        let _guard = lock();
        let dir = std::env::temp_dir().join(format!("mandarina_msg_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        let xyz = dir.join("a.xyz");
        std::fs::write(&png, b"x").unwrap();
        std::fs::write(&xyz, b"x").unwrap();
        let (png, xyz) = (png.to_str().unwrap(), xyz.to_str().unwrap());
        let many = |paths: &[String]| convert_many_with(paths, "jpg", &|_, _, _| {}, Arc::new(|_| {}));

        crate::lang::set_lang(crate::lang::Lang::En);
        assert_eq!(convert_blocking("/no/existe.png", "jpg"), Err("File does not exist".to_string()));
        assert_eq!(convert_blocking(xyz, "jpg"), Err("Unsupported file type".to_string()));
        assert_eq!(convert_blocking(png, "docx"), Err("Cannot convert a.png to docx".to_string()));
        assert_eq!(many(&[]).unwrap_err(), "No files");
        assert_eq!(many(&[xyz.to_string()]).unwrap().failed[0].error, "Unsupported file type");
        assert_eq!(cancelled_run(), Err("[cancelled] Cancelled".to_string()));

        crate::lang::set_lang(crate::lang::Lang::Es);
        assert_eq!(convert_blocking("/no/existe.png", "jpg"), Err("El archivo no existe".to_string()));
        assert_eq!(convert_blocking(xyz, "jpg"), Err("Tipo de archivo no soportado".to_string()));
        assert_eq!(convert_blocking(png, "docx"), Err("No se puede convertir a.png a docx".to_string()));
        assert_eq!(many(&[]).unwrap_err(), "No hay archivos");
        assert_eq!(many(&[xyz.to_string()]).unwrap().failed[0].error, "Tipo de archivo no soportado");
        assert_eq!(
            need(&["herramienta_que_no_existe"], "sudo apt install x"),
            Err(format!("No se encontró herramienta_que_no_existe. Instálalo con: {}", HINT_X))
        );
        assert_eq!(cancelled_run(), Err("[cancelled] Cancelado".to_string()));
        crate::lang::set_lang(crate::lang::Lang::En);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
