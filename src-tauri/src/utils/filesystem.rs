use crate::utils::{DirectoryEntry, FileEntry};
use std::{fs, io::ErrorKind, path::PathBuf};

#[tauri::command]
pub fn read_directory(path: String) -> Result<Vec<DirectoryEntry>, String> {
    let directory = fs::read_dir(&path).map_err(|error| error.to_string())?;

    let mut entries = Vec::new();

    for entry in directory {
        let entry = entry.map_err(|error| error.to_string())?;

        let file_type = entry.file_type().map_err(|error| error.to_string())?;

        entries.push(DirectoryEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: entry.path().to_string_lossy().into_owned(),
            is_directory: file_type.is_dir(),
        });
    }

    // Folders first, then alphabetical order.
    entries.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

#[tauri::command]
pub fn read_file(path: String) -> Result<FileEntry, String> {
    let file: String = fs::read_to_string(&path).map_err(|error| error.to_string())?;

    Ok(FileEntry {
        name: String::from("name"),
        path,
        content: file,
    })
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, &content).map_err(|error: std::io::Error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn create_file(path: String) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn create_directory(path: String) -> Result<(), String> {
    fs::create_dir(&path).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_filedir(path: String) -> Result<(), String> {
    //Check if is file or dir, for now if dir ignore, if file delete.

    let metadata: fs::Metadata =
        fs::metadata(&path).map_err(|error: std::io::Error| error.to_string())?;
    if metadata.is_dir() {
        return Ok(());
    };

    fs::remove_file(&path).map_err(|error: std::io::Error| error.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn move_entry(old_path: String, new_path: String) -> Result<(), String> {
    let source = PathBuf::from(&old_path);
    let destination = PathBuf::from(&new_path);

    let source_metadata =
        fs::symlink_metadata(&source).map_err(|error| format!("Cannot access source: {error}"))?;

    let destination_parent = destination
        .parent()
        .ok_or("Destination has no parent directory")?;

    let canonical_parent = fs::canonicalize(destination_parent)
        .map_err(|error| format!("Cannot access destination folder: {error}"))?;

    if !canonical_parent.is_dir() {
        return Err("Destination parent is not a directory".into());
    }

    // Check directories using resolved paths so aliases such as ".."
    // cannot bypass the descendant check.
    if source_metadata.is_dir() {
        let canonical_source = fs::canonicalize(&source)
            .map_err(|error| format!("Cannot resolve source folder: {error}"))?;

        if canonical_parent.starts_with(&canonical_source) {
            return Err("Cannot move a directory into itself or its descendants".into());
        }
    }

    // Reject existing files, directories, and dangling symbolic links.
    match fs::symlink_metadata(&destination) {
        Ok(_) => {
            return Err("An entry already exists at the destination".into());
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("Cannot access destination: {error}"));
        }
    }

    fs::rename(&source, &destination).map_err(|error| format!("Could not move entry: {error}"))?;

    Ok(())
}
