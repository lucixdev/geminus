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

// Disk health diagnosis: the pre-compare check and the standalone extended check.
// The shape is shared, what talks to the disk is per-system. Linux drives
// smartctl through a privileged shell script; Windows drives it directly from a
// second, elevated copy of the app.

use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

#[cfg(unix)]
mod linux;
#[cfg(unix)]
use linux as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

// Running extended check: the privileged child's pid, and the path of the file
// whose appearance tells it to stop. Only Linux has a process of its own to
// keep track of; on Windows the check is stopped by closing the channel, and
// this stays empty.
#[cfg_attr(windows, allow(dead_code))]
pub struct DeepCheckPid(pub Arc<Mutex<Option<(u32, String)>>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
// What the frontend actually shows: a verdict, and the figure the extended check
// needs. Sector counts and kernel log lines are deliberately absent — the
// diagnosis speaks in plain words, so a number nobody displays is only work.
pub struct DiskDiagnosis {
    pub device: String,              // whole-disk device (e.g. "/dev/sda"), empty if not resolvable
    pub level: String,               // "ok" | "warning" | "critical" | "unknown"
    pub smart_available: bool,
    pub estimated_deep_minutes: u32, // 0 if SMART unavailable or not reported
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisResult {
    pub disk_a: DiskDiagnosis,
    pub disk_b: DiskDiagnosis,
    pub overall_level: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepCheckStarted {
    pub device: String,
    pub estimated_minutes: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepCheckResult {
    pub success: bool,
    pub cancelled: bool,
    pub saved_path: String,
    pub error: Option<crate::CmdError>,
}

// ════════════════════════════════════════════════════════════
// Reading the tool's output: the text is the same everywhere, so there is one
// reader. What depends on the system is how you get hold of that text.
// ════════════════════════════════════════════════════════════

fn priority(level: &str) -> u8 {
    match level {
        "ok" => 0,
        "warning" | "unknown" => 1,
        "critical" => 2,
        _ => 1,
    }
}

pub(crate) fn worst_level(a: &str, b: &str) -> String {
    if priority(a) >= priority(b) { a.to_string() } else { b.to_string() }
}

// A line of the attribute table:
// "197 Current_Pending_Sector 0x0032 100 100 000 Old_age Always - 31".
// The raw value is the tenth column, and on some disks it carries a comment
// after the number ("31 (Average 30)"): only the number in front counts.
// Negative means the attribute is not in this report at all.
fn parse_smart_value(smart_output: &str, attr_name: &str) -> i64 {
    for line in smart_output.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 10 || cols[1] != attr_name {
            continue;
        }
        let digits: String = cols[9].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse::<i64>() {
            return n;
        }
    }
    -1
}

// The health verdict of a spinning disk or a SATA solid-state one, from the two
// attributes that say whether the surface is giving way. None when this report
// carries neither: that is not a healthy disk, it is a disk of another kind.
fn classify_ata(smart: &str) -> Option<&'static str> {
    let pending = parse_smart_value(smart, "Current_Pending_Sector");
    let reallocated = parse_smart_value(smart, "Reallocated_Sector_Ct");
    if pending < 0 && reallocated < 0 {
        return None;
    }
    if reallocated >= 1 || pending >= 10 {
        return Some("critical");
    }
    if pending >= 1 {
        return Some("warning");
    }
    Some("ok")
}

// Whatever follows the colon on a line of the health log, untouched.
fn field<'a>(smart: &'a str, name: &str) -> Option<&'a str> {
    smart.lines().find_map(|line| {
        let (label, value) = line.split_once(':')?;
        if label.trim() == name {
            Some(value.trim())
        } else {
            None
        }
    })
}

// "Available Spare: 100%", "Percentage Used: 5%" — the number in front, without
// its unit.
fn field_number(smart: &str, name: &str) -> Option<i64> {
    let raw = field(smart, name)?;
    let digits: String = raw.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

// "Critical Warning: 0x02" — the flags a disk raises about itself.
fn field_flags(smart: &str, name: &str) -> Option<u64> {
    let raw = field(smart, name)?;
    let after = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X"))?;
    let hex: String = after.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    u64::from_str_radix(&hex, 16).ok()
}

// Solid-state disks on the newer bus report none of the attributes above: their
// health lives in a log of its own. Reading only the old names meant declaring
// every one of them healthy without having looked, and they are what a laptop
// bought today has inside.
fn classify_nvme(smart: &str) -> Option<&'static str> {
    let flags = field_flags(smart, "Critical Warning");
    let spare = field_number(smart, "Available Spare");
    let threshold = field_number(smart, "Available Spare Threshold");
    let used = field_number(smart, "Percentage Used");
    let integrity = field_number(smart, "Media and Data Integrity Errors");
    if flags.is_none() && spare.is_none() && used.is_none() && integrity.is_none() {
        return None;
    }
    // A disk raising any flag about itself outranks every reading below it.
    if flags.unwrap_or(0) != 0 {
        return Some("critical");
    }
    // Spare blocks down to the threshold the disk itself declares: that is the
    // point at which it stops being able to replace what fails.
    if let (Some(s), Some(t)) = (spare, threshold) {
        if s <= t {
            return Some("critical");
        }
    }
    match integrity.unwrap_or(0) {
        n if n >= 10 => return Some("critical"),
        n if n >= 1 => return Some("warning"),
        _ => {}
    }
    // Past the endurance it was sold with the disk still works, and still
    // deserves the warning.
    if used.unwrap_or(0) >= 100 {
        return Some("warning");
    }
    Some("ok")
}

pub(crate) fn parse_extended_test_minutes(smart_output: &str) -> u32 {
    let mut next_is_polling = false;
    for line in smart_output.lines() {
        if line.contains("Extended self-test routine") {
            next_is_polling = true;
            continue;
        }
        if next_is_polling {
            if line.contains("recommended polling time") {
                if let (Some(s), Some(e)) = (line.find('('), line.find(')')) {
                    if let Ok(n) = line[s + 1..e].trim().parse::<u32>() {
                        return n;
                    }
                }
            }
            next_is_polling = false;
        }
    }
    0
}

// The verdict on one disk, from the text the tool produced for it. Answering is
// not the same as having said something: a report that carries none of the
// values this knows how to read is a disk whose health was not read, and the
// only honest verdict there is "unknown" — calling it healthy would be a
// verdict invented out of a failed reading.
pub(crate) fn build_disk(device: &str, smart: &str) -> DiskDiagnosis {
    let answered = !smart.trim().is_empty()
        && !smart.contains("NO_DEVICE")
        && !smart.contains("Unable to detect device type")
        && !smart.contains("Unknown USB bridge");
    let level = if answered {
        classify_ata(smart).or_else(|| classify_nvme(smart))
    } else {
        None
    };
    let smart_available = level.is_some();
    let estimated_deep_minutes = if smart_available { parse_extended_test_minutes(smart) } else { 0 };
    DiskDiagnosis {
        device: device.to_string(),
        level: level.unwrap_or("unknown").to_string(),
        smart_available,
        estimated_deep_minutes,
    }
}

// Whether the tool is installed, and which system this is — the steps to
// install it depend on that. The frontend gets both facts raw and writes the
// sentence, as it does for operation errors.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartctlStatus {
    pub available: bool,
    pub system: String,
}

#[tauri::command]
pub fn check_smartctl() -> SmartctlStatus {
    SmartctlStatus {
        available: crate::sys::smartctl_path().is_some(),
        system: crate::sys::SYSTEM_NAME.to_string(),
    }
}

#[tauri::command]
pub fn run_disk_diagnosis(
    path_a: String,
    path_b: String,
) -> Result<DiagnosisResult, crate::CmdError> {
    imp::run_disk_diagnosis(&path_a, &path_b)
}

// `header` is the opening of the saved report, already written in the language
// the user is reading: the report is a document for them, and no sentence meant
// for the user is composed here.
#[tauri::command]
pub fn run_deep_check(
    app: AppHandle,
    device: String,
    mount_path: String,
    header: String,
) -> Result<(), crate::CmdError> {
    imp::run_deep_check(app, &device, &mount_path, &header)
}

// On Windows this same program has a second life, elevated and windowless,
// whose only job is reading disks. It has to be recognised before anything else
// starts. Elsewhere it does not exist and this is always the real app.
#[cfg(windows)]
pub fn run_helper_if_requested() -> bool {
    imp::run_helper_if_requested()
}

#[cfg(unix)]
pub fn run_helper_if_requested() -> bool {
    false
}

#[tauri::command]
pub fn cancel_deep_check(app: AppHandle) -> Result<(), crate::CmdError> {
    kill_deep_check(&app);
    Ok(())
}

// Called on window close as well: a running extended check must not outlive
// the app.
pub fn kill_deep_check(app: &AppHandle) {
    imp::kill_deep_check(app);
}

// Reading a health report is the one piece of this that can be checked without
// a disk in front of it, and it is the piece that decides what the user is
// told: getting it wrong once meant calling every disk healthy.
#[cfg(test)]
mod tests {
    use super::*;

    const ATA_HEALTHY: &str = "\
ID# ATTRIBUTE_NAME          FLAG     VALUE WORST THRESH TYPE      UPDATED  WHEN_FAILED RAW_VALUE
  5 Reallocated_Sector_Ct   0x0033   100   100   005    Pre-fail  Always       -       0
197 Current_Pending_Sector  0x0012   100   100   000    Old_age   Always       -       0
";

    const ATA_PENDING: &str = "\
ID# ATTRIBUTE_NAME          FLAG     VALUE WORST THRESH TYPE      UPDATED  WHEN_FAILED RAW_VALUE
  5 Reallocated_Sector_Ct   0x0033   100   100   005    Pre-fail  Always       -       0
197 Current_Pending_Sector  0x0012   100   100   000    Old_age   Always       -       3
";

    // Some disks write a comment after the raw value.
    const ATA_WITH_COMMENT: &str = "\
ID# ATTRIBUTE_NAME          FLAG     VALUE WORST THRESH TYPE      UPDATED  WHEN_FAILED RAW_VALUE
  5 Reallocated_Sector_Ct   0x0033   100   100   005    Pre-fail  Always       -       12 (Average 8)
197 Current_Pending_Sector  0x0012   100   100   000    Old_age   Always       -       0
";

    const NVME_HEALTHY: &str = "\
SMART/Health Information (NVMe Log 0x02)
Critical Warning:                   0x00
Temperature:                        41 Celsius
Available Spare:                    100%
Available Spare Threshold:          10%
Percentage Used:                    3%
Data Units Read:                    12,345,678 [6.32 TB]
Media and Data Integrity Errors:    0
Error Information Log Entries:      5
";

    const NVME_SPARE_GONE: &str = "\
SMART/Health Information (NVMe Log 0x02)
Critical Warning:                   0x00
Available Spare:                    8%
Available Spare Threshold:          10%
Percentage Used:                    99%
Media and Data Integrity Errors:    0
";

    const NVME_WORN: &str = "\
SMART/Health Information (NVMe Log 0x02)
Critical Warning:                   0x00
Available Spare:                    90%
Available Spare Threshold:          10%
Percentage Used:                    112%
Media and Data Integrity Errors:    0
";

    const NVME_FLAGGED: &str = "\
SMART/Health Information (NVMe Log 0x02)
Critical Warning:                   0x04
Available Spare:                    100%
Available Spare Threshold:          10%
Percentage Used:                    2%
Media and Data Integrity Errors:    0
";

    // Answered, and said nothing this knows how to read.
    const UNREADABLE: &str = "\
smartctl 7.4 2023-08-01 r5530 [x86_64-linux] (local build)
Copyright (C) 2002-23, Bruce Allen, Christian Franke

SMART support is: Unavailable - device lacks SMART capability.
";

    fn level_of(smart: &str) -> String {
        build_disk("/dev/sda", smart).level
    }

    #[test]
    fn ata_disk_is_read_as_before() {
        assert_eq!(level_of(ATA_HEALTHY), "ok");
        assert_eq!(level_of(ATA_PENDING), "warning");
        assert_eq!(level_of(ATA_WITH_COMMENT), "critical");
    }

    #[test]
    fn nvme_disk_is_read_instead_of_assumed_healthy() {
        assert_eq!(level_of(NVME_HEALTHY), "ok");
        assert_eq!(level_of(NVME_SPARE_GONE), "critical");
        assert_eq!(level_of(NVME_WORN), "warning");
        assert_eq!(level_of(NVME_FLAGGED), "critical");
    }

    // The one that mattered: a report with nothing in it must never come back
    // as a healthy disk.
    #[test]
    fn a_report_without_readable_values_is_unknown() {
        assert_eq!(level_of(UNREADABLE), "unknown");
        assert_eq!(level_of(""), "unknown");
        assert_eq!(level_of("NO_DEVICE"), "unknown");
        assert!(!build_disk("/dev/sda", UNREADABLE).smart_available);
    }

    #[test]
    fn declared_test_length_survives_both_shapes() {
        let ata = "Extended self-test routine\nrecommended polling time: \t (  74) minutes.\n";
        assert_eq!(parse_extended_test_minutes(ata), 74);
        assert_eq!(parse_extended_test_minutes(NVME_HEALTHY), 0);
    }
}
