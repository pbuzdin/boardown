use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::watch::BoardWatcher;

/// The board context of the single window: the project folder the user picked
/// (the one holding `.boardown/`), plus the pieces the commands and the
/// watcher share.
pub struct AppInner {
    pub root: Option<PathBuf>,
    /// Absolute paths this process wrote itself, with the write time, so the
    /// watcher can drop the echo of our own writes.
    pub own_writes: HashMap<PathBuf, Instant>,
    /// Kept alive while a board is open; dropped on close or board switch,
    /// which stops the watcher and ends its flusher thread.
    pub watcher: Option<BoardWatcher>,
}

impl Default for AppInner {
    fn default() -> Self {
        Self {
            root: None,
            own_writes: HashMap::new(),
            watcher: None,
        }
    }
}
