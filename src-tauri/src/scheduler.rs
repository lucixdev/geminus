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

// Automatic jobs: a saved pair of folders plus rules, run only while the app is
// open. The frontend owns the clock (it computes `nextRun` and calls
// `sched_run_now` when due); this side stores the jobs, plans the copies and
// performs them. Nothing here ever deletes anything from a compared folder.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::CmdError;

// ---------------------------------------------------------------------------
// Pure part: no tauri, no I/O. Tested standalone.
// ---------------------------------------------------------------------------
pub mod plan {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Side {
        A,
        B,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Status {
        Same,
        Diff,
        OnlyA,
        OnlyB,
        Unreadable,
        Unknown,
    }

    impl Status {
        pub fn from_str(s: &str) -> Status {
            match s {
                "same" => Status::Same,
                "diff" => Status::Diff,
                "onlyA" => Status::OnlyA,
                "onlyB" => Status::OnlyB,
                "unreadable" => Status::Unreadable,
                _ => Status::Unknown,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DiffRule {
        AToB,
        BToA,
        Newer,
        Ignore,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Rules {
        pub only_a_copy: bool,
        pub only_b_copy: bool,
        pub diff: DiffRule,
    }

    impl Rules {
        pub fn from_strs(only_a: &str, only_b: &str, diff: &str) -> Option<Rules> {
            let only_a_copy = match only_a {
                "copyAB" => true,
                "ignore" => false,
                _ => return None,
            };
            let only_b_copy = match only_b {
                "copyBA" => true,
                "ignore" => false,
                _ => return None,
            };
            let diff = match diff {
                "aToB" => DiffRule::AToB,
                "bToA" => DiffRule::BToA,
                "newer" => DiffRule::Newer,
                "ignore" => DiffRule::Ignore,
                _ => return None,
            };
            Some(Rules { only_a_copy, only_b_copy, diff })
        }
    }

    #[derive(Debug, Clone)]
    pub struct Node {
        pub name: String,
        pub is_dir: bool,
        pub status: Status,
        pub size_a: Option<u64>,
        pub size_b: Option<u64>,
        pub mod_a: Option<i64>,
        pub mod_b: Option<i64>,
        pub link_a: bool,
        pub link_b: bool,
        pub children: Vec<Node>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Action {
        pub src_side: Side,
        pub rel: String,
        pub is_dir: bool,
    }

    #[derive(Debug, Default)]
    pub struct Plan {
        pub actions: Vec<Action>,
        // One line per thing left alone on purpose: "<reason>:<rel>".
        pub skipped: Vec<String>,
    }

    // Modification times closer than this count as equal (file systems round
    // differently).
    pub const SAME_TIME_SECS: i64 = 2;

    pub fn plan_actions(nodes: &[Node], rules: &Rules) -> Plan {
        let mut out = Plan::default();
        walk(nodes, "", rules, &mut out);
        out
    }

    fn walk(nodes: &[Node], prefix: &str, rules: &Rules, out: &mut Plan) {
        for n in nodes {
            let rel = if prefix.is_empty() {
                n.name.clone()
            } else {
                format!("{}/{}", prefix, n.name)
            };
            match n.status {
                Status::Same => {}
                Status::Unreadable => out.skipped.push(format!("unreadable:{}", rel)),
                Status::Unknown => out.skipped.push(format!("unknown_status:{}", rel)),
                Status::OnlyA => {
                    if rules.only_a_copy {
                        if n.link_a {
                            out.skipped.push(format!("symlink:{}", rel));
                        } else {
                            out.actions.push(Action { src_side: Side::A, rel, is_dir: n.is_dir });
                        }
                    }
                }
                Status::OnlyB => {
                    if rules.only_b_copy {
                        if n.link_b {
                            out.skipped.push(format!("symlink:{}", rel));
                        } else {
                            out.actions.push(Action { src_side: Side::B, rel, is_dir: n.is_dir });
                        }
                    }
                }
                Status::Diff => {
                    // A folder on one side and a file on the other: the node
                    // carries a size only for the side that holds a file.
                    let mismatch = if n.is_dir {
                        n.size_a.is_some() || n.size_b.is_some()
                    } else {
                        n.size_a.is_none() || n.size_b.is_none()
                    };
                    if mismatch {
                        out.skipped.push(format!("type_mismatch:{}", rel));
                        continue;
                    }
                    if n.link_a || n.link_b {
                        out.skipped.push(format!("symlink:{}", rel));
                        continue;
                    }
                    if n.is_dir {
                        walk(&n.children, &rel, rules, out);
                        continue;
                    }
                    let side = match rules.diff {
                        DiffRule::Ignore => None,
                        DiffRule::AToB => Some(Side::A),
                        DiffRule::BToA => Some(Side::B),
                        DiffRule::Newer => match (n.mod_a, n.mod_b) {
                            (Some(a), Some(b)) => {
                                if (a - b).abs() <= SAME_TIME_SECS {
                                    out.skipped.push(format!("same_time:{}", rel));
                                    None
                                } else if a > b {
                                    Some(Side::A)
                                } else {
                                    Some(Side::B)
                                }
                            }
                            _ => {
                                out.skipped.push(format!("no_time:{}", rel));
                                None
                            }
                        },
                    };
                    if let Some(src_side) = side {
                        out.actions.push(Action { src_side, rel, is_dir: false });
                    }
                }
            }
        }
    }

    // "HH:MM", 00:00..23:59.
    pub fn valid_hhmm(s: &str) -> bool {
        let b = s.as_bytes();
        if b.len() != 5 || b[2] != b':' {
            return false;
        }
        if !(b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[3].is_ascii_digit() && b[4].is_ascii_digit()) {
            return false;
        }
        let h = (b[0] - b'0') * 10 + (b[1] - b'0');
        let m = (b[3] - b'0') * 10 + (b[4] - b'0');
        h < 24 && m < 60
    }

    pub const DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

    // UTC timestamp "YYYY-MM-DD HH:MM:SS" from unix seconds.
    pub fn fmt_utc(secs: i64) -> String {
        let days = secs.div_euclid(86400);
        let rem = secs.rem_euclid(86400);
        // Civil-from-days (Howard Hinnant).
        let z = days + 719468;
        let era = z.div_euclid(146097);
        let doe = z.rem_euclid(146097);
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60
        )
    }

    // The second half of a log, cut at a line start.
    pub fn keep_last_half(text: &str) -> &str {
        let mid = text.len() / 2;
        let mut start = mid;
        while start < text.len() && !text.is_char_boundary(start) {
            start += 1;
        }
        match text[start..].find('\n') {
            Some(i) => &text[start + i + 1..],
            None => &text[start..],
        }
    }

    // A job id ends up in a file name: letters, digits, '-' and '_' only.
    pub fn safe_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 64
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn node(name: &str, is_dir: bool, status: Status) -> Node {
            Node {
                name: name.into(),
                is_dir,
                status,
                size_a: None,
                size_b: None,
                mod_a: None,
                mod_b: None,
                link_a: false,
                link_b: false,
                children: vec![],
            }
        }
        fn file(name: &str, status: Status, ma: i64, mb: i64) -> Node {
            let mut n = node(name, false, status);
            n.size_a = Some(1);
            n.size_b = Some(2);
            n.mod_a = Some(ma);
            n.mod_b = Some(mb);
            n
        }
        fn rules(a: &str, b: &str, d: &str) -> Rules {
            Rules::from_strs(a, b, d).unwrap()
        }

        #[test]
        fn only_sides_follow_rules() {
            let tree = vec![node("x", true, Status::OnlyA), node("y.txt", false, Status::OnlyB)];
            let p = plan_actions(&tree, &rules("copyAB", "copyBA", "ignore"));
            assert_eq!(
                p.actions,
                vec![
                    Action { src_side: Side::A, rel: "x".into(), is_dir: true },
                    Action { src_side: Side::B, rel: "y.txt".into(), is_dir: false },
                ]
            );
            let p = plan_actions(&tree, &rules("ignore", "copyBA", "ignore"));
            assert_eq!(p.actions.len(), 1);
            assert_eq!(p.actions[0].src_side, Side::B);
            let p = plan_actions(&tree, &rules("ignore", "ignore", "ignore"));
            assert!(p.actions.is_empty());
        }

        #[test]
        fn diff_folder_descends_with_paths() {
            let mut d = node("d", true, Status::Diff);
            d.children = vec![
                file("f", Status::Diff, 10, 20),
                node("same", false, Status::Same),
                node("new", false, Status::OnlyA),
            ];
            let p = plan_actions(&[d], &rules("copyAB", "copyBA", "aToB"));
            assert_eq!(p.actions.len(), 2);
            assert_eq!(p.actions[0].rel, "d/f");
            assert_eq!(p.actions[0].src_side, Side::A);
            assert_eq!(p.actions[1].rel, "d/new");
        }

        #[test]
        fn diff_rules() {
            let t = vec![file("f", Status::Diff, 100, 200)];
            assert_eq!(plan_actions(&t, &rules("ignore", "ignore", "aToB")).actions[0].src_side, Side::A);
            assert_eq!(plan_actions(&t, &rules("ignore", "ignore", "bToA")).actions[0].src_side, Side::B);
            assert_eq!(plan_actions(&t, &rules("ignore", "ignore", "newer")).actions[0].src_side, Side::B);
            assert!(plan_actions(&t, &rules("ignore", "ignore", "ignore")).actions.is_empty());
            let t = vec![file("f", Status::Diff, 300, 200)];
            assert_eq!(plan_actions(&t, &rules("ignore", "ignore", "newer")).actions[0].src_side, Side::A);
        }

        #[test]
        fn newer_with_equal_times_is_skipped_and_logged() {
            let t = vec![file("f", Status::Diff, 100, 102)];
            let p = plan_actions(&t, &rules("ignore", "ignore", "newer"));
            assert!(p.actions.is_empty());
            assert_eq!(p.skipped, vec!["same_time:f".to_string()]);
        }

        #[test]
        fn unreadable_mismatch_and_links_are_skipped() {
            let mut mm = node("m", true, Status::Diff);
            mm.size_b = Some(5); // a file on side B
            let mut ft = node("g", false, Status::Diff);
            ft.size_a = Some(5); // a folder on side B
            let mut lk = file("l", Status::Diff, 1, 100);
            lk.link_b = true;
            let mut lo = node("lo", false, Status::OnlyA);
            lo.link_a = true;
            let t = vec![node("u", false, Status::Unreadable), mm, ft, lk, lo];
            let p = plan_actions(&t, &rules("copyAB", "copyBA", "newer"));
            assert!(p.actions.is_empty());
            assert_eq!(
                p.skipped,
                vec!["unreadable:u", "type_mismatch:m", "type_mismatch:g", "symlink:l", "symlink:lo"]
            );
        }

        #[test]
        fn never_anything_but_copies() {
            // The plan type has no delete: every action is a copy from a side.
            let t = vec![file("f", Status::Diff, 1, 2), node("o", false, Status::OnlyB)];
            let p = plan_actions(&t, &rules("copyAB", "copyBA", "bToA"));
            assert!(p.actions.iter().all(|a| a.src_side == Side::B));
        }

        #[test]
        fn rules_parsing() {
            assert!(Rules::from_strs("copyBA", "copyBA", "aToB").is_none());
            assert!(Rules::from_strs("copyAB", "copyBA", "x").is_none());
        }

        #[test]
        fn hhmm() {
            assert!(valid_hhmm("00:00") && valid_hhmm("23:59") && valid_hhmm("07:05"));
            assert!(!valid_hhmm("24:00") && !valid_hhmm("12:60") && !valid_hhmm("7:05") && !valid_hhmm("ab:cd") && !valid_hhmm(""));
        }

        #[test]
        fn utc_format() {
            assert_eq!(fmt_utc(0), "1970-01-01 00:00:00");
            assert_eq!(fmt_utc(951782400), "2000-02-29 00:00:00");
            assert_eq!(fmt_utc(1700000000), "2023-11-14 22:13:20");
        }

        #[test]
        fn log_halving_and_ids() {
            let t = "aaaa\nbbbb\ncccc\ndddd\n";
            assert_eq!(keep_last_half(t), "dddd\n");
            assert!(safe_id("job-1700000000000") && !safe_id("../x") && !safe_id("") && !safe_id("a b"));
        }
    }
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rules {
    pub only_a: String,
    pub only_b: String,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub kind: String,
    pub time: String,
    #[serde(default)]
    pub days: Vec<String>,
    #[serde(default)]
    pub every_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRun {
    pub at: i64,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub path_a: String,
    pub path_b: String,
    #[serde(default)]
    pub device_a: Option<String>,
    #[serde(default)]
    pub device_b: Option<String>,
    pub mode: String,
    pub rules: Rules,
    pub schedule: Schedule,
    #[serde(default)]
    pub last_run: Option<LastRun>,
    // Unix seconds; computed by the frontend, stored as given.
    #[serde(default)]
    pub next_run: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub status: String,
    pub summary: String,
    pub copied: u64,
    pub skipped: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunDone {
    id: String,
    #[serde(flatten)]
    run: RunSummary,
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

const LOG_MAX_BYTES: u64 = 512 * 1024;
const LOG_TAIL_BYTES: u64 = 64 * 1024;

// jobs.json is read and rewritten from several threads (a save while a run
// finishes): one at a time.
static FILE_LOCK: Mutex<()> = Mutex::new(());
static RUNNING: AtomicBool = AtomicBool::new(false);

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn base_dir(app: &AppHandle) -> Result<PathBuf, CmdError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| CmdError::with_detail("sched_store_failed", e.to_string()))?
        .join("scheduler");
    Ok(dir)
}

fn jobs_path(app: &AppHandle) -> Result<PathBuf, CmdError> {
    Ok(base_dir(app)?.join("jobs.json"))
}

fn log_path(app: &AppHandle, id: &str) -> Result<PathBuf, CmdError> {
    Ok(base_dir(app)?.join("logs").join(format!("{}.log", id)))
}

// Callers hold FILE_LOCK.
fn load_jobs(path: &Path) -> Result<Vec<Job>, CmdError> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            if text.trim().is_empty() {
                return Ok(Vec::new());
            }
            serde_json::from_str(&text)
                .map_err(|e| CmdError::with_detail("sched_store_failed", e.to_string()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(CmdError::from_io("sched_store_failed", &e)),
    }
}

// Callers hold FILE_LOCK. Written beside and renamed, so a crash never leaves
// half a file.
fn store_jobs(path: &Path, jobs: &[Job]) -> Result<(), CmdError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CmdError::from_io("sched_store_failed", &e))?;
    }
    let text = serde_json::to_string_pretty(jobs)
        .map_err(|e| CmdError::with_detail("sched_store_failed", e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| CmdError::from_io("sched_store_failed", &e))?;
    std::fs::rename(&tmp, path).map_err(|e| CmdError::from_io("sched_store_failed", &e))?;
    Ok(())
}

fn log_line(app: &AppHandle, id: &str, msg: &str) {
    use std::io::Write;
    let path = match log_path(app, id) {
        Ok(p) => p,
        Err(_) => return,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Past the cap, the older half goes.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > LOG_MAX_BYTES {
            if let Ok(bytes) = std::fs::read(&path) {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let _ = std::fs::write(&path, plan::keep_last_half(&text));
            }
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "[{} UTC] {}", plan::fmt_utc(now_secs()), msg);
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate(job: &Job) -> Result<(), CmdError> {
    let bad = |what: &str| Err(CmdError::with_detail("sched_invalid", what));
    if job.path_a.trim().is_empty() || job.path_b.trim().is_empty() {
        return bad("path");
    }
    if job.mode != "fast" && job.mode != "deep" {
        return bad("mode");
    }
    if plan::Rules::from_strs(&job.rules.only_a, &job.rules.only_b, &job.rules.diff).is_none() {
        return bad("rules");
    }
    if !plan::valid_hhmm(&job.schedule.time) {
        return bad("time");
    }
    match job.schedule.kind.as_str() {
        "daily" => {}
        "weekly" => {
            if job.schedule.days.is_empty()
                || !job.schedule.days.iter().all(|d| plan::DAYS.contains(&d.as_str()))
            {
                return bad("days");
            }
        }
        "hourly" => {
            if !(1..=23).contains(&job.schedule.every_hours) {
                return bad("everyHours");
            }
        }
        _ => return bad("kind"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn sched_list(app: AppHandle) -> Result<Vec<Job>, CmdError> {
    let path = jobs_path(&app)?;
    let _g = FILE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    load_jobs(&path)
}

#[tauri::command]
pub fn sched_save(app: AppHandle, job: Job) -> Result<Job, CmdError> {
    let mut job = job;
    if job.id.trim().is_empty() {
        job.id = format!("job-{}", now_millis());
    }
    if !plan::safe_id(&job.id) {
        return Err(CmdError::with_detail("sched_invalid", "id"));
    }
    validate(&job)?;

    let path = jobs_path(&app)?;
    let _g = FILE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut jobs = load_jobs(&path)?;
    let existing = jobs.iter().find(|j| j.id == job.id).cloned();

    // The last run is the run's to write, never the editor's.
    job.last_run = existing.as_ref().and_then(|e| e.last_run.clone());

    // The device is remembered once and kept while the path stays; a changed
    // path gets the device it is on now, when the system can tell.
    let keep = |old_path: Option<&String>, old_dev: Option<&Option<String>>, new_path: &str| -> Option<String> {
        match (old_path, old_dev) {
            (Some(p), Some(Some(d))) if p == new_path => Some(d.clone()),
            _ => crate::sys::device_for_path(new_path).ok(),
        }
    };
    job.device_a = keep(
        existing.as_ref().map(|e| &e.path_a),
        existing.as_ref().map(|e| &e.device_a),
        &job.path_a,
    );
    job.device_b = keep(
        existing.as_ref().map(|e| &e.path_b),
        existing.as_ref().map(|e| &e.device_b),
        &job.path_b,
    );

    match jobs.iter_mut().find(|j| j.id == job.id) {
        Some(slot) => *slot = job.clone(),
        None => jobs.push(job.clone()),
    }
    store_jobs(&path, &jobs)?;
    Ok(job)
}

#[tauri::command]
pub fn sched_delete(app: AppHandle, id: String) -> Result<(), CmdError> {
    if !plan::safe_id(&id) {
        return Err(CmdError::with_detail("sched_invalid", "id"));
    }
    let path = jobs_path(&app)?;
    {
        let _g = FILE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let mut jobs = load_jobs(&path)?;
        let before = jobs.len();
        jobs.retain(|j| j.id != id);
        if jobs.len() != before {
            store_jobs(&path, &jobs)?;
        }
    }
    // The log of this job only; never anything of the compared folders.
    if let Ok(lp) = log_path(&app, &id) {
        let _ = std::fs::remove_file(lp);
    }
    Ok(())
}

#[tauri::command]
pub fn sched_log(app: AppHandle, id: String) -> Result<String, CmdError> {
    use std::io::{Read, Seek, SeekFrom};
    if !plan::safe_id(&id) {
        return Err(CmdError::with_detail("sched_invalid", "id"));
    }
    let path = log_path(&app, &id)?;
    let mut f = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(CmdError::from_io("sched_log_failed", &e)),
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let cut = len > LOG_TAIL_BYTES;
    if cut {
        f.seek(SeekFrom::Start(len - LOG_TAIL_BYTES))
            .map_err(|e| CmdError::from_io("sched_log_failed", &e))?;
    }
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| CmdError::from_io("sched_log_failed", &e))?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    if cut {
        // The first line is probably cut in half.
        if let Some(i) = text.find('\n') {
            return Ok(text[i + 1..].to_string());
        }
    }
    Ok(text)
}

#[tauri::command]
pub async fn sched_run_now(app: AppHandle, id: String) -> Result<RunSummary, CmdError> {
    if !plan::safe_id(&id) {
        return Err(CmdError::with_detail("sched_invalid", "id"));
    }
    tauri::async_runtime::spawn_blocking(move || run_job(&app, &id))
        .await
        .map_err(|e| CmdError::with_detail("sched_run_failed", e.to_string()))?
}

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

struct RunGuard;

impl RunGuard {
    fn acquire() -> Option<RunGuard> {
        if RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            Some(RunGuard)
        } else {
            None
        }
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

// While alive, per-file failures are skipped instead of asked about.
struct HeadlessGuard;

impl HeadlessGuard {
    fn new() -> HeadlessGuard {
        let _ = crate::ops::headless_errors_take();
        crate::ops::HEADLESS.store(true, Ordering::SeqCst);
        HeadlessGuard
    }
}

impl Drop for HeadlessGuard {
    fn drop(&mut self) {
        crate::ops::HEADLESS.store(false, Ordering::SeqCst);
    }
}

fn run_job(app: &AppHandle, id: &str) -> Result<RunSummary, CmdError> {
    let _run = RunGuard::acquire().ok_or_else(|| CmdError::plain("sched_busy"))?;

    let path = jobs_path(app)?;
    let job = {
        let _g = FILE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        load_jobs(&path)?
            .into_iter()
            .find(|j| j.id == id)
            .ok_or_else(|| CmdError::plain("sched_job_missing"))?
    };

    let summary = execute(app, &job);

    // Only the last run is touched: the job may have been edited meanwhile.
    {
        let _g = FILE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        if let Ok(mut jobs) = load_jobs(&path) {
            if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
                j.last_run = Some(LastRun {
                    at: now_secs(),
                    status: summary.status.clone(),
                    summary: summary.summary.clone(),
                });
                let _ = store_jobs(&path, &jobs);
            }
        }
    }

    let _ = app.emit("sched_run_done", RunDone { id: id.to_string(), run: summary.clone() });
    Ok(summary)
}

fn outcome(status: &str, summary: String, copied: u64, skipped: u64, failed: u64) -> RunSummary {
    RunSummary { status: status.to_string(), summary, copied, skipped, failed }
}

fn node_from_json(v: &Value) -> Option<plan::Node> {
    let name = v.get("n")?.as_str()?.to_string();
    let is_dir = v.get("t")?.as_str()? == "d";
    let status = plan::Status::from_str(v.get("s")?.as_str()?);
    let children = match v.get("ch").and_then(|c| c.as_array()) {
        Some(arr) => arr.iter().filter_map(node_from_json).collect(),
        None => Vec::new(),
    };
    Some(plan::Node {
        name,
        is_dir,
        status,
        size_a: v.get("sA").and_then(|x| x.as_u64()),
        size_b: v.get("sB").and_then(|x| x.as_u64()),
        mod_a: v.get("mA").and_then(|x| x.as_i64()),
        mod_b: v.get("mB").and_then(|x| x.as_i64()),
        link_a: v.get("linkA").and_then(|x| x.as_bool()).unwrap_or(false),
        link_b: v.get("linkB").and_then(|x| x.as_bool()).unwrap_or(false),
        children,
    })
}

fn execute(app: &AppHandle, job: &Job) -> RunSummary {
    let id = job.id.as_str();
    let lg = |m: &str| log_line(app, id, m);
    lg(&format!(
        "run started: \"{}\" A={} B={} mode={}",
        job.name, job.path_a, job.path_b, job.mode
    ));

    let rules = match plan::Rules::from_strs(&job.rules.only_a, &job.rules.only_b, &job.rules.diff) {
        Some(r) => r,
        None => {
            lg("failed: invalid rules");
            return outcome("failed", "invalid_rules".into(), 0, 0, 0);
        }
    };

    // 1. Both folders there, and on the same disks they were saved with.
    for (label, p) in [("A", &job.path_a), ("B", &job.path_b)] {
        if !Path::new(p).is_dir() {
            lg(&format!("skipped: folder {} not available ({})", label, p));
            return outcome("skipped", format!("path_missing:{}", label), 0, 0, 0);
        }
    }
    for (label, p, saved) in [
        ("A", &job.path_a, &job.device_a),
        ("B", &job.path_b, &job.device_b),
    ] {
        if let (Some(saved), Ok(now)) = (saved, crate::sys::device_for_path(p)) {
            if *saved != now {
                lg(&format!(
                    "skipped: folder {} is on a different disk (saved {}, now {})",
                    label, saved, now
                ));
                return outcome("skipped", format!("device_changed:{}", label), 0, 0, 0);
            }
        }
    }

    // Per-file failures are skipped, never asked about, until this returns.
    let _headless = HeadlessGuard::new();

    // 2. The real comparison. The listeners go in before it starts.
    let (tx, rx) = mpsc::channel::<(bool, String)>();
    let tx_done = tx.clone();
    let id_done = app.once("compare_complete", move |e| {
        let _ = tx_done.send((true, e.payload().to_string()));
    });
    let tx_stop = tx;
    let id_stop = app.once("compare_stopped", move |e| {
        let _ = tx_stop.send((false, e.payload().to_string()));
    });

    let mode = if job.mode == "deep" {
        crate::diff::CompareMode::Deep
    } else {
        crate::diff::CompareMode::Fast
    };
    crate::diff::compare_blocking(app, Path::new(&job.path_a), Path::new(&job.path_b), mode);
    let got = rx.recv_timeout(Duration::from_secs(3));
    app.unlisten(id_done);
    app.unlisten(id_stop);

    let payload = match got {
        Ok((true, payload)) => payload,
        Ok((false, _)) => {
            lg("failed: comparison was stopped");
            return outcome("failed", "compare_stopped".into(), 0, 0, 0);
        }
        Err(_) => {
            lg("failed: no comparison result");
            return outcome("failed", "compare_failed".into(), 0, 0, 0);
        }
    };
    let value: Value = match serde_json::from_str(&payload) {
        Ok(v) => v,
        Err(e) => {
            lg(&format!("failed: unreadable comparison result ({})", e));
            return outcome("failed", "compare_failed".into(), 0, 0, 0);
        }
    };
    let tree: Vec<plan::Node> = match value.get("tree").and_then(|t| t.as_array()) {
        Some(arr) => arr.iter().filter_map(node_from_json).collect(),
        None => {
            lg("failed: comparison result has no tree");
            return outcome("failed", "compare_failed".into(), 0, 0, 0);
        }
    };
    lg(&format!(
        "comparison done: unreadable={} errors={} aborted={}",
        value.get("unreadable").and_then(|x| x.as_u64()).unwrap_or(0),
        value.get("errors").and_then(|x| x.as_u64()).unwrap_or(0),
        value.get("aborted").and_then(|x| x.as_u64()).unwrap_or(0),
    ));

    // 3. Plan.
    let pl = plan::plan_actions(&tree, &rules);
    let mut skipped = pl.skipped.len() as u64;
    for s in &pl.skipped {
        lg(&format!("left alone: {}", s));
    }
    lg(&format!("planned {} copies", pl.actions.len()));

    // 4. Copy. Nothing is ever removed.
    let mut copied = 0u64;
    let mut failed = 0u64;
    for action in &pl.actions {
        let (src_root, dst_root, dir_label) = match action.src_side {
            plan::Side::A => (&job.path_a, &job.path_b, "A->B"),
            plan::Side::B => (&job.path_b, &job.path_a, "B->A"),
        };
        lg(&format!(
            "copy {} {}{}",
            dir_label,
            action.rel,
            if action.is_dir { "/" } else { "" }
        ));
        let res = tauri::async_runtime::block_on(crate::ops::copy_item(
            app.clone(),
            src_root.clone(),
            dst_root.clone(),
            action.rel.clone(),
            action.rel.clone(),
            false,
            None,
        ));
        for e in crate::ops::headless_errors_take() {
            lg(&format!("  file error: {}", e));
        }
        match res {
            Ok(r) => {
                copied += r.files_copied;
                failed += r.files_skipped;
                skipped += r.files_kept + r.symlinks_skipped;
                if r.status == "stopped" {
                    lg("copy stopped by the user, run ends here");
                    failed += 1;
                    break;
                }
            }
            Err(e) => {
                failed += 1;
                lg(&format!("  failed: {} {}", e.code, e.detail));
            }
        }
    }

    // 5. Verdict.
    let status = if failed == 0 {
        "ok"
    } else if copied > 0 {
        "partial"
    } else {
        "failed"
    };
    let text = format!("copied {}, skipped {}, failed {}", copied, skipped, failed);
    lg(&format!("run finished: {} ({})", status, text));
    outcome(status, text, copied, skipped, failed)
}
