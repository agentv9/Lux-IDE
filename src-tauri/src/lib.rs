mod utils;
mod discord;
mod terminal;
mod typescript;
use tauri::Manager;


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(terminal::Terminals::default())
        .manage(typescript::TypeScript::default())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window.state::<terminal::Terminals>().close_all();
                let _ = typescript::typescript_stop(window.state::<typescript::TypeScript>());
            }
        })
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
            utils::filesystem::move_entry,
            terminal::terminal_open, terminal::terminal_write, terminal::terminal_resize, terminal::terminal_close,
            typescript::typescript_start, typescript::typescript_send, typescript::typescript_stop])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
