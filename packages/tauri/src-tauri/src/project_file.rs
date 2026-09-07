use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::ipc::Response;
use tauri::State;

use crate::fs::resolve_target;
use crate::state::AppInner;

/// Mirrors core's PROJECT_FILE_MAX_BYTES: the size gate must run before the
/// bytes are read into memory, so it is decided here; the text/binary decision
/// itself stays in core (`classifyProjectFile`, run over the raw bytes by the
/// renderer).
const PROJECT_FILE_MAX_BYTES: u64 = 1024 * 1024;

/// Same guard as fs_op but rooted at the project folder (the one holding
/// `.boardown/`) — and additionally rejects the root itself, which is a
/// folder, not a file.
fn resolve_project_target(root: &Path, user_path: &str) -> Option<PathBuf> {
    let target = resolve_target(root, user_path)?;
    if target == root {
        return None;
    }
    Some(target)
}

/// Returns the file's raw bytes (a raw IPC response, no JSON envelope), or an
/// Err whose string is a classification tag the renderer maps onto
/// ProjectFileRead.
#[tauri::command]
pub fn read_project_file(
    state: State<'_, Arc<Mutex<AppInner>>>,
    path: String,
) -> Result<Response, String> {
    let inner = state.lock().map_err(|_| "lock poisoned".to_string())?;
    let Some(root) = inner.root.clone() else {
        return Err("unreadable".to_string());
    };
    let target =
        resolve_project_target(&root, &path).ok_or_else(|| "unreadable".to_string())?;
    let meta = fs::metadata(&target).map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            "not-found".to_string()
        } else {
            "unreadable".to_string()
        }
    })?;
    if !meta.is_file() {
        return Err("unreadable".to_string());
    }
    if meta.len() > PROJECT_FILE_MAX_BYTES {
        return Err("too-large".to_string());
    }
    let bytes = fs::read(&target).map_err(|_| "unreadable".to_string())?;
    Ok(Response::new(bytes))
}
