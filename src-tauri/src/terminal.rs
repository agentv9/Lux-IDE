use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;
use tauri::{State, ipc::Channel};

struct Terminal {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
pub struct Terminals {
    sessions: Mutex<HashMap<String, Terminal>>,
}

impl Terminals {
    pub fn close_all(&self) {
        if let Ok(mut terminals) = self.sessions.lock() {
            terminals.clear();
        }
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        cols: cols.max(1),
        rows: rows.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[tauri::command]
pub fn terminal_open(
    id: String,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    output: Channel<Vec<u8>>,
    state: State<Terminals>,
) -> Result<(), String> {
    let pair = native_pty_system()
        .openpty(size(cols, rows))
        .map_err(|error| error.to_string())?;
    let mut shell = "sh";
    if cfg!(windows) {
        shell = "powershell.exe";
    }
    let mut command = CommandBuilder::new(shell);
    if cfg!(windows) {
        command.arg("-NoLogo");
        command.arg("-NoExit");
        command.arg("-Command");
        command.arg("[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new(); $OutputEncoding = [Console]::OutputEncoding");
    }
    if let Some(path) = cwd {
        command.cwd(path);
    }
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| error.to_string())?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| error.to_string())?;
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| error.to_string())?;
    let terminal = Terminal {
        master: pair.master,
        writer,
        child,
    };
    state
        .sessions
        .lock()
        .map_err(|error| error.to_string())?
        .insert(id, terminal);
    std::thread::spawn(move || {
        let mut buffer = [0; 8192];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            let data = buffer[..count].to_vec();
            if output.send(data).is_err() {
                break;
            }
        }
        let _ = output.send(Vec::new());
    });
    Ok(())
}

#[tauri::command]
pub fn terminal_write(id: String, data: String, state: State<Terminals>) -> Result<(), String> {
    let mut terminals = state.sessions.lock().map_err(|error| error.to_string())?;
    let terminal = terminals.get_mut(&id).ok_or("Terminal is closed")?;
    terminal
        .writer
        .write_all(data.as_bytes())
        .map_err(|error| error.to_string())?;
    terminal.writer.flush().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn terminal_resize(
    id: String,
    cols: u16,
    rows: u16,
    state: State<Terminals>,
) -> Result<(), String> {
    let terminals = state.sessions.lock().map_err(|error| error.to_string())?;
    let terminal = terminals.get(&id).ok_or("Terminal is closed")?;
    terminal
        .master
        .resize(size(cols, rows))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn terminal_close(id: String, state: State<Terminals>) -> Result<(), String> {
    state
        .sessions
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&id);
    Ok(())
}
