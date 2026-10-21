use crate::lang::tr;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, PhysicalSize};

#[derive(Serialize, Debug, PartialEq)]
pub struct FileInfo {
    name: String,
    size: u64,
    is_dir: bool,
}

// Every argument that does not start with "-" (0 is the program name), as absolute
// paths, only those that exist, without repeats and in the order received
fn paths_from_args(args: &[String]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for arg in args.iter().skip(1).filter(|a| !a.starts_with('-')) {
        let Ok(path) = std::path::absolute(arg) else {
            continue;
        };
        if path.exists() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

// Adds up the size of a whole folder without following symbolic links
fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        // symlink_metadata does not follow the link: a symlink counts only as a link
        let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if meta.is_dir() {
            total += dir_size(&entry.path());
        } else {
            total += meta.len();
        }
    }
    total
}

fn info_of(path: &Path) -> Result<FileInfo, String> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| tr("File does not exist", "El archivo no existe"))?;
    // If the path itself is a link, look at what it points to
    let meta = if meta.is_symlink() {
        std::fs::metadata(path).unwrap_or(meta)
    } else {
        meta
    };
    let is_dir = meta.is_dir();
    Ok(FileInfo {
        name: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        size: if is_dir { dir_size(path) } else { meta.len() },
        is_dir,
    })
}

// Top-left corner of a w x h window with its center at (cx, cy),
// without leaving the monitor (mx, my, mw, mh). All in physical pixels.
// If the window is larger than the monitor, it is stuck to its top-left corner.
fn clamp_to_monitor(cx: i32, cy: i32, w: i32, h: i32, mx: i32, my: i32, mw: i32, mh: i32) -> (i32, i32) {
    let x = (cx - w / 2).min(mx + mw - w).max(mx);
    let y = (cy - h / 2).min(my + mh - h).max(my);
    (x, y)
}

// Files that macOS delivered through Finder's "Open With" (an Apple Event, not argv). See lib.rs.
static OPENED: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

// Files the app was launched with: the arguments plus, on macOS, those received through "Open With"
fn launch_paths() -> Vec<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let mut paths = paths_from_args(&args);
    for path in OPENED.lock().map(|o| o.clone()).unwrap_or_default() {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

// Keeps the files of an "Open With" event (only those that exist, without repeats) and returns the
// new ones, which the caller forwards to the page in case it had already started without files
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn add_opened(paths: Vec<PathBuf>) -> Vec<String> {
    let Ok(mut opened) = OPENED.lock() else {
        return Vec::new();
    };
    let mut new = Vec::new();
    for path in paths {
        if path.exists() && !opened.contains(&path) {
            new.push(path.to_string_lossy().to_string());
            opened.push(path);
        }
    }
    new
}

#[tauri::command]
pub fn initial_paths() -> Vec<String> {
    launch_paths().iter().map(|p| p.to_string_lossy().to_string()).collect()
}

#[tauri::command]
pub fn file_info(path: String) -> Result<FileInfo, String> {
    info_of(Path::new(&path))
}

#[tauri::command]
pub fn show_wheel(app: AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if !launch_paths().is_empty() {
        // With files the window starts enlarged by hand until it covers the monitor: on Wayland
        // the app cannot know where the cursor is, but the page does receive it as soon as the
        // window appears under it. Then the interface calls place_wheel to shrink it.
        // set_fullscreen is not used: that mode is managed by the compositor (it hides the top bar,
        // animates the workspace change) and the flicker of that transition would be noticeable on
        // every use. When moving and resizing the window by hand, the compositor does not intervene.
        #[cfg(target_os = "macos")]
        let monitor = app
            .cursor_position()
            .ok()
            .and_then(|c| window.monitor_from_point(c.x, c.y).ok().flatten())
            .or_else(|| window.primary_monitor().ok().flatten());
        #[cfg(not(target_os = "macos"))]
        let monitor = window.primary_monitor().ok().flatten();
        if let Some(monitor) = monitor {
            let pos = monitor.position();
            let size = monitor.size();
            let _ = window.set_position(PhysicalPosition::new(pos.x, pos.y));
            let _ = window.set_size(PhysicalSize::new(size.width, size.height));
        }
    } else {
        let _ = window.center();
    }
    let _ = window.show();
    let _ = window.set_focus();
}

// Puts the window at the logical size (width, height) at (x, y). The window manager ignores the
// move if it arrives right after the resize, so it waits a little and repeats the position.
// It is safe to block here: async commands do not run on the main thread.
fn resize_and_move(window: &tauri::WebviewWindow, width: f64, height: f64, x: i32, y: i32) {
    let _ = window.set_size(LogicalSize::new(width, height));
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(60));
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

// Shrinks the window to 460x460 with its center at (x, y), the cursor in page pixels
// (the window covers the monitor with its origin at (0, 0) of the page, so they are also
// monitor pixels). It is still async because the frontend invokes it with await.
#[tauri::command]
pub async fn place_wheel(app: AppHandle, x: f64, y: f64) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        let _ = window.set_size(LogicalSize::new(460.0, 460.0));
        let _ = window.center();
        return;
    };
    let (mpos, msize) = (monitor.position(), monitor.size());
    let side = (460.0 * scale).round() as i32;
    // macOS keeps windows below the menu bar, so the page origin is the window's, not the monitor's
    #[cfg(target_os = "macos")]
    let origin = window.inner_position().map(|p| (p.x, p.y)).unwrap_or((mpos.x, mpos.y));
    #[cfg(not(target_os = "macos"))]
    let origin = (mpos.x, mpos.y);
    let (cx, cy) = (origin.0 + (x * scale) as i32, origin.1 + (y * scale) as i32);
    let (wx, wy) = clamp_to_monitor(cx, cy, side, side, mpos.x, mpos.y, msize.width as i32, msize.height as i32);
    resize_and_move(&window, 460.0, 460.0, wx, wy);
}

// Cursor in page pixels, when the platform can tell (macOS). On Linux/Wayland it returns None
// and the page waits for the first mouse event instead; on macOS that event only arrives once
// the mouse moves, so the wheel would otherwise open in the center of the screen.
#[tauri::command]
pub fn cursor_in_window(app: AppHandle) -> Option<(f64, f64)> {
    #[cfg(target_os = "macos")]
    {
        let window = app.get_webview_window("main")?;
        let cursor = app.cursor_position().ok()?;
        let origin = window.inner_position().ok()?;
        let scale = window.scale_factor().unwrap_or(1.0);
        Some(((cursor.x - origin.x as f64) / scale, (cursor.y - origin.y as f64) / scale))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        None
    }
}

// Puts the window (460x460) in the center of the monitor. Used for the message cards
// (progress, "Done", errors), which go in the center of the screen and not next to the cursor.
#[tauri::command]
pub async fn center_wheel(app: AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        let _ = window.set_size(LogicalSize::new(460.0, 460.0));
        let _ = window.center();
        return;
    };
    let (mpos, msize) = (monitor.position(), monitor.size());
    let side = (460.0 * scale).round() as i32;
    let (cx, cy) = (mpos.x + msize.width as i32 / 2, mpos.y + msize.height as i32 / 2);
    let (wx, wy) = clamp_to_monitor(cx, cy, side, side, mpos.x, mpos.y, msize.width as i32, msize.height as i32);
    resize_and_move(&window, 460.0, 460.0, wx, wy);
}

// Logical size (width, height) that fits in a monitor of mon_w x mon_h (also logical) leaving
// `margin` on each side. If it already fits it does not change; it never goes below 100 so as not to give absurd sizes.
fn fit_to_monitor(width: f64, height: f64, mon_w: f64, mon_h: f64, margin: f64) -> (f64, f64) {
    let max_w = (mon_w - 2.0 * margin).max(100.0);
    let max_h = (mon_h - 2.0 * margin).max(100.0);
    (width.min(max_w), height.min(max_h))
}

// Puts the window at that logical size, centered on the monitor (for the tools editor).
// If the monitor is smaller, the size is reduced so that it fits with a 24 px margin.
#[tauri::command]
pub async fn resize_center(app: AppHandle, width: u32, height: u32) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        let _ = window.set_size(LogicalSize::new(width as f64, height as f64));
        let _ = window.center();
        return;
    };
    let (mpos, msize) = (monitor.position(), monitor.size());
    let (w, h) = fit_to_monitor(width as f64, height as f64, msize.width as f64 / scale, msize.height as f64 / scale, 24.0);
    let (pw, ph) = ((w * scale).round() as i32, (h * scale).round() as i32);
    let (cx, cy) = (mpos.x + msize.width as i32 / 2, mpos.y + msize.height as i32 / 2);
    let (wx, wy) = clamp_to_monitor(cx, cy, pw, ph, mpos.x, mpos.y, msize.width as i32, msize.height as i32);
    resize_and_move(&window, w, h, wx, wy);
}

#[tauri::command]
pub fn cancel() {
    crate::converters::CANCEL.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn ventana_centrada_en_el_cursor() {
        // 1920x1080 monitor at the origin, 460x460 window
        assert_eq!(clamp_to_monitor(1000, 500, 460, 460, 0, 0, 1920, 1080), (770, 270));
    }

    #[test]
    fn ventana_limitada_al_monitor() {
        assert_eq!(clamp_to_monitor(0, 0, 460, 460, 0, 0, 1920, 1080), (0, 0));
        assert_eq!(clamp_to_monitor(1920, 1080, 460, 460, 0, 0, 1920, 1080), (1460, 620));
        // Second monitor on the right (origin 1920,0)
        assert_eq!(clamp_to_monitor(1930, 10, 460, 460, 1920, 0, 1920, 1080), (1920, 0));
        // Monitor on the left with a negative origin
        assert_eq!(clamp_to_monitor(-5, 500, 460, 460, -1920, 0, 1920, 1080), (-460, 270));
        // Window larger than the monitor
        assert_eq!(clamp_to_monitor(100, 100, 500, 500, 0, 0, 300, 300), (0, 0));
    }

    #[test]
    fn tamano_ajustado_al_monitor() {
        // It fits: no change
        assert_eq!(fit_to_monitor(780.0, 640.0, 1920.0, 1080.0, 24.0), (780.0, 640.0));
        // Smaller monitor: it is reduced, leaving a margin on each side
        assert_eq!(fit_to_monitor(780.0, 640.0, 700.0, 600.0, 24.0), (652.0, 552.0));
        // Only one dimension does not fit
        assert_eq!(fit_to_monitor(780.0, 640.0, 1920.0, 500.0, 24.0), (780.0, 452.0));
        // Tiny monitor: never less than 100
        assert_eq!(fit_to_monitor(780.0, 640.0, 50.0, 50.0, 24.0), (100.0, 100.0));
    }

    #[test]
    fn abrir_con_guarda_solo_lo_nuevo_que_existe() {
        let dir = std::env::temp_dir().join(format!("mandarina_open_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a b.txt");
        std::fs::write(&file, "x").unwrap();
        let missing = dir.join("no existe.txt");
        assert_eq!(add_opened(vec![file.clone(), missing, file.clone()]), vec![file.to_string_lossy().to_string()]);
        // The same file again is not new
        assert!(add_opened(vec![file.clone()]).is_empty());
        assert!(launch_paths().contains(&file));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rutas_iniciales_desde_argumentos() {
        let dir = std::env::temp_dir().join(format!("mandarina_w_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Real path: on macOS the temp dir lives behind the /var -> /private/var symlink
        let dir = dir.canonicalize().unwrap();
        let file = dir.join("a.txt");
        let other = dir.join("b b.txt");
        std::fs::write(&file, b"x").unwrap();
        std::fs::write(&other, b"x").unwrap();
        let file_str = file.to_str().unwrap();
        let other_str = other.to_str().unwrap();
        // Ignores the program name and the options
        assert_eq!(paths_from_args(&args(&["mandarina", "--flag", file_str])), vec![file.clone()]);
        // Several paths, in the order received
        assert_eq!(
            paths_from_args(&args(&["mandarina", other_str, file_str])),
            vec![other.clone(), file.clone()]
        );
        // No duplicates (also if one comes relative and another absolute)
        assert_eq!(
            paths_from_args(&args(&["mandarina", file_str, other_str, file_str])),
            vec![file.clone(), other.clone()]
        );
        // One that does not exist in between is discarded and the others go on
        assert_eq!(
            paths_from_args(&args(&["mandarina", file_str, "/no/existe/nada", "-x", other_str])),
            vec![file.clone(), other.clone()]
        );
        // A folder is also valid
        assert_eq!(paths_from_args(&args(&["mandarina", dir.to_str().unwrap()])), vec![dir.clone()]);
        // Relative path: it becomes absolute
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let found = paths_from_args(&args(&["mandarina", "a.txt", file_str]));
        std::env::set_current_dir(old_dir).unwrap();
        assert_eq!(found, vec![file.clone()]);
        // It does not exist, or there are no arguments
        assert!(paths_from_args(&args(&["mandarina", "/no/existe/nada"])).is_empty());
        assert!(paths_from_args(&args(&["mandarina"])).is_empty());
        assert!(paths_from_args(&args(&["mandarina", "-x"])).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn informacion_de_archivo_y_carpeta() {
        let dir = std::env::temp_dir().join(format!("mandarina_fi_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), b"12345").unwrap();
        std::fs::write(dir.join("sub").join("b.txt"), b"123").unwrap();
        // The link to a large file outside the folder must not add up its content
        let big = std::env::temp_dir().join(format!("mandarina_fi_big_{}", std::process::id()));
        std::fs::write(&big, vec![0u8; 100_000]).unwrap();
        std::os::unix::fs::symlink(&big, dir.join("enlace")).unwrap();

        let file = info_of(&dir.join("a.txt")).unwrap();
        assert_eq!(file, FileInfo { name: "a.txt".into(), size: 5, is_dir: false });
        let folder = info_of(&dir).unwrap();
        assert!(folder.is_dir);
        assert!(folder.size >= 8 && folder.size < 1000, "tamaño: {}", folder.size);
        assert_eq!(info_of(&dir.join("no_existe")), Err("File does not exist".to_string()));
        crate::lang::set_lang(crate::lang::Lang::Es);
        assert_eq!(info_of(&dir.join("no_existe")), Err("El archivo no existe".to_string()));
        crate::lang::set_lang(crate::lang::Lang::En);
        std::fs::remove_file(&big).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
