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

// Linux diagnosis: asks the health tool about both disks under a single pkexec
// call and classifies each one. Shared types and the commands live in the
// parent module.

use super::{
    build_disk, worst_level, DeepCheckPid, DeepCheckResult, DeepCheckStarted, DiagnosisResult,
};
use crate::CmdError;
use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter, Manager};

// Where the health tool is. Asked of the system boundary, which is the only
// place that knows how to find it: invoking it at a fixed path meant the app
// could report it as installed and then read nothing.
fn tool_path() -> String {
    crate::sys::smartctl_path()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

fn extract_section<'a>(haystack: &'a str, start: &str, end: &str) -> &'a str {
    if let Some(s) = haystack.find(start) {
        let after = &haystack[s + start.len()..];
        if let Some(e) = after.find(end) {
            return &after[..e];
        }
    }
    ""
}

// A name nobody else can work out in advance. The system's own randomness seeds
// this; it does not have to be unguessable forever, only for the moment the
// file exists.
fn random_token() -> String {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_usize(std::process::id() as usize);
    format!("{:016x}", hasher.finish())
}

// Where a script that will run as administrator can wait without anybody else
// being able to read it, replace it, or put a link in its place. The shared
// temporary folder is exactly the wrong place for that, so this uses a
// directory of this user's own, kept to this user's eyes only.
fn private_dir() -> Result<PathBuf, CmdError> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".cache")
        });
    let dir = base.join("geminus");
    fs::create_dir_all(&dir).map_err(|e| CmdError::from_io("diagnosis_failed", &e))?;
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    sweep_stale(&dir);
    Ok(dir)
}

// Each of these files carries a name of its own, so an app killed mid-check
// leaves one behind instead of overwriting it next time. A day is well past the
// longest test a disk declares, and a script already being read stays readable
// to the shell reading it even once its name is gone.
fn sweep_stale(dir: &std::path::Path) {
    const A_DAY: u64 = 24 * 60 * 60;
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let age = entry
            .metadata()
            .and_then(|m| m.modified())
            .and_then(|t| t.elapsed().map_err(|e| std::io::Error::other(e.to_string())));
        if let Ok(age) = age {
            if age.as_secs() > A_DAY {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

// The script itself: created in exclusive mode, so it is ours or the call
// fails, and never handed a name that already existed.
fn write_private_script(prefix: &str, body: &str) -> Result<PathBuf, CmdError> {
    let path = private_dir()?.join(format!("{}-{}.sh", prefix, random_token()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&path)
        .map_err(|e| CmdError::from_io("diagnosis_failed", &e))?;
    file.write_all(body.as_bytes())
        .map_err(|e| CmdError::from_io("diagnosis_failed", &e))?;
    Ok(path)
}

// Nothing is pasted into this script: what it works on arrives as arguments, so
// a name carrying a quote stays a name instead of becoming another command in a
// shell that runs as administrator.
const DIAGNOSIS_SCRIPT: &str = r#"#!/bin/bash
set +e
TOOL="$1"
DEV_A="$2"
DEV_B="$3"
echo "===SMART_A==="
if [ -x "$TOOL" ] && [ -n "$DEV_A" ] && [ -b "$DEV_A" ]; then
  "$TOOL" -a "$DEV_A" 2>&1
else
  echo "NO_DEVICE"
fi
echo "===END_SMART_A==="
echo "===SMART_B==="
if [ -x "$TOOL" ] && [ -n "$DEV_B" ] && [ -b "$DEV_B" ]; then
  "$TOOL" -a "$DEV_B" 2>&1
else
  echo "NO_DEVICE"
fi
echo "===END_SMART_B==="
"#;

pub fn run_disk_diagnosis(path_a: &str, path_b: &str) -> Result<DiagnosisResult, CmdError> {
    let dev_a = crate::sys::device_for_path(path_a).unwrap_or_default();
    let dev_b = crate::sys::device_for_path(path_b).unwrap_or_default();

    let script = write_private_script("diag", DIAGNOSIS_SCRIPT)?;

    let out = Command::new("pkexec")
        .arg("bash")
        .arg(&script)
        .arg(tool_path())
        .arg(&dev_a)
        .arg(&dev_b)
        .output()
        .map_err(|e| CmdError::from_io("diagnosis_failed", &e))?;

    let _ = fs::remove_file(&script);

    // pkexec exit 126 = dialog dismissed, 127 = not authorized. Either way: user refused.
    if !out.status.success() {
        let code = out.status.code().unwrap_or(-1);
        if code == 126 || code == 127 {
            return Err(CmdError::plain("auth_dismissed"));
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(CmdError::with_detail("diagnosis_failed", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let smart_a_sec = extract_section(&stdout, "===SMART_A===\n", "===END_SMART_A===");
    let smart_b_sec = extract_section(&stdout, "===SMART_B===\n", "===END_SMART_B===");

    let disk_a = build_disk(&dev_a, smart_a_sec);
    let disk_b = build_disk(&dev_b, smart_b_sec);
    let overall = worst_level(&disk_a.level, &disk_b.level);

    Ok(DiagnosisResult {
        disk_a,
        disk_b,
        overall_level: overall,
    })
}

// ════════════════════════════════════════════════════════════
// DEEP CHECK (smartctl -t long)
// ════════════════════════════════════════════════════════════

// xdg-user-dir returns the localized Downloads folder (e.g. ~/Scaricati on IT systems).
// Falls back to $HOME/Downloads if xdg-user-dir is missing or resolves to $HOME itself.
fn resolve_downloads_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    if let Ok(out) = Command::new("xdg-user-dir").arg("DOWNLOAD").output() {
        if out.status.success() {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() && path != home {
                return path;
            }
        }
    }
    format!("{}/Downloads", home)
}

// Extract a filename-safe label from a mount path: "/media/luca/Backup 4T" -> "Backup_4T".
fn sanitize_label(mount_path: &str) -> String {
    let base = mount_path.trim_end_matches('/').rsplit('/').next().unwrap_or(mount_path);
    let cleaned: String = base.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if cleaned.is_empty() { "disk".to_string() } else { cleaned }
}

// Arguments, never pasted-in text — same reason as the script above. The report
// is not written here either: it comes back on the standard output and the app
// saves it, so nothing is ever created as administrator inside the user's own
// folders and no ownership has to be handed back afterwards.
const DEEP_CHECK_SCRIPT: &str = r#"#!/bin/bash
set +e
TOOL="$1"
DEV="$2"
SENTINEL="$3"

# Estimated time (minutes) reported by the disk itself.
ESTIMATED=$("$TOOL" -c "$DEV" 2>&1 | grep -A1 "Extended self-test routine" | grep "polling time" | grep -oE "[0-9]+" | head -1)
if [ -z "$ESTIMATED" ]; then ESTIMATED=60; fi
echo "ESTIMATED:$ESTIMATED"

# Kick off the long self-test (runs in drive firmware, returns immediately).
"$TOOL" -t long "$DEV" > /dev/null 2>&1

# Poll every 10s. Timeout at 2x the estimated minutes. Self-terminate if parent (GEMINUS) died.
MAX_WAIT=$((ESTIMATED * 2 * 60))
ELAPSED=0
while [ $ELAPSED -lt $MAX_WAIT ]; do
  sleep 10
  ELAPSED=$((ELAPSED + 10))

  # Auto-termination: if GEMINUS died our PPID becomes 1 (init) → abort self-test and exit.
  CURRENT_PPID=$(ps -o ppid= -p $$ 2>/dev/null | tr -d ' ')
  if [ "$CURRENT_PPID" = "1" ] || [ -z "$CURRENT_PPID" ]; then
    "$TOOL" -X "$DEV" 2>/dev/null
    exit 130
  fi

  # Cancel sentinel: GEMINUS drops this file to stop us. It cannot signal this
  # process — we are root and it is not.
  if [ -f "$SENTINEL" ]; then
    rm -f "$SENTINEL"
    "$TOOL" -X "$DEV" 2>/dev/null
    exit 130
  fi

  # 240 and up means the test is still running; below that it is over, well or
  # badly. Waiting for a plain 0 would only recognise the good ending, and leave
  # a failed test polling until the timeout. The first answer is not trusted:
  # a disk that has not started yet still reports the previous test.
  STATUS=$("$TOOL" -c "$DEV" 2>&1 | grep "Self-test execution status" | grep -oE "\([[:space:]]*[0-9]+[[:space:]]*\)" | grep -oE "[0-9]+" | head -1)
  if [ -n "$STATUS" ] && [ $ELAPSED -gt 10 ] && [ "$STATUS" -lt 240 ]; then
    break
  fi
done

echo "WHEN:$(date +"%Y-%m-%d %H:%M:%S")"
echo "===GEMINUS-REPORT==="
echo "=== smartctl -a ==="
"$TOOL" -a "$DEV" 2>&1
echo ""
echo "=== smartctl -l selftest ==="
"$TOOL" -l selftest "$DEV" 2>&1

# The tool answers with a set of flags, not with success or failure: a disk
# whose test found something makes it exit non-zero. Ending on that would turn
# the very report worth keeping into "the check did not work", so what this
# script says about itself is only whether it got to the end.
exit 0
"#;

const REPORT_MARK: &str = "===GEMINUS-REPORT===";

// Stops the disk's self-test and ends the privileged process running it.
pub fn kill_deep_check(app: &AppHandle) {
    let snapshot: Option<(u32, String)> = {
        let state = app.state::<DeepCheckPid>();
        let guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    };
    let (pid, sentinel) = match snapshot {
        Some(s) => s,
        None => return,
    };

    // Drop the sentinel file: the privileged script sees it on its next poll
    // and stops the test itself, so no second consent prompt is needed.
    let _ = fs::write(&sentinel, b"cancel");

    // A TERM on top, to cut a sleep short. It usually fails — that process is
    // root and this one is not — and the sentinel above is what really stops it.
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();

    let state = app.state::<DeepCheckPid>();
    let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    *guard = None;
}

// The report belongs to this side, as it does on the other system: the
// privileged half only reads the disk, and what lands in the user's folders is
// written by the process that is already that user.
fn save_report(
    downloads: &str,
    label: &str,
    header: &str,
    when: &str,
    body: &str,
) -> Result<String, CmdError> {
    let dir = std::path::Path::new(downloads);
    fs::create_dir_all(dir).map_err(|e| CmdError::from_io("report_not_saved", &e))?;
    // The name stays neutral and shaped the same on both systems: it lives on
    // the disk long after the session. The moment comes from when the disk
    // finished, reduced to its digits.
    let digits: String = when.chars().filter(|c| c.is_ascii_digit()).collect();
    let stamp = if digits.len() >= 14 {
        format!("{}_{}", &digits[..8], &digits[8..14])
    } else {
        "unknown".to_string()
    };
    let file = dir.join(format!("geminus_health_{}_{}.txt", label, stamp));
    let text = format!("{}\n\n{}", header.replace("{date}", when), body);
    fs::write(&file, text).map_err(|e| CmdError::from_io("report_not_saved", &e))?;
    Ok(file.to_string_lossy().into_owned())
}

pub fn run_deep_check(
    app: AppHandle,
    device: &str,
    mount_path: &str,
    header: &str,
) -> Result<(), CmdError> {
    if !device.starts_with("/dev/") {
        return Err(CmdError::plain("invalid_device"));
    }
    let downloads = resolve_downloads_dir();
    let label = sanitize_label(mount_path);
    let header = header.to_string();

    let script = write_private_script("deep", DEEP_CHECK_SCRIPT)?;
    // The privileged half cannot be signalled by this one — it is root and this
    // is not — so cancelling passes through a file it watches for. It lives
    // beside the script, in the same private directory.
    let sentinel = script.with_extension("cancel");

    let app_thread = app.clone();
    let device_thread = device.to_string();
    let script_thread = script;
    let sentinel_thread = sentinel.clone();

    std::thread::spawn(move || {
        let emit_err = |app: &AppHandle, err: CmdError| {
            let _ = app.emit("deep_check_result", DeepCheckResult {
                success: false, cancelled: false,
                saved_path: String::new(), error: Some(err),
            });
        };
        let cleanup = |script: &std::path::Path, sentinel: &std::path::Path| {
            let _ = fs::remove_file(script);
            let _ = fs::remove_file(sentinel);
        };

        let mut child = match Command::new("pkexec")
            .arg("bash")
            .arg(&script_thread)
            .arg(tool_path())
            .arg(&device_thread)
            .arg(&sentinel_thread)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                emit_err(&app_thread, CmdError::from_io("deep_check_failed", &e));
                cleanup(&script_thread, &sentinel_thread);
                return;
            }
        };

        let child_pid = child.id();
        {
            let state = app_thread.state::<DeepCheckPid>();
            let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some((child_pid, sentinel_thread.to_string_lossy().into_owned()));
        }

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                emit_err(&app_thread, CmdError::plain("deep_check_failed"));
                cleanup(&script_thread, &sentinel_thread);
                return;
            }
        };

        // Everything after the mark is the report itself; before it come the
        // two facts the app needs while it waits.
        let mut finished_at = String::new();
        let mut body = String::new();
        let mut in_report = false;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if in_report {
                body.push_str(&line);
                body.push('\n');
            } else if line.trim() == REPORT_MARK {
                in_report = true;
            } else if let Some(n) = line.strip_prefix("ESTIMATED:") {
                if let Ok(est) = n.trim().parse::<u32>() {
                    let _ = app_thread.emit("deep_check_started", DeepCheckStarted {
                        device: device_thread.clone(),
                        estimated_minutes: est,
                    });
                }
            } else if let Some(w) = line.strip_prefix("WHEN:") {
                finished_at = w.trim().to_string();
            }
        }

        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => {
                emit_err(&app_thread, CmdError::from_io("deep_check_failed", &e));
                cleanup(&script_thread, &sentinel_thread);
                return;
            }
        };
        cleanup(&script_thread, &sentinel_thread);

        let was_cancelled = {
            let state = app_thread.state::<DeepCheckPid>();
            let guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
            guard.is_none()
        };
        {
            let state = app_thread.state::<DeepCheckPid>();
            let mut guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
            *guard = None;
        }

        if was_cancelled {
            let _ = app_thread.emit("deep_check_result", DeepCheckResult {
                success: false, cancelled: true,
                saved_path: String::new(), error: None,
            });
            return;
        }

        if !status.success() {
            let code = status.code().unwrap_or(-1);
            let err = if code == 126 || code == 127 {
                CmdError::plain("auth_dismissed")
            } else {
                CmdError::with_detail("deep_check_failed", format!("exit {}", code))
            };
            emit_err(&app_thread, err);
            return;
        }

        if body.trim().is_empty() {
            emit_err(&app_thread, CmdError::plain("deep_check_no_report"));
            return;
        }

        match save_report(&downloads, &label, &header, &finished_at, &body) {
            Ok(saved_path) => {
                let _ = app_thread.emit("deep_check_result", DeepCheckResult {
                    success: true, cancelled: false,
                    saved_path, error: None,
                });
            }
            Err(err) => emit_err(&app_thread, err),
        }
    });

    Ok(())
}
