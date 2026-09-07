// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod fs;
mod git_run;
mod project_file;
mod state;
mod watch;

use std::sync::{Arc, Mutex};

use tauri::State;
use tauri_plugin_dialog::DialogExt;

use state::AppInner;

/// Open the native folder picker, make the picked project folder the board
/// context and start the auto-refresh watcher. Returns the folder, or null on
/// cancel. The renderer never names a board root itself — only a path chosen
/// in this dialog can become one.
#[tauri::command]
async fn open_folder(
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<AppInner>>>,
) -> Result<Option<String>, String> {
    let picked = app.dialog().file().blocking_pick_folder();
    let Some(folder) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(None);
    };
    if !folder.is_dir() {
        return Ok(None);
    }

    let board_root = folder.join(".boardown");
    // ponytail: a folder opened before its board exists (fresh onboarding
    // inside the app) gets no auto-refresh until it is re-opened — the watcher
    // has nothing to attach to. Upgrade path: watch the project root for
    // `.boardown` creation and re-attach.
    let watcher = if board_root.is_dir() {
        Some(watch::start(board_root, app.clone(), Arc::clone(&state)).map_err(|err| err.to_string())?)
    } else {
        None
    };

    let mut inner = state.lock().map_err(|_| "lock poisoned".to_string())?;
    inner.root = Some(folder.clone());
    inner.own_writes.clear();
    inner.watcher = watcher;
    Ok(Some(folder.to_string_lossy().into_owned()))
}

/// Drop the board context and stop the watcher; the renderer returns to the
/// welcome screen.
#[tauri::command]
fn close_folder(state: State<'_, Arc<Mutex<AppInner>>>) -> Result<(), String> {
    let mut inner = state.lock().map_err(|_| "lock poisoned".to_string())?;
    inner.root = None;
    inner.own_writes.clear();
    inner.watcher = None;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(Mutex::new(AppInner::default())))
        .invoke_handler(tauri::generate_handler![
            open_folder,
            close_folder,
            fs::fs_op,
            project_file::read_project_file,
            git_run::git_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running boardown");
}
