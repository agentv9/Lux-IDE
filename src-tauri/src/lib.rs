mod discord;
mod terminal;
mod typescript;
mod utils;
use std::env;
use tauri::{Emitter, Manager};

fn parse_file_arg(args: Vec<String>) -> Option<String> {
    if args.len() > 1 {
        let path = args[1].clone();
        if !path.starts_with("--") {
            return Some(path);
        }
    }
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(file_path) = parse_file_arg(args) {
                // Emit raw path to Vue
                let _ = app.emit("context-menu-path", file_path);
                
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        }))
        .manage(terminal::Terminals::default())
        .manage(typescript::TypeScript::default())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window.state::<terminal::Terminals>().close_all();
                let _ = typescript::typescript_stop(window.state::<typescript::TypeScript>());
            }
        })
        .setup(|app| {
            let args: Vec<String> = env::args().collect();
            if let Some(file_path) = parse_file_arg(args) {
                let app_handle = app.handle().clone();
                // Wait for Vue app to finish mounting before sending the string
                tauri::async_runtime::spawn(async move {
                    std::thread::sleep(std::time::Duration::from_millis(600));
                    let _ = app_handle.emit("context-menu-path", file_path);
                });
            }
            discord::rich_presence::init(app.handle());
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            utils::filesystem::read_directory,
            utils::filesystem::read_file,
            utils::filesystem::write_file,
            utils::filesystem::create_file,
            utils::filesystem::delete_filedir,
            utils::filesystem::move_entry,
            terminal::terminal_open,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_close,
            typescript::typescript_start,
            typescript::typescript_send,
            typescript::typescript_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
