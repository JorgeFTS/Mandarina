mod config;
mod converters;
mod lang;
mod thumbnail;
mod tools;
mod wheel;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            converters::convert_many,
            thumbnail::thumbnail,
            wheel::initial_paths,
            wheel::file_info,
            wheel::show_wheel,
            wheel::place_wheel,
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
