use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::fs::ECHO_WINDOW;
use crate::state::AppInner;

/// Collapse a burst of changes (e.g. a `git checkout`) into a single refresh —
/// the flusher thread wakes on this tick, drains what arrived and emits once.
const FLUSH_TICK: Duration = Duration::from_millis(200);

/// Keeps the OS watch alive. Dropping it stops the watcher, which closes the
/// channel the flusher thread reads and so ends the thread.
pub struct BoardWatcher {
    _inner: RecommendedWatcher,
}

/// Watch the open board's `.boardown/` recursively and emit `board-changed`
/// for external changes — the echo of the host's own writes suppressed.
/// The caller must ensure the directory exists.
pub fn start(
    board_root: PathBuf,
    app: AppHandle,
    state: Arc<Mutex<AppInner>>,
) -> notify::Result<BoardWatcher> {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    watcher.watch(&board_root, RecursiveMode::Recursive)?;

    std::thread::spawn(move || loop {
        std::thread::sleep(FLUSH_TICK);
        let mut events = Vec::new();
        let mut connected = true;
        loop {
            match rx.try_recv() {
                Ok(Ok(event)) => events.extend(event.paths),
                Ok(Err(_)) => {} // watch error: nothing usable this batch
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    connected = false;
                    break;
                }
            }
        }
        if !connected {
            return;
        }
        if events.is_empty() {
            continue;
        }
        let external = {
            let mut inner = match state.lock() {
                Ok(inner) => inner,
                Err(_) => return,
            };
            let now = Instant::now();
            inner
                .own_writes
                .retain(|_, at| now.duration_since(*at) < ECHO_WINDOW);
            let own: Vec<PathBuf> = inner.own_writes.keys().cloned().collect();
            // Suppress an event on, under, or above a path we just wrote: a
            // write echoes as the file itself and sometimes as its parent
            // directory. The "above" check can hide one external change in the
            // same directory within the echo window — the next burst re-fires.
            events
                .into_iter()
                .filter(|path| {
                    !own.iter().any(|written| {
                        path == written
                            || path.starts_with(written)
                            || written.starts_with(path)
                    })
                })
                .collect::<Vec<_>>()
        };
        if !external.is_empty() {
            let _ = app.emit("board-changed", ());
        }
    });

    Ok(BoardWatcher { _inner: watcher })
}
