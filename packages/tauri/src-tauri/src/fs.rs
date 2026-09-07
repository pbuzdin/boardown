use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::state::AppInner;

/// How long after the host writes a file its own watch event is ignored, so the
/// renderer's own saves don't bounce back as an "external change" refresh.
pub const ECHO_WINDOW: Duration = Duration::from_secs(2);

/// Join a renderer-supplied relative path onto `root`, rejecting absolute
/// paths and any '..' escape — the Rust mirror of packages/electron's
/// resolveTarget, the sole boundary between the renderer and arbitrary disk
/// paths. Lexical, like Node's path.resolve: it does not touch the disk, so it
/// is equally correct for targets that do not exist yet.
pub fn resolve_target(root: &Path, user_path: &str) -> Option<PathBuf> {
    let normalized = user_path.replace('\\', "/");
    if normalized.starts_with('/') {
        return None;
    }
    // A drive letter (`C:/…`), the other shape of absolute on Windows.
    let mut chars = normalized.chars();
    if let (Some(first), Some(':')) = (chars.next(), chars.next()) {
        if first.is_ascii_alphabetic() {
            return None;
        }
    }
    let mut parts: Vec<OsString> = Vec::new();
    for segment in normalized.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?; // escaping above the root
            }
            other => parts.push(other.into()),
        }
    }
    let mut target = root.to_path_buf();
    for part in parts {
        target.push(part);
    }
    Some(target)
}

/// Record the host's own write so the watcher suppresses its echo. The parent
/// is recorded too, since some platforms emit a directory event for it.
/// Stale entries are pruned here, on the only path that grows the map.
pub fn record_write(inner: &mut AppInner, target: &Path) {
    let now = Instant::now();
    inner
        .own_writes
        .retain(|_, at| now.duration_since(*at) < ECHO_WINDOW);
    if let Some(parent) = target.parent() {
        inner.own_writes.insert(parent.to_path_buf(), now);
    }
    inner.own_writes.insert(target.to_path_buf(), now);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsRequest {
    method: String,
    path: String,
    content: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileStat {
    last_modified: u64,
}

#[derive(Serialize)]
struct FsEntry {
    name: String,
    is_directory: bool,
}

/// Run one FsAdapter operation against the open board's `.boardown/` root.
/// NotFound is an expected, handled case for callers (missing config ->
/// onboarding, optional backlog), so read -> null, list -> [], stat -> null;
/// the renderer turns a null read back into the rejection FsAdapter.read
/// promises.
#[tauri::command]
pub fn fs_op(state: State<'_, Arc<Mutex<AppInner>>>, req: FsRequest) -> Result<Value, String> {
    let mut inner = state.lock().map_err(|_| "lock poisoned".to_string())?;
    let Some(root) = inner.root.clone() else {
        return Err("No project is open".to_string());
    };
    let board_root = root.join(".boardown");
    let target = resolve_target(&board_root, &req.path)
        .ok_or_else(|| format!("Invalid path: {}", req.path))?;
    let method = req.method.clone();
    let fail = |err: std::io::Error| format!("{} ({})", err, method);

    match req.method.as_str() {
        "read" => match fs::read(&target) {
            Ok(bytes) => Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned())),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Value::Null),
            Err(err) => Err(fail(err)),
        },
        "write" => {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(fail)?;
            }
            fs::write(&target, req.content.unwrap_or_default()).map_err(fail)?;
            record_write(&mut inner, &target);
            Ok(Value::Null)
        }
        "list" => match fs::read_dir(&target) {
            Ok(entries) => {
                let listed: Vec<Value> = entries
                    .filter_map(|entry| {
                        let entry = entry.ok()?;
                        let file_type = entry.file_type().ok()?;
                        Some(
                            serde_json::to_value(FsEntry {
                                name: entry.file_name().to_string_lossy().into_owned(),
                                is_directory: file_type.is_dir(),
                            })
                            .ok()?,
                        )
                    })
                    .collect();
                Ok(Value::Array(listed))
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Value::Array(vec![])),
            Err(err) => Err(fail(err)),
        },
        "mkdir" => {
            fs::create_dir_all(&target).map_err(fail)?;
            record_write(&mut inner, &target);
            Ok(Value::Null)
        }
        "remove" => {
            match fs::symlink_metadata(&target) {
                Ok(meta) if meta.is_dir() => fs::remove_dir_all(&target).map_err(fail)?,
                Ok(_) => fs::remove_file(&target).map_err(fail)?,
                // Removing a path that does not exist is not an error.
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(fail(err)),
            }
            record_write(&mut inner, &target);
            Ok(Value::Null)
        }
        "stat" => match fs::metadata(&target) {
            Ok(meta) => {
                let last_modified = meta
                    .modified()
                    .ok()
                    .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|age| age.as_millis() as u64)
                    .unwrap_or(0);
                serde_json::to_value(FileStat { last_modified })
                    .map_err(|_| "serialization failed".to_string())
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Value::Null),
            Err(err) => Err(fail(err)),
        },
        other => Err(format!("Unknown fs method: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::resolve_target;

    #[test]
    fn joins_relative_paths_under_the_root() {
        let root = Path::new("/board");
        assert_eq!(
            resolve_target(root, "releases/2026-07.md"),
            Some(PathBuf::from("/board/releases/2026-07.md"))
        );
        assert_eq!(
            resolve_target(root, ".\\epics\\a.md"),
            Some(PathBuf::from("/board/epics/a.md"))
        );
        assert_eq!(
            resolve_target(root, "a/./b.md"),
            Some(PathBuf::from("/board/a/b.md"))
        );
    }

    #[test]
    fn rejects_escapes_and_absolute_paths() {
        let root = Path::new("/board");
        assert_eq!(resolve_target(root, "../outside.md"), None);
        assert_eq!(resolve_target(root, "a/../../outside.md"), None);
        assert_eq!(resolve_target(root, "/etc/passwd"), None);
        assert_eq!(resolve_target(root, "C:/Windows"), None);
        assert_eq!(resolve_target(root, "c:/Windows"), None);
    }
}
