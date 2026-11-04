mod utils;
mod discord;


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            discord::rich_presence::init(app.handle());
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![utils::filesystem::read_directory, 
            utils::filesystem::read_file, 
            utils::filesystem::write_file, 
            utils::filesystem::create_file,
            utils::filesystem::delete_filedir,
            utils::filesystem::move_entry])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
