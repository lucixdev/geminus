/* This file is part of GEMINUS.
 *
 * Copyright (C) 2026 lucix.dev <lucix.dev@proton.me>
 *
 * GEMINUS is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * GEMINUS is distributed in the hope that it will be useful, but WITHOUT ANY
 * WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
 * details.
 *
 * You should have received a copy of the GNU General Public License along
 * with GEMINUS. If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

// File operations behind drag & drop: copy and move, single files and whole
// directory trees.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::CmdError;

const COPY_BUF_SIZE: usize = 64 * 1024;
const PROGRESS_EVERY_BYTES: u64 = 1024 * 1024;

static STOP_OP: AtomicBool = AtomicBool::new(false);

// Single-shot channel carrying the user's answer to a per-file failure: the
// copy thread parks on it while the frontend shows the dialog. One pending
// question at a time, because one operation runs at a time.
static PENDING_ERROR_TX: OnceLock<Mutex<Option<mpsc::Sender<ErrorChoice>>>> = OnceLock::new();

fn pending_error_tx() -> &'static Mutex<Option<mpsc::Sender<ErrorChoice>>> {
    PENDING_ERROR_TX.get_or_init(|| Mutex::new(None))
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorChoice {
    Skip,
    Retry,
    Abort,
}

// Only the position relative to the compared folder travels to the frontend,
// with forward slashes: that is the form the in-memory tree understands on
// either system. Absolute paths stay on this side.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpFileDone {
    src_rel: String,
    dst_rel: String,
    is_dir: bool,
    status: String, // "copied" | "created" | "skipped"
}

fn rel_child(rel: &str, name: &str) -> String {
    if rel.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", rel, name)
    }
}

// The backend classifies, the frontend translates: `phase` is what was being
// done, `cause` why it failed, `reason` the system's own raw text. No sentence
// for the user is composed here — it would not follow the language toggle.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpFileError {
    src_rel: String,
    dst_rel: String,
    is_dir: bool,
    phase: String,
    cause: String,
    reason: String,
}

// The same error numbers mean different things on different systems, so the
// classification lives behind the boundary, one per system.
use crate::sys::cause_of;

// One copy's failure. `phase` == "stopped" is the cancel sentinel, not a
// failure to show.
#[derive(Debug)]
struct OpErr {
    phase: &'static str,
    cause: &'static str,
    detail: String,
}

impl OpErr {
    fn stopped() -> Self {
        OpErr { phase: "stopped", cause: "stopped", detail: String::new() }
    }
    // The system's own words go along only when the cause was not recognised:
    // where it was, the sentence the user reads already says it, and repeating
    // it — in whatever language the system happens to speak — adds nothing.
    fn at(phase: &'static str, e: &std::io::Error) -> Self {
        let cause = cause_of(e);
        let detail = if cause == "other" { crate::system_text(e) } else { String::new() };
        OpErr { phase, cause, detail }
    }
}

// Emits op_file_error and blocks this thread until the frontend answers. If the
// channel dies with the window, the answer defaults to Abort.
fn ask_user_choice(
    app: &AppHandle,
    src_rel: &str,
    dst_rel: &str,
    is_dir: bool,
    phase: &str,
    cause: &str,
    reason: &str,
) -> ErrorChoice {
    let (tx, rx) = mpsc::channel::<ErrorChoice>();
    {
        let mut guard = pending_error_tx().lock().expect("pending_error_tx lock poisoned");
        *guard = Some(tx);
    }
    let _ = app.emit(
        "op_file_error",
        OpFileError {
            src_rel: src_rel.to_string(),
            dst_rel: dst_rel.to_string(),
            is_dir,
            phase: phase.to_string(),
            cause: cause.to_string(),
            reason: reason.to_string(),
        },
    );
    rx.recv().unwrap_or(ErrorChoice::Abort)
}

// Internal wiring: these failures never reach the user as a sentence — the
// dialog they answer is already gone when they happen.
#[tauri::command]
pub fn submit_error_choice(choice: ErrorChoice) -> Result<(), CmdError> {
    let mut guard = pending_error_tx()
        .lock()
        .map_err(|_| CmdError::plain("answer_not_delivered"))?;
    let tx = guard.take().ok_or_else(|| CmdError::plain("answer_not_delivered"))?;
    tx.send(choice).map_err(|_| CmdError::plain("answer_not_delivered"))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub status: String,
    pub final_dst: String,
    pub files_copied: u64,
    pub files_skipped: u64,
    pub symlinks_skipped: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpProgress {
    bytes_done: u64,
    bytes_total: u64,
    current: String,
    current_index: u64,
}

#[derive(Default)]
struct CopyStats {
    files_copied: u64,
    files_skipped: u64,
    symlinks_skipped: u64,
    current_index: u64,
}

#[tauri::command]
pub fn check_exists(root: String, rel: String) -> bool {
    crate::sys::join_rel(Path::new(&root), &rel).symlink_metadata().is_ok()
}

#[tauri::command]
pub fn stop_op() {
    STOP_OP.store(true, Ordering::SeqCst);
}

// A half-written file must never take the place of a good one: the copy goes to
// a temporary next to the destination and steps into place only once it is
// whole. Cancelling, failing or dying halfway leaves the destination as it was.
struct TempFile {
    path: PathBuf,
    armed: bool,
}

impl TempFile {
    fn new(path: PathBuf) -> Self {
        TempFile { path, armed: true }
    }
    // Called once the temporary has become the destination: there is nothing
    // left at that path to clean up.
    fn placed(&mut self) {
        self.armed = false;
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

// A copy the app did not survive leaves its working file behind, and the
// comparison skips those by name — which would make it invisible as well as
// useless, and a big one wastes real space. Clearing them out belongs to the
// next operation aimed at the same place: by then whoever wrote them is gone.
// Only other runs' files go; the ones this run is using carry its own name.
fn sweep_orphan_temps(dst_root: &Path) {
    let mine = format!("{}{}-", crate::diff::COPY_TEMP_PREFIX, std::process::id());
    let mut stack = vec![dst_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            let meta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                stack.push(path);
            } else if name.starts_with(crate::diff::COPY_TEMP_PREFIX) && !name.starts_with(&mine) {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

// One folder only: a single file is written in one place, and walking the whole
// destination for it would cost far more than it saves.
fn sweep_orphan_temps_shallow(dir: &Path) {
    let mine = format!("{}{}-", crate::diff::COPY_TEMP_PREFIX, std::process::id());
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with(crate::diff::COPY_TEMP_PREFIX) && !name.starts_with(&mine) {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

// Same directory as the destination, or the move onto it would cross a
// filesystem and stop being a single step.
fn temp_path_for(dst: &Path) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let name = format!(
        "{}{}-{}",
        crate::diff::COPY_TEMP_PREFIX,
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    match dst.parent() {
        Some(p) => p.join(name),
        None => PathBuf::from(name),
    }
}

// Moves the finished copy onto the destination, replacing whatever is there in
// one step. A write-protected destination refuses the move on Windows, so the
// protection comes off and the move is tried once more; if that fails too the
// error stands and nothing is replaced.
fn place(tmp: &Path, dst: &Path) -> Result<(), OpErr> {
    match fs::rename(tmp, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && dst.exists() => {
            crate::sys::clear_write_protection(dst).map_err(|_| OpErr::at("write", &e))?;
            fs::rename(tmp, dst).map_err(|e2| OpErr::at("write", &e2))
        }
        Err(e) => Err(OpErr::at("write", &e)),
    }
}

// Asks the user on failure (Skip/Retry/Abort) and announces every finished
// file, so the tree can follow the operation as it goes.
fn copy_file_one(
    app: &AppHandle,
    src: &Path,
    dst: &Path,
    src_rel: &str,
    dst_rel: &str,
    stats: &mut CopyStats,
) -> Result<bool, String> {
    loop {
        match try_copy_file_once(app, src, dst, src_rel, stats) {
            Ok(()) => {
                let _ = app.emit(
                    "op_file_done",
                    OpFileDone {
                        src_rel: src_rel.to_string(),
                        dst_rel: dst_rel.to_string(),
                        is_dir: false,
                        status: "copied".to_string(),
                    },
                );
                return Ok(true);
            }
            Err(e) if e.phase == "stopped" => return Err("stopped".to_string()),
            Err(e) => {
                let choice = ask_user_choice(
                    app,
                    src_rel,
                    dst_rel,
                    false,
                    e.phase,
                    e.cause,
                    &e.detail,
                );
                match choice {
                    ErrorChoice::Retry => continue,
                    ErrorChoice::Skip => {
                        stats.files_skipped += 1;
                        let _ = app.emit(
                            "op_file_done",
                            OpFileDone {
                                src_rel: src_rel.to_string(),
                                dst_rel: dst_rel.to_string(),
                                is_dir: false,
                                status: "skipped".to_string(),
                            },
                        );
                        return Ok(false);
                    }
                    ErrorChoice::Abort => {
                        STOP_OP.store(true, Ordering::SeqCst);
                        return Err("stopped".to_string());
                    }
                }
            }
        }
    }
}

// One file, start to finish, through a temporary that becomes the destination
// only when the whole file is there. Emits op_progress during streaming
// read/write. Preserves mtime and (on unix) permissions.
fn try_copy_file_once(
    app: &AppHandle,
    src: &Path,
    dst: &Path,
    src_rel: &str,
    stats: &mut CopyStats,
) -> Result<(), OpErr> {
    let src_meta = fs::metadata(src).map_err(|e| OpErr::at("read", &e))?;

    if let Some(parent) = dst.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| OpErr::at("mkdir", &e))?;
        }
    }

    let bytes_total = src_meta.len();
    let current_str = src_rel.to_string();
    stats.current_index += 1;
    let idx = stats.current_index;

    let mut reader = fs::File::open(src).map_err(|e| OpErr::at("read", &e))?;

    let tmp_path = temp_path_for(dst);
    let mut tmp = TempFile::new(tmp_path.clone());
    let mut writer = fs::File::create(&tmp_path).map_err(|e| OpErr::at("write", &e))?;

    let mut buf = vec![0u8; COPY_BUF_SIZE];
    let mut bytes_done: u64 = 0;
    let mut bytes_since_emit: u64 = 0;

    loop {
        if STOP_OP.load(Ordering::SeqCst) {
            return Err(OpErr::stopped());
        }
        let n = reader.read(&mut buf).map_err(|e| OpErr::at("read", &e))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .map_err(|e| OpErr::at("write", &e))?;
        bytes_done += n as u64;
        bytes_since_emit += n as u64;
        if bytes_since_emit >= PROGRESS_EVERY_BYTES {
            let _ = app.emit(
                "op_progress",
                OpProgress {
                    bytes_done,
                    bytes_total,
                    current: current_str.clone(),
                    current_index: idx,
                },
            );
            bytes_since_emit = 0;
        }
    }
    writer.flush().map_err(|e| OpErr::at("write", &e))?;

    if let Ok(mtime) = src_meta.modified() {
        let _ = writer.set_modified(mtime);
    }
    drop(writer);

    // A link at the destination is replaced, never written through: what it
    // points at can sit outside the compared folders. It goes now, one step
    // before the copy takes its place, so a failure earlier on leaves it alone.
    if let Ok(meta) = fs::symlink_metadata(dst) {
        if meta.file_type().is_symlink() {
            crate::sys::remove_link(dst, &meta).map_err(|e| OpErr::at("write", &e))?;
        }
    }

    place(&tmp_path, dst)?;
    tmp.placed();

    // A write-protected source gives a write-protected copy on both systems: on
    // Windows this carries the read-only attribute over. After the move, not
    // before — a file already marked read-only is one more thing that can stand
    // in the way of moving it.
    let _ = fs::set_permissions(dst, src_meta.permissions());

    let _ = app.emit(
        "op_progress",
        OpProgress {
            bytes_done,
            bytes_total,
            current: current_str.clone(),
            current_index: idx,
        },
    );

    stats.files_copied += 1;
    Ok(())
}

// Recursively copies a directory tree (merge semantics at each level).
// Symlinks are counted and skipped.
// Every failure asks the user (Skip/Retry/Abort), so this returns either Ok(())
// or Err("stopped") and never an unhandled error.
// Permissions and mtime of directories are applied AFTER contents are written,
// so a read-only source dir does not block writing its children.
fn copy_dir_recursive(
    app: &AppHandle,
    src: &Path,
    dst: &Path,
    src_rel: &str,
    dst_rel: &str,
    stats: &mut CopyStats,
) -> Result<(), String> {
    if STOP_OP.load(Ordering::SeqCst) {
        return Err("stopped".to_string());
    }

    // Same rule as for files: a link at the destination is replaced, not
    // followed. Merging into it would write inside the folder it points at, and
    // create_dir_all would not even notice — it follows the link and finds a
    // directory there.
    loop {
        let meta = match fs::symlink_metadata(dst) {
            Ok(m) if m.file_type().is_symlink() => m,
            _ => break,
        };
        match crate::sys::remove_link(dst, &meta) {
            Ok(()) => break,
            Err(e) => {
                let choice = ask_user_choice(
                    app,
                    src_rel,
                    dst_rel,
                    true,
                    "mkdir",
                    cause_of(&e),
                    &e.to_string(),
                );
                match choice {
                    ErrorChoice::Retry => continue,
                    ErrorChoice::Skip => {
                        stats.files_skipped += 1;
                        let _ = app.emit(
                            "op_file_done",
                            OpFileDone {
                                src_rel: src_rel.to_string(),
                                dst_rel: dst_rel.to_string(),
                                is_dir: true,
                                status: "skipped".to_string(),
                            },
                        );
                        return Ok(());
                    }
                    ErrorChoice::Abort => {
                        STOP_OP.store(true, Ordering::SeqCst);
                        return Err("stopped".to_string());
                    }
                }
            }
        }
    }

    // Whether this copy is the one that brings the folder into being decides,
    // further down, if its attributes may follow the source.
    let created = !dst.exists();

    // Create the destination folder, with a dialog on failure.
    if !dst.exists() {
        loop {
            match fs::create_dir_all(dst) {
                Ok(()) => break,
                Err(e) => {
                    let choice = ask_user_choice(
                        app,
                        src_rel,
                        dst_rel,
                        true,
                        "mkdir",
                        cause_of(&e),
                        &e.to_string(),
                    );
                    match choice {
                        ErrorChoice::Retry => continue,
                        ErrorChoice::Skip => {
                            stats.files_skipped += 1;
                            let _ = app.emit(
                                "op_file_done",
                                OpFileDone {
                                    src_rel: src_rel.to_string(),
                                    dst_rel: dst_rel.to_string(),
                                    is_dir: true,
                                    status: "skipped".to_string(),
                                },
                            );
                            return Ok(());
                        }
                        ErrorChoice::Abort => {
                            STOP_OP.store(true, Ordering::SeqCst);
                            return Err("stopped".to_string());
                        }
                    }
                }
            }
        }
    }

    // The folder exists now, so it is announced now and not at the end of the
    // recursion: if the operation stops halfway, the tree must still know it is
    // there. The status differs from "copied" on purpose — that one means the
    // whole folder went through.
    let _ = app.emit(
        "op_file_done",
        OpFileDone {
            src_rel: src_rel.to_string(),
            dst_rel: dst_rel.to_string(),
            is_dir: true,
            status: "created".to_string(),
        },
    );

    let copied_before = stats.files_copied;

    // Open the source folder, with a dialog on failure.
    let entries = loop {
        match fs::read_dir(src) {
            Ok(e) => break e,
            Err(err) => {
                let choice = ask_user_choice(
                    app,
                    src_rel,
                    dst_rel,
                    true,
                    "readdir",
                    cause_of(&err),
                    &err.to_string(),
                );
                match choice {
                    ErrorChoice::Retry => continue,
                    ErrorChoice::Skip => {
                        stats.files_skipped += 1;
                        let _ = app.emit(
                            "op_file_done",
                            OpFileDone {
                                src_rel: src_rel.to_string(),
                                dst_rel: dst_rel.to_string(),
                                is_dir: true,
                                status: "skipped".to_string(),
                            },
                        );
                        return Ok(());
                    }
                    ErrorChoice::Abort => {
                        STOP_OP.store(true, Ordering::SeqCst);
                        return Err("stopped".to_string());
                    }
                }
            }
        }
    };

    for entry_res in entries {
        if STOP_OP.load(Ordering::SeqCst) {
            return Err("stopped".to_string());
        }
        let entry = match entry_res {
            Ok(e) => e,
            Err(err) => {
                let choice = ask_user_choice(
                    app,
                    src_rel,
                    dst_rel,
                    true,
                    "readdir",
                    cause_of(&err),
                    &err.to_string(),
                );
                match choice {
                    // Retry cannot rewind a consumed iterator, so here it can
                    // only mean what Skip means.
                    ErrorChoice::Retry | ErrorChoice::Skip => {
                        stats.files_skipped += 1;
                        continue;
                    }
                    ErrorChoice::Abort => {
                        STOP_OP.store(true, Ordering::SeqCst);
                        return Err("stopped".to_string());
                    }
                }
            }
        };
        let child_src = entry.path();
        let name = entry.file_name();
        let child_dst = dst.join(&name);
        let name_str = name.to_string_lossy();
        let child_src_rel = rel_child(src_rel, &name_str);
        let child_dst_rel = rel_child(dst_rel, &name_str);

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(err) => {
                let choice = ask_user_choice(
                    app,
                    &child_src_rel,
                    &child_dst_rel,
                    false,
                    "read",
                    cause_of(&err),
                    &err.to_string(),
                );
                match choice {
                    ErrorChoice::Retry | ErrorChoice::Skip => {
                        stats.files_skipped += 1;
                        continue;
                    }
                    ErrorChoice::Abort => {
                        STOP_OP.store(true, Ordering::SeqCst);
                        return Err("stopped".to_string());
                    }
                }
            }
        };

        if file_type.is_symlink() {
            stats.symlinks_skipped += 1;
            continue;
        } else if file_type.is_dir() {
            if let Err(e) = copy_dir_recursive(
                app, &child_src, &child_dst, &child_src_rel, &child_dst_rel, stats,
            ) {
                if e == "stopped" {
                    return Err(e);
                }
            }
        } else if file_type.is_file() {
            match copy_file_one(app, &child_src, &child_dst, &child_src_rel, &child_dst_rel, stats) {
                Ok(_) => {}
                Err(e) if e == "stopped" => return Err(e),
                Err(_) => {}
            }
        } else {
            // Neither file, folder nor link (device, fifo, socket): skipped.
            stats.files_skipped += 1;
        }
    }

    // The folder's own attributes follow the source only when this copy made the
    // folder or actually put something inside it. A merge where everything was
    // skipped must not quietly unlock a folder the user had protected: nothing
    // arrived, so nothing about it changes.
    let touched = created || stats.files_copied > copied_before;
    if touched {
        if let Ok(src_meta) = fs::metadata(src) {
            #[cfg(unix)]
            {
                let perms = src_meta.permissions();
                let _ = fs::set_permissions(dst, perms);
            }
            if let Ok(mtime) = src_meta.modified() {
                if let Ok(f) = fs::File::open(dst) {
                    let _ = f.set_modified(mtime);
                }
            }
        }
    }

    let _ = app.emit(
        "op_file_done",
        OpFileDone {
            src_rel: src_rel.to_string(),
            dst_rel: dst_rel.to_string(),
            is_dir: true,
            status: "copied".to_string(),
        },
    );

    Ok(())
}

#[tauri::command]
pub async fn copy_item(
    app: AppHandle,
    src_root: String,
    dst_root: String,
    src_rel: String,
    dst_rel: String,
    move_mode: bool,
) -> Result<OpResult, CmdError> {
    STOP_OP.store(false, Ordering::SeqCst);

    let src_path = crate::sys::join_rel(Path::new(&src_root), &src_rel);
    let dst_path = crate::sys::join_rel(Path::new(&dst_root), &dst_rel);

    let src_meta = fs::symlink_metadata(&src_path)
        .map_err(|e| CmdError::from_io("source_unreadable", &e))?;

    if src_meta.file_type().is_symlink() {
        return Err(CmdError::plain("symlink_not_copyable"));
    }

    // Making room for a single file can create folders on the way, and a folder
    // nobody announced is one the view does not know about: it would sit on the
    // disk unseen until the next comparison. So they are created one level at a
    // time, each one announced as it appears.
    fn make_way(app: &AppHandle, dst_root: &Path, dst_rel: &str) -> Result<(), CmdError> {
        let parts: Vec<&str> = dst_rel.split('/').collect();
        if parts.len() < 2 {
            return Ok(());
        }
        let mut so_far = String::new();
        for part in &parts[..parts.len() - 1] {
            if !so_far.is_empty() {
                so_far.push('/');
            }
            so_far.push_str(part);
            let path = crate::sys::join_rel(dst_root, &so_far);
            if path.is_dir() {
                continue;
            }
            fs::create_dir(&path).map_err(|e| CmdError::from_io("folder_not_created", &e))?;
            let _ = app.emit(
                "op_file_done",
                OpFileDone {
                    src_rel: so_far.clone(),
                    dst_rel: so_far.clone(),
                    is_dir: true,
                    status: "created".to_string(),
                },
            );
        }
        Ok(())
    }

    if src_meta.is_file() {
        make_way(&app, Path::new(&dst_root), &dst_rel)?;
    }

    // Anything left behind by a copy that never finished goes now, before this
    // one starts writing in the same place.
    if src_meta.is_dir() {
        sweep_orphan_temps(&dst_path);
    } else if let Some(parent) = dst_path.parent() {
        sweep_orphan_temps_shallow(parent);
    }

    let mut stats = CopyStats::default();

    if src_meta.is_file() {
        match copy_file_one(&app, &src_path, &dst_path, &src_rel, &dst_rel, &mut stats) {
            Ok(true) => {
                if move_mode {
                    fs::remove_file(&src_path)
                        .map_err(|e| CmdError::from_io("move_source_not_removed", &e))?;
                }
                Ok(OpResult {
                    status: if move_mode { "moved".into() } else { "copied".into() },
                    final_dst: dst_path.to_string_lossy().into_owned(),
                    files_copied: stats.files_copied,
                    files_skipped: stats.files_skipped,
                    symlinks_skipped: stats.symlinks_skipped,
                })
            }
            Ok(false) => Ok(OpResult {
                status: "skipped".into(),
                final_dst: dst_path.to_string_lossy().into_owned(),
                files_copied: 0,
                files_skipped: stats.files_skipped,
                symlinks_skipped: 0,
            }),
            Err(_) => Ok(OpResult {
                status: "stopped".into(),
                final_dst: dst_path.to_string_lossy().into_owned(),
                files_copied: stats.files_copied,
                files_skipped: stats.files_skipped,
                symlinks_skipped: stats.symlinks_skipped,
            }),
        }
    } else if src_meta.is_dir() {
        match copy_dir_recursive(&app, &src_path, &dst_path, &src_rel, &dst_rel, &mut stats) {
            Ok(()) => {
                // Move-mode: the source folder goes only if nothing stayed
                // behind. Skipped symlinks count as much as skipped files: a
                // symlink is never copied, so removing the source would delete
                // the only copy of it.
                let left_behind = stats.files_skipped > 0 || stats.symlinks_skipped > 0;
                if move_mode && !left_behind {
                    fs::remove_dir_all(&src_path)
                        .map_err(|e| CmdError::from_io("move_source_not_removed", &e))?;
                }
                let status = if move_mode && !left_behind {
                    "moved"
                } else if move_mode || stats.files_skipped > 0 {
                    "partial"
                } else {
                    "copied"
                };
                Ok(OpResult {
                    status: status.into(),
                    final_dst: dst_path.to_string_lossy().into_owned(),
                    files_copied: stats.files_copied,
                    files_skipped: stats.files_skipped,
                    symlinks_skipped: stats.symlinks_skipped,
                })
            }
            Err(_) => Ok(OpResult {
                status: "stopped".into(),
                final_dst: dst_path.to_string_lossy().into_owned(),
                files_copied: stats.files_copied,
                files_skipped: stats.files_skipped,
                symlinks_skipped: stats.symlinks_skipped,
            }),
        }
    } else {
        Err(CmdError::plain("source_kind_not_supported"))
    }
}

// Permanent deletion of a file, a folder or a link. There is no recycle bin
// behind this: what goes, goes.
// Deleting a folder walks its contents and can stop halfway, with part of it
// already gone. Saying only "it failed" would leave the view showing a folder
// that is no longer there in full, so what is still on the disk comes back with
// the answer and the view is rebuilt from that.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    pub removed: bool,
    pub remaining: Vec<String>,
    pub unreadable: Vec<String>,
    pub error: Option<CmdError>,
}

// What is still on the disk under `rel`, itself included, in the shared form —
// and separately the folders that could not be looked into at all. A folder
// that refuses to be read says nothing about what it holds, and treating that
// silence as "gone" would erase from the view files that are still there.
struct Surviving {
    seen: Vec<String>,
    unknown: Vec<String>,
}

fn surviving_below(root: &Path, rel: &str) -> Surviving {
    let base = crate::sys::join_rel(root, rel);
    let mut seen: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    if fs::symlink_metadata(&base).is_err() {
        return Surviving { seen, unknown };
    }
    seen.push(rel.to_string());
    let mut stack = vec![(base, rel.to_string())];
    while let Some((dir, dir_rel)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => {
                unknown.push(dir_rel);
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let child_rel = match path.strip_prefix(root) {
                Ok(r) => crate::sys::rel_from_os(r),
                Err(_) => continue,
            };
            seen.push(child_rel.clone());
            if meta.is_dir() {
                stack.push((path, child_rel));
            }
        }
    }
    Surviving { seen, unknown }
}

#[tauri::command]
pub fn delete_item(root: String, rel: String) -> Result<DeleteOutcome, CmdError> {
    let root_path = PathBuf::from(&root);
    let p = crate::sys::join_rel(&root_path, &rel);
    let p = p.as_path();

    // symlink_metadata does not follow the link, so a link — dangling or not —
    // is removed as a link and never as what it points at.
    let meta = match fs::symlink_metadata(p) {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(CmdError::from_io("permission", &e));
        }
        Err(_) => {
            // Already gone: the view still has to lose it, so this is an
            // outcome with a message, not a refusal.
            return Ok(DeleteOutcome {
                removed: true,
                remaining: Vec::new(),
                unreadable: Vec::new(),
                error: Some(CmdError::plain("item_missing")),
            });
        }
        Ok(m) => m,
    };

    // is_dir() is false for a link, even one pointing at a directory. Removing
    // it happens behind the boundary, because how you remove it depends on the
    // system — and on neither system is the target touched.
    let result = if meta.file_type().is_symlink() {
        crate::sys::remove_link(p, &meta)
    } else if meta.is_dir() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    };

    match result {
        Ok(()) => Ok(DeleteOutcome {
            removed: true,
            remaining: Vec::new(),
            unreadable: Vec::new(),
            error: None,
        }),
        Err(e) => {
            let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
                "permission"
            } else {
                "delete_failed"
            };
            let survived = surviving_below(&root_path, &rel);
            Ok(DeleteOutcome {
                removed: survived.seen.is_empty(),
                remaining: survived.seen,
                unreadable: survived.unknown,
                error: Some(CmdError::from_io(code, &e)),
            })
        }
    }
}

// Hands an item to the program the system associates with it. How you ask is
// system business, so it lives behind the boundary.
#[tauri::command]
pub fn open_item(root: String, rel: String) -> Result<(), CmdError> {
    let p = crate::sys::join_rel(Path::new(&root), &rel);
    let p = p.as_path();

    match fs::symlink_metadata(p) {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(CmdError::from_io("permission", &e));
        }
        Err(_) => {
            return Err(CmdError::plain("item_missing"));
        }
        Ok(_) => {}
    }

    crate::sys::open_with_default(p)
}

// Everything the picker needs to draw a folder: where you are, what is above,
// the trail behind, the contents. The frontend neither splits nor builds paths:
// their shape belongs to the system.
#[tauri::command]
pub fn browse_dir(path: String) -> Result<serde_json::Value, CmdError> {
    let p = std::path::Path::new(&path);
    let mut entries: Vec<serde_json::Value> = Vec::new();
    let rd = std::fs::read_dir(p).map_err(|e| CmdError::from_io("folder_not_readable", &e))?;
    for entry in rd.flatten() {
        let ep = entry.path();
        let meta = ep.symlink_metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let is_sym = meta.as_ref().map(|m| m.file_type().is_symlink()).unwrap_or(false);
        let is_hidden = meta.as_ref().map(|m| crate::sys::is_hidden(&ep, m)).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().to_string();
        entries.push(serde_json::json!({
            "name": name,
            "path": ep.to_string_lossy().to_string(),
            "is_dir": is_dir,
            "is_symlink": is_sym,
            "is_hidden": is_hidden
        }));
    }
    // Folders first, then by name ignoring case: the order the user sees in
    // their own file manager, not the byte order.
    entries.sort_by(|a, b| {
        let ad = a["is_dir"].as_bool().unwrap_or(false);
        let bd = b["is_dir"].as_bool().unwrap_or(false);
        let an = a["name"].as_str().unwrap_or("");
        let bn = b["name"].as_str().unwrap_or("");
        bd.cmp(&ad)
            .then(an.to_lowercase().cmp(&bn.to_lowercase()))
            .then(an.cmp(bn))
    });

    let crumbs: Vec<serde_json::Value> = crate::sys::crumbs(p)
        .into_iter()
        .map(|(label, path)| serde_json::json!({ "label": label, "path": path }))
        .collect();

    Ok(serde_json::json!({
        "path": path,
        "parent": crate::sys::parent_of(p),
        "crumbs": crumbs,
        "entries": entries
    }))
}

#[tauri::command]
pub fn get_home_dir() -> String {
    crate::sys::home_dir()
}

// None where there is no single root: the button that leads there hides.
#[tauri::command]
pub fn get_root_path() -> Option<String> {
    crate::sys::root_path()
}

#[tauri::command]
pub fn list_devices() -> Result<Vec<serde_json::Value>, CmdError> {
    let devices = crate::sys::list_devices()?;
    Ok(devices
        .into_iter()
        .map(|(name, path)| serde_json::json!({
            "name": name,
            "path": path,
            "is_dir": true
        }))
        .collect())
}

// Which disk carries a user path. How you ask depends on the system; here we
// only ask.
#[tauri::command]
pub fn get_device_for_path(path: String) -> Result<String, CmdError> {
    crate::sys::device_for_path(&path)
}

// Opens a page in the user's browser. Web addresses only: whatever is handed to
// the system leaves the app, and a path or a command dressed as an address must
// not get through here.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), CmdError> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(CmdError::plain("not_a_web_address"));
    }
    crate::sys::open_url(&url)
}

// The pieces of the copy that can be checked without a window in front of
// them: where the working file goes, what happens to it when the copy does not
// finish, and what a half-done deletion reports back.
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    fn scratch(tag: &str) -> PathBuf {
        let mut h = RandomState::new().build_hasher();
        h.write_usize(std::process::id() as usize);
        let dir = std::env::temp_dir().join(format!("geminus-test-{}-{:x}", tag, h.finish()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_working_file_sits_beside_its_destination() {
        let dir = scratch("beside");
        let dst = dir.join("sub").join("file.txt");
        let tmp = temp_path_for(&dst);
        assert_eq!(tmp.parent(), dst.parent());
        assert!(tmp.file_name().unwrap().to_str().unwrap().starts_with(crate::diff::COPY_TEMP_PREFIX));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_working_files_never_share_a_name() {
        let dir = scratch("unique");
        let dst = dir.join("file.txt");
        assert_ne!(temp_path_for(&dst), temp_path_for(&dst));
        let _ = fs::remove_dir_all(&dir);
    }

    // The point of the whole thing: give up halfway and the destination is
    // untouched, with nothing left lying around.
    #[test]
    fn giving_up_leaves_the_destination_alone() {
        let dir = scratch("giveup");
        let dst = dir.join("backup.txt");
        fs::write(&dst, b"the old one").unwrap();
        {
            let tmp_path = temp_path_for(&dst);
            let mut guard = TempFile::new(tmp_path.clone());
            fs::write(&tmp_path, b"half of the new one").unwrap();
            assert!(tmp_path.exists());
            guard.placed();
            guard.armed = true; // as if the copy had failed instead
        }
        assert_eq!(fs::read(&dst).unwrap(), b"the old one");
        let leftovers: Vec<_> = fs::read_dir(&dir).unwrap().flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(crate::diff::COPY_TEMP_PREFIX))
            .collect();
        assert!(leftovers.is_empty(), "a working file was left behind");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_finished_copy_takes_the_place_of_the_old_one() {
        let dir = scratch("place");
        let dst = dir.join("backup.txt");
        fs::write(&dst, b"the old one").unwrap();
        let tmp_path = temp_path_for(&dst);
        let mut guard = TempFile::new(tmp_path.clone());
        fs::write(&tmp_path, b"the new one").unwrap();
        place(&tmp_path, &dst).unwrap();
        guard.placed();
        assert_eq!(fs::read(&dst).unwrap(), b"the new one");
        assert!(!tmp_path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    // A write-protected file in the backup is the normal case, not the odd one.
    #[test]
    fn a_protected_destination_is_replaced_all_the_same() {
        let dir = scratch("protected");
        let dst = dir.join("locked.txt");
        fs::write(&dst, b"old").unwrap();
        let mut perms = fs::metadata(&dst).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&dst, perms).unwrap();
        let tmp_path = temp_path_for(&dst);
        let mut guard = TempFile::new(tmp_path.clone());
        fs::write(&tmp_path, b"new").unwrap();
        place(&tmp_path, &dst).expect("a locked destination must not stop the copy");
        guard.placed();
        assert_eq!(fs::read(&dst).unwrap(), b"new");
        let mut perms = fs::metadata(&dst).unwrap().permissions();
        perms.set_readonly(false);
        let _ = fs::set_permissions(&dst, perms);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn another_runs_leftovers_go_but_ours_stay() {
        let dir = scratch("sweep");
        let mine = dir.join(format!("{}{}-7", crate::diff::COPY_TEMP_PREFIX, std::process::id()));
        let theirs = dir.join(format!("{}999999-1", crate::diff::COPY_TEMP_PREFIX));
        let real = dir.join("documento.txt");
        for p in [&mine, &theirs, &real] {
            fs::write(p, b"x").unwrap();
        }
        sweep_orphan_temps(&dir);
        assert!(mine.exists(), "a file this run is using was removed");
        assert!(!theirs.exists(), "a leftover from a dead run stayed");
        assert!(real.exists(), "a real file was removed");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn leftovers_are_swept_from_below_too() {
        let dir = scratch("sweep-deep");
        let deep = dir.join("a").join("b");
        fs::create_dir_all(&deep).unwrap();
        let theirs = deep.join(format!("{}999999-2", crate::diff::COPY_TEMP_PREFIX));
        fs::write(&theirs, b"x").unwrap();
        sweep_orphan_temps(&dir);
        assert!(!theirs.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    // A deletion that stops halfway has to say what is still there, and admit
    // which folders it could not look into at all.
    #[test]
    fn what_survives_a_half_deletion_is_reported() {
        let dir = scratch("survive");
        fs::create_dir_all(dir.join("top").join("kept")).unwrap();
        fs::write(dir.join("top").join("kept").join("f.txt"), b"x").unwrap();
        let s = surviving_below(&dir, "top");
        assert!(s.seen.contains(&"top".to_string()));
        assert!(s.seen.contains(&"top/kept".to_string()));
        assert!(s.seen.contains(&"top/kept/f.txt".to_string()));
        assert!(s.unknown.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_refuses_to_be_read_is_declared_unknown() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("blind");
        let shut = dir.join("top").join("shut");
        fs::create_dir_all(&shut).unwrap();
        fs::write(shut.join("hidden.txt"), b"x").unwrap();
        fs::set_permissions(&shut, fs::Permissions::from_mode(0o000)).unwrap();
        let s = surviving_below(&dir, "top");
        assert!(s.seen.contains(&"top/shut".to_string()));
        assert!(!s.seen.contains(&"top/shut/hidden.txt".to_string()));
        assert!(s.unknown.contains(&"top/shut".to_string()),
            "silence from a folder must not pass for absence");
        fs::set_permissions(&shut, fs::Permissions::from_mode(0o700)).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn gone_means_nothing_is_reported() {
        let dir = scratch("gone");
        let s = surviving_below(&dir, "never-existed");
        assert!(s.seen.is_empty() && s.unknown.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
