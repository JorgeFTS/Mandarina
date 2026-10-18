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

// Todos los argumentos que no empiecen por "-" (el 0 es el nombre del programa), como rutas
// absolutas, solo los que existen, sin repetir y en el orden recibido
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

// Suma el tamaño de una carpeta entera sin seguir enlaces simbólicos
fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        // symlink_metadata no sigue el enlace: un symlink cuenta solo como enlace
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
    // Si la ruta misma es un enlace, se mira a qué apunta
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

// Esquina superior izquierda de una ventana w x h con su centro en (cx, cy),
// sin salirse del monitor (mx, my, mw, mh). Todo en píxeles físicos.
// Si la ventana es más grande que el monitor, se pega a su esquina superior izquierda.
fn clamp_to_monitor(cx: i32, cy: i32, w: i32, h: i32, mx: i32, my: i32, mw: i32, mh: i32) -> (i32, i32) {
    let x = (cx - w / 2).min(mx + mw - w).max(mx);
    let y = (cy - h / 2).min(my + mh - h).max(my);
    (x, y)
}

#[tauri::command]
pub fn initial_paths() -> Vec<String> {
    let args: Vec<String> = std::env::args().collect();
    paths_from_args(&args)
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect()
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
    let args: Vec<String> = std::env::args().collect();
    if !paths_from_args(&args).is_empty() {
        // Con archivos la ventana empieza agrandada a mano hasta cubrir el monitor: en Wayland
        // la app no puede saber dónde está el cursor, pero la página sí lo recibe en cuanto la
        // ventana aparece debajo de él. Después la interfaz llama a place_wheel para encogerla.
        // No se usa set_fullscreen: ese modo lo gestiona el compositor (oculta la barra superior,
        // anima el cambio de espacio de trabajo) y el parpadeo de esa transición se notaría en
        // cada uso. Al mover y redimensionar la ventana a mano, el compositor no interviene.
        if let Some(monitor) = window.primary_monitor().ok().flatten() {
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

// Pone la ventana con el tamaño lógico (width, height) en (x, y). El gestor de ventanas ignora el
// movimiento si llega pegado al cambio de tamaño, así que se espera un poco y se repite la posición.
// Es seguro bloquear aquí: los comandos async no corren en el hilo principal.
fn resize_and_move(window: &tauri::WebviewWindow, width: f64, height: f64, x: i32, y: i32) {
    let _ = window.set_size(LogicalSize::new(width, height));
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(60));
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

// Encoge la ventana a 460x460 con su centro en (x, y), el cursor en píxeles de la página
// (la ventana cubre el monitor con su origen en (0, 0) de la página, así que son también
// píxeles del monitor). Sigue siendo async porque el frontend la invoca con await.
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
    let (cx, cy) = (mpos.x + (x * scale) as i32, mpos.y + (y * scale) as i32);
    let (wx, wy) = clamp_to_monitor(cx, cy, side, side, mpos.x, mpos.y, msize.width as i32, msize.height as i32);
    resize_and_move(&window, 460.0, 460.0, wx, wy);
}

// Pone la ventana (460x460) en el centro del monitor. Se usa para las tarjetas de mensajes
// (progreso, "Listo", errores), que van en el centro de la pantalla y no junto al cursor.
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

// Tamaño lógico (ancho, alto) que cabe en un monitor de mon_w x mon_h (también lógicos) dejando
// `margin` a cada lado. Si ya cabe no cambia; nunca baja de 100 para no dar tamaños absurdos.
fn fit_to_monitor(width: f64, height: f64, mon_w: f64, mon_h: f64, margin: f64) -> (f64, f64) {
    let max_w = (mon_w - 2.0 * margin).max(100.0);
    let max_h = (mon_h - 2.0 * margin).max(100.0);
    (width.min(max_w), height.min(max_h))
}

// Pone la ventana con ese tamaño lógico centrada en el monitor (para el editor de las herramientas).
// Si el monitor es más pequeño, el tamaño se reduce para que quepa con un margen de 24 px.
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
        // Monitor 1920x1080 en el origen, ventana 460x460
        assert_eq!(clamp_to_monitor(1000, 500, 460, 460, 0, 0, 1920, 1080), (770, 270));
    }

    #[test]
    fn ventana_limitada_al_monitor() {
        assert_eq!(clamp_to_monitor(0, 0, 460, 460, 0, 0, 1920, 1080), (0, 0));
        assert_eq!(clamp_to_monitor(1920, 1080, 460, 460, 0, 0, 1920, 1080), (1460, 620));
        // Segundo monitor a la derecha (origen 1920,0)
        assert_eq!(clamp_to_monitor(1930, 10, 460, 460, 1920, 0, 1920, 1080), (1920, 0));
        // Monitor a la izquierda con origen negativo
        assert_eq!(clamp_to_monitor(-5, 500, 460, 460, -1920, 0, 1920, 1080), (-460, 270));
        // Ventana más grande que el monitor
        assert_eq!(clamp_to_monitor(100, 100, 500, 500, 0, 0, 300, 300), (0, 0));
    }

    #[test]
    fn tamano_ajustado_al_monitor() {
        // Cabe: no cambia
        assert_eq!(fit_to_monitor(780.0, 640.0, 1920.0, 1080.0, 24.0), (780.0, 640.0));
        // Monitor más pequeño: se reduce dejando margen a cada lado
        assert_eq!(fit_to_monitor(780.0, 640.0, 700.0, 600.0, 24.0), (652.0, 552.0));
        // Solo una dimensión no cabe
        assert_eq!(fit_to_monitor(780.0, 640.0, 1920.0, 500.0, 24.0), (780.0, 452.0));
        // Monitor diminuto: nunca menos de 100
        assert_eq!(fit_to_monitor(780.0, 640.0, 50.0, 50.0, 24.0), (100.0, 100.0));
    }

    #[test]
    fn rutas_iniciales_desde_argumentos() {
        let dir = std::env::temp_dir().join(format!("mandarina_w_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        let other = dir.join("b b.txt");
        std::fs::write(&file, b"x").unwrap();
        std::fs::write(&other, b"x").unwrap();
        let file_str = file.to_str().unwrap();
        let other_str = other.to_str().unwrap();
        // Ignora el nombre del programa y las opciones
        assert_eq!(paths_from_args(&args(&["mandarina", "--flag", file_str])), vec![file.clone()]);
        // Varias rutas, en el orden recibido
        assert_eq!(
            paths_from_args(&args(&["mandarina", other_str, file_str])),
            vec![other.clone(), file.clone()]
        );
        // Sin duplicados (también si una viene relativa y otra absoluta)
        assert_eq!(
            paths_from_args(&args(&["mandarina", file_str, other_str, file_str])),
            vec![file.clone(), other.clone()]
        );
        // Una que no existe entre medias se descarta y las demás siguen
        assert_eq!(
            paths_from_args(&args(&["mandarina", file_str, "/no/existe/nada", "-x", other_str])),
            vec![file.clone(), other.clone()]
        );
        // Una carpeta también vale
        assert_eq!(paths_from_args(&args(&["mandarina", dir.to_str().unwrap()])), vec![dir.clone()]);
        // Ruta relativa: se vuelve absoluta
        let old_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let found = paths_from_args(&args(&["mandarina", "a.txt", file_str]));
        std::env::set_current_dir(old_dir).unwrap();
        assert_eq!(found, vec![file.clone()]);
        // No existe, o no hay argumentos
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
        // El enlace a un archivo grande fuera de la carpeta no debe sumar su contenido
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
