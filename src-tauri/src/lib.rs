mod config;
mod converters;
mod lang;
mod thumbnail;
mod tools;
mod wheel;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "macos")]
    extend_path_macos();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|_app| {
            // No Dock icon or app menu: the wheel is a floating accessory window
            #[cfg(target_os = "macos")]
            _app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            lang::get_language,
            config::get_prefs,
            config::set_language,
            config::set_accent,
            config::get_config,
            config::set_formats,
            config::reset_formats,
            converters::get_targets,
            converters::convert,
            converters::get_targets_many,
            converters::take_warnings,
            converters::convert_many,
            thumbnail::thumbnail,
            wheel::initial_paths,
            wheel::file_info,
            wheel::show_wheel,
            wheel::place_wheel,
            wheel::cursor_in_window,
            wheel::center_wheel,
            wheel::resize_center,
            tools::get_tools,
            tools::run_tool,
            tools::media_info,
            tools::preview_image,
            tools::video_frame,
            tools::waveform,
            wheel::cancel,
            wheel::quit
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Apps launched from Finder or a Quick Action get a bare PATH (/usr/bin:/bin:...), so the
// converters installed by Homebrew, in ~/.local/bin or inside LibreOffice.app would not be found
#[cfg(target_os = "macos")]
fn extend_path_macos() {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join("Applications/LibreOffice.app/Contents/MacOS"));
    }
    // ffmpeg-full is keg-only (Homebrew does not link it into bin), so its folder goes first
    for dir in [
        "/opt/homebrew/opt/ffmpeg-full/bin",
        "/usr/local/opt/ffmpeg-full/bin",
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/opt/local/bin",
        "/Applications/LibreOffice.app/Contents/MacOS",
    ] {
        dirs.push(dir.into());
    }
    let current = std::env::var_os("PATH").unwrap_or_default();
    dirs.extend(std::env::split_paths(&current));
    if let Ok(joined) = std::env::join_paths(dirs) {
        // Safe: it runs first thing in main, before any other thread exists
        unsafe { std::env::set_var("PATH", joined) };
    }
}
