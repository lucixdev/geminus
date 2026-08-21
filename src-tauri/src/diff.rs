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

// Tree diff: compares two directory trees using a three-level algorithm.
//   Level 1 — structure: presence/absence of each relative path.
//   Level 2 — metadata: file size and modification time (with tolerance).
//   Level 3 — hash: blake3 streaming, only for candidates matching at L2.
// Errors during scan or hash never abort: they are logged and counted.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

const MTIME_TOLERANCE_SEC: i64 = 2;
const HASH_BUF_SIZE: usize = 64 * 1024;
const PROGRESS_EVERY_SCAN: u64 = 500;
const PROGRESS_EVERY_L2: u64 = 500;
const PROGRESS_EVERY_L3: u64 = 50;

// Hash timeout proporzionale: 30s base + dimensione/5MB/s (soglia minima USB 2.0).
// Two-hour cap: a guard against a corrupt size, not a realistic limit.
const HASH_MIN_SPEED_BPS: u64 = 5 * 1024 * 1024;
const HASH_BASE_TIMEOUT_SEC: u64 = 30;
const HASH_MAX_TIMEOUT_SEC: u64 = 2 * 60 * 60;

// Stall watchdog: when the reading thread stops advancing its counter for
// longer than this, the disk is stuck and that file is given up on.
const HASH_STALL_LIMIT_SEC: u64 = 15;
const HASH_WATCHDOG_POLL_MS: u64 = 500;

// Soft stall: after this much silence the overlay says the disk is not
// answering, re-emitted so the window does not look frozen.
const HASH_STALL_NOTIFY_SEC: u64 = 5;
const HASH_STALL_REEMIT_SEC: u64 = 2;

// Inside one huge file the bar would sit still for minutes: progress is emitted
// on this interval so it keeps moving.
const HASH_PROGRESS_EMIT_SEC: u64 = 2;

// Budget of leaked threads — reads that never came back. Past it the disk
// counts as compromised: the content phase stops and what is left is marked
// unreadable rather than waited on one file at a time.
const MAX_LEAKED_THREADS: u64 = 8;

static STOP: AtomicBool = AtomicBool::new(false);
static LEAKED_THREADS: AtomicU64 = AtomicU64::new(0);

// Shared with the copy: the name it gives a file still being written.
pub const COPY_TEMP_PREFIX: &str = ".geminus-part-";

const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".rustup",
    ".cargo",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    ".cache",
    "Trash",
];

#[derive(Debug, Clone, Copy)]
struct ScannedFile {
    is_dir: bool,
    is_symlink: bool,
    hidden: bool,
    size: u64,
    modified: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum DiffStatus {
    Same,
    Diff,
    OnlyA,
    OnlyB,
    Unreadable,
}

#[derive(Debug)]
enum HashError {
    Failed,
    Timeout,
}

// The comparison the frontend asked for.
//   Fast — structure and metadata only, no content read.
//   Deep — content too, for whatever survived the metadata check.
// "Full" never arrives here: it is Deep plus the health check, and orchestrating
// the two belongs to the frontend.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompareMode {
    Fast,
    Deep,
}

fn mode_to_str(mode: CompareMode) -> &'static str {
    match mode {
        CompareMode::Fast => "fast",
        CompareMode::Deep => "deep",
    }
}

#[derive(Debug, Clone, Serialize)]
struct DiffNode {
    n: String,
    t: char,
    s: DiffStatus,
    #[serde(rename = "sA")]
    size_a: Option<u64>,
    #[serde(rename = "sB")]
    size_b: Option<u64>,
    #[serde(rename = "linkA")]
    link_a: bool,
    #[serde(rename = "linkB")]
    link_b: bool,
    h: bool,
    ch: Vec<DiffNode>,
}

#[derive(Debug, Clone, Serialize)]
struct CompareProgress {
    phase: String,
    count: u64,
    total: u64,
    #[serde(rename = "currentPath")]
    current_path: String,
    #[serde(rename = "bytesDone")]
    bytes_done: u64,
    #[serde(rename = "bytesTotal")]
    bytes_total: u64,
}

#[derive(Debug, Clone, Serialize)]
struct CompareComplete {
    tree: Vec<DiffNode>,
    #[serde(rename = "totalA")]
    total_a: u64,
    #[serde(rename = "totalB")]
    total_b: u64,
    errors: u64,
    unreadable: u64,
    excluded: u64,
    aborted: u64,
    mode: String,
}

#[derive(Debug, Clone, Serialize)]
struct CompareStopped {
    phase: String,
}

#[tauri::command]
pub async fn start_compare(
    app: AppHandle,
    root_a: String,
    root_b: String,
    mode: CompareMode,
) -> Result<(), crate::CmdError> {
    let path_a = PathBuf::from(&root_a);
    let path_b = PathBuf::from(&root_b);

    if !path_a.is_dir() {
        return Err(crate::CmdError::with_detail("path_not_a_folder", root_a));
    }
    if !path_b.is_dir() {
        return Err(crate::CmdError::with_detail("path_not_a_folder", root_b));
    }

    // Two different devices can be read at once; one device cannot, or its head
    // spends the time seeking. Which device a path sits on is system business —
    // here the two answers are only compared.
    let dev_a = crate::sys::device_id(&path_a);
    let dev_b = crate::sys::device_id(&path_b);
    let same_device = match (&dev_a, &dev_b) {
        (Some(a), Some(b)) => a == b,
        _ => true, // unknown: treat as one device, the safe assumption
    };
    log::info!(
        "Compare devices: A={:?} B={:?} same_device={}",
        dev_a, dev_b, same_device
    );

    STOP.store(false, Ordering::Relaxed);
    LEAKED_THREADS.store(0, Ordering::Relaxed);

    tauri::async_runtime::spawn_blocking(move || {
        compare_paths(&app, &path_a, &path_b, same_device, mode);
    });

    Ok(())
}

#[tauri::command]
pub fn stop_compare() {
    STOP.store(true, Ordering::Relaxed);
}

fn compare_paths(app: &AppHandle, root_a: &Path, root_b: &Path, same_device: bool, mode: CompareMode) {
    let mut errors: u64 = 0;
    let mut unreadable: u64 = 0;
    let mut aborted: u64 = 0;

    let mut excluded: u64 = 0;

    let map_a = scan_tree(app, root_a, "scan_a", &mut errors, &mut excluded);
    if STOP.load(Ordering::Relaxed) {
        emit_stopped(app, "scan_a");
        return;
    }

    let map_b = scan_tree(app, root_b, "scan_b", &mut errors, &mut excluded);
    if STOP.load(Ordering::Relaxed) {
        emit_stopped(app, "scan_b");
        return;
    }

    let total_a = map_a.len() as u64;
    let total_b = map_b.len() as u64;

    let status_map = compute_diff(app, root_a, root_b, &map_a, &map_b, &mut unreadable, &mut aborted, same_device, mode);
    if STOP.load(Ordering::Relaxed) {
        emit_stopped(app, "diff");
        return;
    }

    let tree = build_tree(&map_a, &map_b, &status_map);

    let _ = app.emit(
        "compare_complete",
        CompareComplete {
            tree,
            total_a,
            total_b,
            errors,
            unreadable,
            excluded,
            aborted,
            mode: mode_to_str(mode).to_string(),
        },
    );
}

fn emit_stopped(app: &AppHandle, phase: &str) {
    let _ = app.emit(
        "compare_stopped",
        CompareStopped { phase: phase.to_string() },
    );
}

fn scan_tree(
    app: &AppHandle,
    root: &Path,
    phase: &str,
    errors: &mut u64,
    excluded: &mut u64,
) -> BTreeMap<String, ScannedFile> {
    let mut map = BTreeMap::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    let mut count: u64 = 0;

    while let Some(current_dir) = stack.pop() {
        if STOP.load(Ordering::Relaxed) {
            return map;
        }
        let entries = match std::fs::read_dir(&current_dir) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Cannot read directory {}: {}", current_dir.display(), e);
                *errors += 1;
                continue;
            }
        };

        for entry_result in entries {
            if STOP.load(Ordering::Relaxed) {
                return map;
            }
            let entry = match entry_result {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("Cannot read entry: {}", e);
                    *errors += 1;
                    continue;
                }
            };

            let path = entry.path();

            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    log::warn!("Cannot read metadata for {}: {}", path.display(), e);
                    *errors += 1;
                    continue;
                }
            };

            let is_dir = metadata.is_dir();
            let is_symlink = metadata.file_type().is_symlink();
            let hidden = crate::sys::is_hidden(&path, &metadata);

            // A copy in flight writes next to its destination under a name of
            // ours. One left behind by a kill is our litter, not a difference
            // between the two sides.
            if !is_dir {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with(COPY_TEMP_PREFIX) {
                    continue;
                }
            }

            // Skip excluded directories: not indexed, not descended into
            if is_dir {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if is_excluded_dir(dir_name) {
                    *excluded += 1;
                    continue;
                }
            }

            let size = if is_dir { 0 } else { metadata.len() };
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            // The relative position is born here and normalized here: from this
            // point on it carries forward slashes on either system.
            let rel = match path.strip_prefix(root) {
                Ok(r) => crate::sys::rel_from_os(r),
                Err(_) => continue,
            };

            if !rel.is_empty() {
                map.insert(
                    rel,
                    ScannedFile {
                        is_dir,
                        is_symlink,
                        hidden,
                        size,
                        modified,
                    },
                );
                count += 1;

                if count % PROGRESS_EVERY_SCAN == 0 {
                    let _ = app.emit(
                        "compare_progress",
                        CompareProgress {
                            phase: phase.to_string(),
                            count,
                            total: 0,
                            current_path: path.to_string_lossy().into_owned(),
                            bytes_done: 0,
                            bytes_total: 0,
                        },
                    );
                }
            }

            if is_dir {
                stack.push(path);
            }
        }
    }

    let _ = app.emit(
        "compare_progress",
        CompareProgress {
            phase: phase.to_string(),
            count,
            total: count,
            current_path: root.to_string_lossy().into_owned(),
            bytes_done: 0,
            bytes_total: 0,
        },
    );

    map
}

// Two links are the same link when they carry the same target, read without
// following it: a pair of dangling links pointing at the same missing file is a
// matching pair, not a difference.
fn link_targets_match(root_a: &Path, root_b: &Path, rel: &str) -> bool {
    let a = std::fs::read_link(crate::sys::join_rel(root_a, rel));
    let b = std::fs::read_link(crate::sys::join_rel(root_b, rel));
    match (a, b) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn compute_diff(
    app: &AppHandle,
    root_a: &Path,
    root_b: &Path,
    map_a: &BTreeMap<String, ScannedFile>,
    map_b: &BTreeMap<String, ScannedFile>,
    unreadable: &mut u64,
    aborted: &mut u64,
    same_device: bool,
    mode: CompareMode,
) -> BTreeMap<String, DiffStatus> {
    let mut result: BTreeMap<String, DiffStatus> = BTreeMap::new();
    let mut hash_candidates: Vec<String> = Vec::new();
    let mut l2_count: u64 = 0;
    let l2_total = map_a.len().min(map_b.len()) as u64;

    // Levels 1 + 2 in a single pass over map_a
    for (key, a) in map_a {
        if STOP.load(Ordering::Relaxed) {
            return result;
        }
        match map_b.get(key) {
            None => {
                result.insert(key.clone(), DiffStatus::OnlyA);
            }
            Some(b) => {
                let status = if a.is_symlink || b.is_symlink {
                    // A link is compared as a link: same kind on both sides and
                    // the same target written in it. What it points at is never
                    // read — it can sit outside the compared folders, and its
                    // size and time are the link's own, not the target's.
                    let same_link = a.is_symlink
                        && b.is_symlink
                        && link_targets_match(root_a, root_b, key);
                    Some(if same_link { DiffStatus::Same } else { DiffStatus::Diff })
                } else if a.is_dir != b.is_dir {
                    Some(DiffStatus::Diff)
                } else if a.is_dir {
                    Some(DiffStatus::Same)
                } else if a.size != b.size {
                    Some(DiffStatus::Diff)
                } else if (a.modified - b.modified).abs() > MTIME_TOLERANCE_SEC {
                    Some(DiffStatus::Diff)
                } else {
                    None
                };

                match status {
                    Some(s) => {
                        result.insert(key.clone(), s);
                    }
                    None => {
                        hash_candidates.push(key.clone());
                    }
                }

                l2_count += 1;
                if l2_count % PROGRESS_EVERY_L2 == 0 {
                    let _ = app.emit(
                        "compare_progress",
                        CompareProgress {
                            phase: "diff_l2".to_string(),
                            count: l2_count,
                            total: l2_total,
                            current_path: key.clone(),
                            bytes_done: 0,
                            bytes_total: 0,
                        },
                    );
                }
            }
        }
    }

    // Level 1 — only-B (paths in B not present in A)
    for key in map_b.keys() {
        if STOP.load(Ordering::Relaxed) {
            return result;
        }
        if !map_a.contains_key(key) {
            result.insert(key.clone(), DiffStatus::OnlyB);
        }
    }

    // Fast: no content is read, so whatever matched on size and time counts as
    // equal. The limit is real and the user is told about it — two files with
    // the same size and date but different content are called equal here.
    if matches!(mode, CompareMode::Fast) {
        for key in &hash_candidates {
            result.insert(key.clone(), DiffStatus::Same);
        }
        return result;
    }

    // Level 3 — blake3 hash on candidates
    let l3_total = hash_candidates.len() as u64;

    // Bytes to read in the content phase, doubled because every candidate is
    // read on both sides.
    let total_bytes_l3: u64 = hash_candidates.iter()
        .map(|k| map_a.get(k).map(|s| s.size).unwrap_or(0))
        .sum::<u64>()
        .saturating_mul(2);
    let cumulative_bytes = Arc::new(AtomicU64::new(0));
    for (i, key) in hash_candidates.iter().enumerate() {
        if STOP.load(Ordering::Relaxed) {
            break;
        }
        // Budget spent: the disk is not answering. Mark the rest unreadable
        // instead of hanging on it file after file.
        if LEAKED_THREADS.load(Ordering::Relaxed) >= MAX_LEAKED_THREADS {
            log::warn!(
                "Hash phase aborted at file {}/{}: leaked thread budget ({}) exceeded",
                i, hash_candidates.len(), MAX_LEAKED_THREADS
            );
            for remaining_key in &hash_candidates[i..] {
                result.insert(remaining_key.clone(), DiffStatus::Unreadable);
                *aborted += 1;
            }
            break;
        }
        let path_a = crate::sys::join_rel(root_a, key);
        let path_b = crate::sys::join_rel(root_b, key);

        // The sizes matched already, so either side sizes the timeout.
        let size = map_a.get(key).map(|s| s.size).unwrap_or(0);

        // Different devices are read in parallel; the same device is read one
        // side at a time, or the head thrashes between two places.
        let (h_a, h_b) = if same_device {
            let h_a = hash_file(app, &path_a, size, &cumulative_bytes, total_bytes_l3);
            if STOP.load(Ordering::Relaxed) { break; }
            let h_b = hash_file(app, &path_b, size, &cumulative_bytes, total_bytes_l3);
            (h_a, h_b)
        } else {
            std::thread::scope(|s| {
                let ja = s.spawn(|| hash_file(app, &path_a, size, &cumulative_bytes, total_bytes_l3));
                let jb = s.spawn(|| hash_file(app, &path_b, size, &cumulative_bytes, total_bytes_l3));
                let ra = ja.join().unwrap_or(Err(HashError::Failed));
                let rb = jb.join().unwrap_or(Err(HashError::Failed));
                (ra, rb)
            })
        };
        if STOP.load(Ordering::Relaxed) { break; }

        let status = match (h_a, h_b) {
            (Ok(h_a), Ok(h_b)) => {
                if h_a == h_b {
                    DiffStatus::Same
                } else {
                    DiffStatus::Diff
                }
            }
            // Whatever the disk refused — timeout, permissions, physical error —
            // the content could not be read, and a file that cannot be read is
            // not a file that changed. Calling it Diff hid it from the ⚠ filter
            // and showed a backup as out of line when it was not.
            _ => {
                *unreadable += 1;
                DiffStatus::Unreadable
            }
        };

        result.insert(key.clone(), status);

        if ((i as u64) + 1) % PROGRESS_EVERY_L3 == 0 {
            let _ = app.emit(
                "compare_progress",
                CompareProgress {
                    phase: "diff_l3".to_string(),
                    count: (i as u64) + 1,
                    total: l3_total,
                    current_path: key.clone(),
                    bytes_done: cumulative_bytes.load(Ordering::Relaxed),
                    bytes_total: total_bytes_l3,
                },
            );
        }
    }

    // Final emit for L3: covers cases where l3_total < PROGRESS_EVERY_L3
    // and ensures the overlay shows the hash phase even on small datasets.
    if l3_total > 0 {
        let last_key = hash_candidates.last().cloned().unwrap_or_default();
        let _ = app.emit(
            "compare_progress",
            CompareProgress {
                phase: "diff_l3".to_string(),
                count: l3_total,
                total: l3_total,
                current_path: last_key,
                bytes_done: cumulative_bytes.load(Ordering::Relaxed),
                bytes_total: total_bytes_l3,
            },
        );
    }

    result
}

fn hash_file(
    app: &AppHandle,
    path: &Path,
    size: u64,
    cumulative_bytes: &Arc<AtomicU64>,
    bytes_total: u64,
) -> Result<blake3::Hash, HashError> {
    // Probe: open and read one byte in a separate thread. If the disk hangs on
    // the open, that thread stays stuck and this one moves on — a leaked thread
    // is the accepted price for not freezing the whole app.
    let (tx_probe, rx_probe) = mpsc::channel();
    let probe_path = path.to_path_buf();
    std::thread::spawn(move || {
        let result = (|| -> Result<(), ()> {
            let mut file = std::fs::File::open(&probe_path).map_err(|_| ())?;
            let mut one = [0u8; 1];
            file.read(&mut one).map_err(|_| ())?;
            Ok(())
        })();
        let _ = tx_probe.send(result);
    });

    match rx_probe.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => {}
        Ok(Err(())) => {
            log::warn!("Probe failed for {}", path.display());
            return Err(HashError::Failed);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            log::warn!("Probe timeout (10s) for {}", path.display());
            LEAKED_THREADS.fetch_add(1, Ordering::Relaxed);
            return Err(HashError::Timeout);
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            log::warn!("Probe thread disconnected for {}", path.display());
            return Err(HashError::Failed);
        }
    }

    // The reading thread bumps a counter after every successful read, and this
    // one watches two things: a total timeout sized on the file, and a counter
    // that stopped moving. Either way the child stays stuck in read() and this
    // thread moves on — the same accepted leak as the probe.
    let progress_count = Arc::new(AtomicU64::new(0));
    let progress_writer = progress_count.clone();
    let cumulative_writer = cumulative_bytes.clone();
    let (tx_hash, rx_hash) = mpsc::channel();
    let hash_path = path.to_path_buf();

    std::thread::spawn(move || {
        let result = (|| -> Result<blake3::Hash, HashError> {
            let mut file = std::fs::File::open(&hash_path).map_err(|e| {
                log::warn!("Cannot open {} for hashing: {}", hash_path.display(), e);
                HashError::Failed
            })?;
            let mut hasher = blake3::Hasher::new();
            let mut buf = vec![0u8; HASH_BUF_SIZE];
            loop {
                if STOP.load(Ordering::Relaxed) {
                    return Err(HashError::Failed);
                }
                let n = match file.read(&mut buf) {
                    Ok(n) => n,
                    Err(e) => {
                        log::warn!("Read error on {}: {}", hash_path.display(), e);
                        return Err(HashError::Failed);
                    }
                };
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                progress_writer.fetch_add(1, Ordering::Relaxed);
                cumulative_writer.fetch_add(n as u64, Ordering::Relaxed);
            }
            Ok(hasher.finalize())
        })();
        let _ = tx_hash.send(result);
    });

    let total_timeout = compute_hash_timeout(size);
    let stall_limit = Duration::from_secs(HASH_STALL_LIMIT_SEC);
    let stall_notify = Duration::from_secs(HASH_STALL_NOTIFY_SEC);
    let reemit_interval = Duration::from_secs(HASH_STALL_REEMIT_SEC);
    let progress_emit_interval = Duration::from_secs(HASH_PROGRESS_EMIT_SEC);
    let poll = Duration::from_millis(HASH_WATCHDOG_POLL_MS);
    let start = Instant::now();
    let mut last_seen_count: u64 = 0;
    let mut last_progress_at = Instant::now();
    let mut last_notify_at: Option<Instant> = None;
    let mut last_progress_emit_at: Option<Instant> = None;

    loop {
        match rx_hash.recv_timeout(poll) {
            Ok(Ok(hash)) => return Ok(hash),
            Ok(Err(e)) => return Err(e),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                log::warn!("Hash thread disconnected for {}", path.display());
                return Err(HashError::Failed);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if start.elapsed() > total_timeout {
                    log::warn!(
                        "Hash total timeout ({}s, size {} B) for {}",
                        total_timeout.as_secs(), size, path.display()
                    );
                    LEAKED_THREADS.fetch_add(1, Ordering::Relaxed);
                    return Err(HashError::Timeout);
                }
                let cur = progress_count.load(Ordering::Relaxed);
                if cur != last_seen_count {
                    last_seen_count = cur;
                    last_progress_at = Instant::now();
                    last_notify_at = None;
                    // Periodic progress, so the bar keeps moving even inside a
                    // single enormous file.
                    let should_emit_progress = match last_progress_emit_at {
                        None => true,
                        Some(t) => t.elapsed() > progress_emit_interval,
                    };
                    if should_emit_progress {
                        let _ = app.emit(
                            "compare_progress",
                            CompareProgress {
                                phase: "diff_l3".to_string(),
                                count: 0,
                                total: 0,
                                current_path: path.display().to_string(),
                                bytes_done: cumulative_bytes.load(Ordering::Relaxed),
                                bytes_total,
                            },
                        );
                        last_progress_emit_at = Some(Instant::now());
                    }
                } else {
                    let stall = last_progress_at.elapsed();
                    if stall > stall_limit {
                        log::warn!(
                            "Hash stalled ({}s no progress) for {}",
                            stall.as_secs(), path.display()
                        );
                        LEAKED_THREADS.fetch_add(1, Ordering::Relaxed);
                        return Err(HashError::Timeout);
                    }
                    if stall > stall_notify {
                        let should_emit = match last_notify_at {
                            None => true,
                            Some(t) => t.elapsed() > reemit_interval,
                        };
                        if should_emit {
                            let _ = app.emit(
                                "compare_progress",
                                CompareProgress {
                                    phase: "diff_l3_stalled".to_string(),
                                    count: 0,
                                    total: 0,
                                    current_path: path.display().to_string(),
                                    bytes_done: cumulative_bytes.load(Ordering::Relaxed),
                                    bytes_total,
                                },
                            );
                            last_notify_at = Some(Instant::now());
                        }
                    }
                }
            }
        }
    }
}

fn compute_hash_timeout(size: u64) -> Duration {
    let secs = HASH_BASE_TIMEOUT_SEC.saturating_add(size / HASH_MIN_SPEED_BPS);
    Duration::from_secs(secs.min(HASH_MAX_TIMEOUT_SEC))
}

fn build_tree(
    map_a: &BTreeMap<String, ScannedFile>,
    map_b: &BTreeMap<String, ScannedFile>,
    status_map: &BTreeMap<String, DiffStatus>,
) -> Vec<DiffNode> {
    use std::collections::HashMap;

    let mut all_paths: Vec<&str> = status_map.keys().map(|s| s.as_str()).collect();
    // Sorted the way a file manager sorts, ignoring case, with the exact order
    // as a tie-break so it stays stable. A parent is a prefix of its children,
    // so it still comes first and the arena below always finds it.
    all_paths.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)));

    let mut arena: Vec<Option<DiffNode>> = Vec::with_capacity(all_paths.len());
    let mut children_of: Vec<Vec<usize>> = Vec::with_capacity(all_paths.len());
    let mut path_to_idx: HashMap<String, usize> = HashMap::with_capacity(all_paths.len());
    let mut root_indices: Vec<usize> = Vec::new();

    for path in &all_paths {
        let parts: Vec<&str> = path.split('/').collect();
        let name = parts[parts.len() - 1].to_string();

        let is_dir = map_a
            .get(*path)
            .map(|s| s.is_dir)
            .or_else(|| map_b.get(*path).map(|s| s.is_dir))
            .unwrap_or(false);

        let size_a = map_a.get(*path).filter(|s| !s.is_dir).map(|s| s.size);
        let size_b = map_b.get(*path).filter(|s| !s.is_dir).map(|s| s.size);
        let link_a = map_a.get(*path).map(|s| s.is_symlink).unwrap_or(false);
        let link_b = map_b.get(*path).map(|s| s.is_symlink).unwrap_or(false);
        // Hidden only where every side that has it says so: hidden on one side
        // and plain on the other is a divergence, and hiding it would conceal
        // exactly what this app exists to show.
        let hidden = match (map_a.get(*path), map_b.get(*path)) {
            (Some(a), Some(b)) => a.hidden && b.hidden,
            (Some(s), None) | (None, Some(s)) => s.hidden,
            (None, None) => false,
        };
        let status = *status_map.get(*path).unwrap_or(&DiffStatus::Same);

        let node = DiffNode {
            n: name,
            t: if is_dir { 'd' } else { 'f' },
            s: status,
            size_a,
            size_b,
            link_a,
            link_b,
            h: hidden,
            ch: Vec::new(),
        };

        let idx = arena.len();
        arena.push(Some(node));
        children_of.push(Vec::new());
        path_to_idx.insert(path.to_string(), idx);

        if parts.len() == 1 {
            root_indices.push(idx);
        } else {
            let parent_path = parts[..parts.len() - 1].join("/");
            if let Some(&parent_idx) = path_to_idx.get(&parent_path) {
                children_of[parent_idx].push(idx);
            } else {
                root_indices.push(idx);
            }
        }
    }

    fn assemble(
        idx: usize,
        arena: &mut Vec<Option<DiffNode>>,
        children_of: &[Vec<usize>],
    ) -> DiffNode {
        let mut node = arena[idx].take().expect("node already assembled");
        for &child_idx in &children_of[idx] {
            node.ch.push(assemble(child_idx, arena, children_of));
        }
        // Bottom-up: a folder present on both sides turns different as soon as
        // one child differs. Folders present on one side only keep their status.
        if node.t == 'd' && node.s == DiffStatus::Same
            && node.ch.iter().any(|c| c.s != DiffStatus::Same)
        {
            node.s = DiffStatus::Diff;
        }
        node
    }

    root_indices
        .iter()
        .map(|&idx| assemble(idx, &mut arena, &children_of))
        .collect()
}

fn is_excluded_dir(name: &str) -> bool {
    if EXCLUDED_DIRS.iter().any(|&ex| ex == name) {
        return true;
    }
    if crate::sys::is_system_dir(name) {
        return true;
    }
    // External filesystems use .Trash-UID (e.g. .Trash-1000)
    if name.starts_with(".Trash-") {
        return true;
    }
    false
}
