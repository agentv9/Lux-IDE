use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{Manager, State, ipc::Channel};

struct Server {
    child: Child,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
pub struct TypeScript {
    server: Mutex<Option<Server>>,
}

fn server_command(script: &Path) -> Command {
    let mut command = Command::new("node");
    // Node cannot resolve Tauri's Windows verbatim paths (\\?\C:\...).
    command.arg(dunce::simplified(script));
    command.arg("--useInferredProjectPerProjectRoot");
    command.arg("--disableAutomaticTypingAcquisition");
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Hide the extra console window.
        command.creation_flags(0x08000000);
    }
    command
}

#[tauri::command]
pub fn typescript_start(
    app: tauri::AppHandle,
    output: Channel<serde_json::Value>,
    state: State<TypeScript>,
) -> Result<(), String> {
    let mut server = state.server.lock().map_err(|error| error.to_string())?;
    // A frontend reload needs a fresh channel and document state.
    server.take();
    let script = app
        .path()
        .resolve(
            "typescript/tsserver.js",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|error| error.to_string())?;
    if !script.is_file() {
        return Err(format!(
            "TypeScript server is missing: {}",
            script.display()
        ));
    }
    let mut command = server_command(&script);
    let mut child = command.spawn().map_err(|error| {
        format!("Could not start TypeScript. Install Node.js and restart Lux: {error}")
    })?;
    let stdout = child.stdout.take().ok_or("Missing TypeScript output")?;
    let stderr = child.stderr.take().ok_or("Missing TypeScript errors")?;
    let errors = output.clone();
    let error_reader = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let line = match line {
                Ok(line) => line,
                Err(_) => break,
            };
            let _ = errors
                .send(serde_json::json!({ "type": "event", "event": "serverError", "body": line }));
        }
    });
    *server = Some(Server { child });
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            let count = reader.read_line(&mut line).unwrap_or(0);
            if count == 0 {
                break;
            }

            let header = match line.strip_prefix("Content-Length: ") {
                Some(header) => header,
                None => continue,
            };
            let length = match header.trim().parse::<usize>() {
                Ok(length) => length,
                Err(_) => break,
            };
            line.clear();
            if reader.read_line(&mut line).is_err() {
                break;
            }
            let mut body = vec![0; length];
            if reader.read_exact(&mut body).is_err() {
                break;
            }
            if let Ok(message) = serde_json::from_slice(&body) {
                if output.send(message).is_err() {
                    break;
                }
            }
        }
        let _ = error_reader.join();
        let _ = output.send(serde_json::json!({ "type": "event", "event": "serverExit" }));
    });
    Ok(())
}

#[tauri::command]
pub fn typescript_send(message: serde_json::Value, state: State<TypeScript>) -> Result<(), String> {
    let mut server = state.server.lock().map_err(|error| error.to_string())?;
    let server = server.as_mut().ok_or("TypeScript server is not running")?;
    let stdin = server
        .child
        .stdin
        .as_mut()
        .ok_or("TypeScript input is closed")?;
    writeln!(stdin, "{message}").map_err(|error| error.to_string())?;
    stdin.flush().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn typescript_stop(state: State<TypeScript>) -> Result<(), String> {
    state
        .server
        .lock()
        .map_err(|error| error.to_string())?
        .take();
    Ok(())
}
