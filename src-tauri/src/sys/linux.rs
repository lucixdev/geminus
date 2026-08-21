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

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::CmdError;

// Relative path in the shared form (segments separated by '/'), as it is born
// from a strip_prefix. Already the system form here.
pub fn rel_from_os(rel: &Path) -> String {
    rel.to_string_lossy().into_owned()
}

// Back to the form the disk understands.
pub fn join_rel(root: &Path, rel: &str) -> PathBuf {
    if rel.is_empty() {
        return root.to_path_buf();
    }
    root.join(rel)
}

// Mounted devices worth showing: real /dev/* mounts, minus squashfs and the
// system prefixes. Each entry is (name shown, path to open).
pub fn list_devices() -> Result<Vec<(String, String)>, CmdError> {
    let content = std::fs::read_to_string("/proc/mounts")
        .map_err(|e| CmdError::from_io("devices_not_listed", &e))?;
    let home = home_dir();
    let mut devices = Vec::new();
    let mut already: Vec<String> = Vec::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 { continue; }
        let dev = parts[0];
        let mount = parts[1];
        let fstype = parts[2];
        if !dev.starts_with("/dev/") { continue; }
        if fstype == "squashfs" { continue; }
        if is_system_mount(mount) { continue; }
        // What this list is for is the disks somebody plugged in. The root and
        // whatever holds the personal folder are the machine itself, and both
        // already have a button of their own beside this one.
        if holds_home(mount, &home) { continue; }
        // One disk, one entry: a filesystem with subvolumes shows the same
        // device mounted in several places, and each would look like a disk of
        // its own.
        if already.iter().any(|d| d == dev) { continue; }
        already.push(dev.to_string());
        // Last segment of the mount point, except for the root: there the last
        // segment is empty, and an entry with no name is what the picker showed.
        let name = match mount.rsplit('/').find(|s| !s.is_empty()) {
            Some(n) => n.to_string(),
            None => mount.to_string(),
        };
        devices.push((name, mount.to_string()));
    }
    Ok(devices)
}

// Mount points that belong to the system rather than to the user. Matched by
// whole segments, not by the start of the string: a disk mounted on
// /development is not /dev, and comparing text alone made it disappear from the
// list.
fn is_system_mount(mount: &str) -> bool {
    const SYSTEM_MOUNTS: &[&[&str]] = &[
        &["boot"],
        &["snap"],
        &["sys"],
        &["proc"],
        &["dev"],
        &["run", "user"],
    ];
    let segments: Vec<&str> = mount.split('/').filter(|s| !s.is_empty()).collect();
    SYSTEM_MOUNTS.iter().any(|prefix| segments.starts_with(prefix))
}

// True for the root and for whatever mount the personal folder sits on.
fn holds_home(mount: &str, home: &str) -> bool {
    if mount == "/" {
        return true;
    }
    home == mount || home.starts_with(&format!("{}/", mount))
}

// Where the folder picker opens.
pub fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
}

// The single root of the filesystem. Windows has none, hence the Option.
pub fn root_path() -> Option<String> {
    Some("/".to_string())
}

// The trail behind a path, root first, the path itself last.
// Each entry is (label shown, path to open).
pub fn crumbs(path: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = path
        .ancestors()
        .map(|anc| {
            let label = match anc.file_name() {
                Some(n) => n.to_string_lossy().into_owned(),
                None => "/".to_string(),
            };
            (label, anc.to_string_lossy().into_owned())
        })
        .collect();
    out.reverse();
    out
}

// What is above a path, None at the root.
pub fn parent_of(path: &Path) -> Option<String> {
    path.parent().map(|p| p.to_string_lossy().into_owned())
}

// Hidden the way this system means it: a leading dot.
pub fn is_hidden(path: &Path, _meta: &std::fs::Metadata) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

// Directories this system keeps for itself, on top of the shared development
// exclusions. Nothing here: the ones that matter already start with a dot.
pub fn is_system_dir(_name: &str) -> bool {
    false
}

// Makes a write-protected file writable again, so a merge overwrite can go
// through. Only the owner write bit, never group or other.
pub fn clear_write_protection(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o200);
    std::fs::set_permissions(path, perms)
}

// Removes a link without touching what it points to.
pub fn remove_link(path: &Path, _meta: &std::fs::Metadata) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

// Why an operation failed, in the stable form the frontend turns into a
// sentence. ErrorKind covers the stable cases; the rest is the errno number,
// which on this system is a kernel contract.
pub fn cause_of(e: &std::io::Error) -> &'static str {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => "permission",
        std::io::ErrorKind::NotFound => "missing",
        _ => match e.raw_os_error() {
            Some(5) => "io",                // EIO — unreadable sector
            Some(28) => "nospace",          // ENOSPC
            Some(30) => "readonly",         // EROFS
            Some(6) | Some(19) => "device", // ENXIO / ENODEV — disk gone
            _ => "other",
        },
    }
}

// Hands the item to the program the user's desktop associates with it.
pub fn open_with_default(path: &Path) -> Result<(), CmdError> {
    let output = std::process::Command::new("xdg-open")
        .arg(path)
        .output()
        .map_err(|e| CmdError::from_io("open_failed", &e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim_end().to_string();
        Err(CmdError::with_detail("no_app_for_type", stderr))
    }
}

// Identifies the storage device holding `path` (st_dev). Heuristic by nature:
// two partitions of one physical disk report different values while sharing the
// same head. None when the path cannot be read.
pub fn device_id(path: &Path) -> Option<String> {
    std::fs::metadata(path).map(|m| m.dev().to_string()).ok()
}

// The name of this system, for the few texts whose wording depends on it.
pub const SYSTEM_NAME: &str = "linux";

// The device node the health tool must be pointed at for a user path: the whole
// disk, never the partition. Longest-prefix match against the mount table; no
// privileges needed.
pub fn device_for_path(path: &str) -> Result<String, CmdError> {
    let content = std::fs::read_to_string("/proc/mounts")
        .map_err(|e| CmdError::from_io("disk_not_resolved", &e))?;
    let mut best_mount = String::new();
    let mut best_dev = String::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }
        let dev = parts[0];
        let mount = parts[1];
        if !dev.starts_with("/dev/") { continue; }
        let matches = if mount == "/" {
            true
        } else {
            path == mount || path.starts_with(&format!("{}/", mount))
        };
        if matches && mount.len() > best_mount.len() {
            best_mount = mount.to_string();
            best_dev = dev.to_string();
        }
    }
    if best_dev.is_empty() {
        Err(CmdError::with_detail("disk_not_resolved", path))
    } else {
        Ok(whole_disk(&best_dev))
    }
}

// /dev/sda1 -> /dev/sda, /dev/nvme0n1p2 -> /dev/nvme0n1, /dev/mmcblk0p1 -> /dev/mmcblk0.
fn whole_disk(dev: &str) -> String {
    let name = match dev.strip_prefix("/dev/") {
        Some(n) => n,
        None => return dev.to_string(),
    };
    if name.starts_with("nvme") || name.starts_with("mmcblk") {
        if let Some(idx) = name.rfind('p') {
            let after = &name[idx + 1..];
            if !after.is_empty() && after.chars().all(|c| c.is_ascii_digit()) {
                return format!("/dev/{}", &name[..idx]);
            }
        }
        return dev.to_string();
    }
    if name.starts_with("sd") || name.starts_with("hd") {
        return format!("/dev/{}", name.trim_end_matches(|c: char| c.is_ascii_digit()));
    }
    dev.to_string()
}

// Where the health tool is, or None if it is not installed. PATH first, then
// the directories a system package puts it in — a desktop session does not
// always carry the admin directories in PATH.
pub fn smartctl_path() -> Option<PathBuf> {
    if let Some(found) = in_path("smartctl") {
        return Some(found);
    }
    ["/usr/sbin/smartctl", "/sbin/smartctl", "/usr/local/sbin/smartctl"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

fn in_path(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

// Opens a web page in the user's browser. Nothing else is handed over: the
// caller has already checked that this is a web address.
pub fn open_url(url: &str) -> Result<(), CmdError> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| CmdError::from_io("no_browser", &e))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Matching the start of the text instead of whole segments made disks
    // disappear from the list for the sole crime of their mount point starting
    // like a system one.
    #[test]
    fn system_mounts_are_matched_by_segment() {
        for m in ["/boot", "/boot/efi", "/dev", "/proc", "/sys", "/snap", "/run/user/1000"] {
            assert!(is_system_mount(m), "{} should be kept out", m);
        }
        for m in ["/", "/home", "/development", "/bootcamp", "/snapshots", "/run/media/luca/USB",
                  "/media/luca/Backup", "/mnt/dischi/devices"] {
            assert!(!is_system_mount(m), "{} should be shown", m);
        }
    }

    // The root and the mount holding the personal folder are the machine, not
    // something somebody plugged in — and both have a button of their own.
    #[test]
    fn the_machines_own_mounts_stay_out_of_the_device_list() {
        let home = "/home/lucix";
        assert!(holds_home("/", home));
        assert!(holds_home("/home", home));
        assert!(holds_home("/home/lucix", home));
        assert!(!holds_home("/media/lucix/USB", home));
        assert!(!holds_home("/home2", home));
        assert!(!holds_home("/mnt/dati", home));
    }

    #[test]
    fn a_partition_resolves_to_the_whole_disk() {
        assert_eq!(whole_disk("/dev/sda1"), "/dev/sda");
        assert_eq!(whole_disk("/dev/sda"), "/dev/sda");
        assert_eq!(whole_disk("/dev/nvme0n1p2"), "/dev/nvme0n1");
        assert_eq!(whole_disk("/dev/nvme0n1"), "/dev/nvme0n1");
        assert_eq!(whole_disk("/dev/mmcblk0p1"), "/dev/mmcblk0");
        assert_eq!(whole_disk("/dev/mapper/casa"), "/dev/mapper/casa");
    }
}
