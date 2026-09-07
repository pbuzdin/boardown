use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;

use crate::state::AppInner;

/// A lock wait or a repository on a slow mount must not leave a panel at
/// `Loading…` forever: past this the child is killed and the read answers
/// unavailable, like a git that could not be spawned at all.
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GitRunResult {
    Exited {
        code: i32,
        stdout: String,
        stderr: String,
    },
    Unavailable,
}

fn drain<R: Read + Send + 'static>(mut pipe: R) -> String {
    let mut bytes = Vec::new();
    let _ = pipe.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The host's whole share of the feature: run git in the project folder and
/// report what happened. Every decision about what the answer means lives in
/// `readTaskCommits` in core — the exit-code chain that separates "no
/// repository" from "an empty one" never appears here.
///
/// Async so the poll loop runs off the main thread; a slow git must not
/// freeze the window. Tauri requires async commands to answer Result, so the
/// command wraps the value — the renderer maps a spawn failure or an
/// unrecognized answer onto `unavailable`.
#[tauri::command]
pub async fn git_run(
    state: State<'_, Arc<Mutex<AppInner>>>,
    args: Vec<String>,
) -> Result<GitRunResult, String> {
    let cwd: Option<PathBuf> = {
        let inner = state.lock().ok();
        inner.as_ref().and_then(|guarded| guarded.root.clone())
    };
    let Some(cwd) = cwd else {
        return Ok(GitRunResult::Unavailable);
    };

    let mut command = Command::new("git");
    command
        .args(&args)
        .current_dir(&cwd)
        // Pinned so core reads git's own words rather than a translation of
        // them; nothing else about the environment is changed.
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: git must not flash a console window.
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return Ok(GitRunResult::Unavailable),
    };

    // Readers on threads so a chatty git cannot fill a pipe buffer and
    // deadlock the poll loop below.
    let stdout_pipe = child.stdout.take().expect("stdout was piped");
    let stderr_pipe = child.stderr.take().expect("stderr was piped");
    let stdout_task = thread::spawn(move || drain(stdout_pipe));
    let stderr_task = thread::spawn(move || drain(stderr_pipe));

    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(GitRunResult::Unavailable);
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(_) => return Ok(GitRunResult::Unavailable),
        }
    };

    let stdout = stdout_task.join().unwrap_or_default();
    let stderr = stderr_task.join().unwrap_or_default();
    match status.code() {
        Some(code) => Ok(GitRunResult::Exited {
            code,
            stdout,
            stderr,
        }),
        // No exit code (killed by a signal): we learned nothing about why.
        None => Ok(GitRunResult::Unavailable),
    }
}
