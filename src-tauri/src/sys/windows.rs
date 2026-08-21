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

use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf, Prefix};
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, GetLogicalDrives, GetVolumePathNameW, FILE_SHARE_READ, FILE_SHARE_WRITE,
    IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, OPEN_EXISTING,
};
use windows_sys::Win32::System::Ioctl::{DISK_EXTENT, VOLUME_DISK_EXTENTS};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::CmdError;

// A string as this system's calls want it: UTF-16, null-terminated.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// Relative path in the shared form (segments separated by '/'): here it is born
// with backslashes and gets normalized once, at birth.
pub fn rel_from_os(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

// Back to the form the disk understands.
pub fn join_rel(root: &Path, rel: &str) -> PathBuf {
    if rel.is_empty() {
        return root.to_path_buf();
    }
    root.join(rel.replace('/', "\\"))
}

// Drives currently present. Each entry is (name shown, path to open).
pub fn list_devices() -> Result<Vec<(String, String)>, CmdError> {
    // Asking the system for the bitmap of drives that exist, instead of
    // touching each letter: reaching for an empty optical drive raises the
    // system's own "insert a disk" box, in front of an app that only wanted a
    // list.
    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        let err = unsafe { GetLastError() };
        return Err(CmdError::with_detail(
            "devices_not_listed",
            format!("Windows error {}", err),
        ));
    }
    let mut devices = Vec::new();
    for (i, letter) in (b'A'..=b'Z').enumerate() {
        if mask & (1 << i) == 0 {
            continue;
        }
        let root = format!("{}:\\", letter as char);
        devices.push((format!("{}:", letter as char), root));
    }
    Ok(devices)
}

// Where the folder picker opens.
pub fn home_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string())
}

// No single root here: the drives are the top, and the picker already reaches
// them through Devices. The button that leads to a root simply has nowhere to
// go, and hides.
pub fn root_path() -> Option<String> {
    None
}

// The trail behind a path, volume first, the path itself last.
// Each entry is (label shown, path to open).
pub fn crumbs(path: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = path
        .ancestors()
        .map(|anc| {
            let full = anc.to_string_lossy().into_owned();
            let label = match anc.file_name() {
                Some(n) => n.to_string_lossy().into_owned(),
                // The volume itself: "C:\" reads better as "C:", while a UNC
                // share keeps the whole "\\server\share".
                None => full.trim_end_matches('\\').to_string(),
            };
            (label, full)
        })
        .collect();
    out.reverse();
    out
}

// What is above a path, None at the root of a volume.
pub fn parent_of(path: &Path) -> Option<String> {
    path.parent().map(|p| p.to_string_lossy().into_owned())
}

// Hidden the way this system means it: the attribute, not the name. Catches
// $RECYCLE.BIN and the other volume-root furniture, which carry it.
pub fn is_hidden(_path: &Path, meta: &std::fs::Metadata) -> bool {
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

// Directories this system keeps for itself, on top of the shared development
// exclusions. They sit in the root of every drive and are not the user's.
// Compared case-insensitively, the way this system compares names.
pub fn is_system_dir(name: &str) -> bool {
    const SYSTEM_DIRS: &[&str] = &["$RECYCLE.BIN", "System Volume Information", "Config.Msi"];
    SYSTEM_DIRS.iter().any(|d| d.eq_ignore_ascii_case(name))
}

// Makes a write-protected file writable again, so a merge overwrite can go
// through: here it is one attribute, not a permission bit.
pub fn clear_write_protection(path: &Path) -> std::io::Result<()> {
    let mut perms = std::fs::metadata(path)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(path, perms)
}

// Removes a link without touching what it points to. A link pointing at a
// directory — a symlink or a junction — has to go the way a directory goes,
// or the system answers "access denied".
pub fn remove_link(path: &Path, meta: &std::fs::Metadata) -> std::io::Result<()> {
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    if meta.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
        std::fs::remove_dir(path)
    } else {
        std::fs::remove_file(path)
    }
}

// Why an operation failed, in the stable form the frontend turns into a
// sentence. The numbers are read first: a file held open by another program
// arrives already labelled "permission denied", and that name would lose the
// only detail that tells the user what to do about it. The same numbers mean
// something else entirely on the other system, which is why this lives here.
pub fn cause_of(e: &std::io::Error) -> &'static str {
    match e.raw_os_error() {
        Some(32) | Some(33) => return "inuse",       // SHARING / LOCK_VIOLATION
        Some(112) => return "nospace",               // DISK_FULL
        Some(19) => return "readonly",               // WRITE_PROTECT
        Some(21) | Some(433) | Some(1006) | Some(1167) => return "device",
        Some(23) | Some(1117) => return "io",        // CRC / IO_DEVICE
        _ => {}
    }
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => "permission",
        std::io::ErrorKind::NotFound => "missing",
        _ => "other",
    }
}

// Hands the item to the program Windows associates with it. Asking the shell
// directly instead of going through the command interpreter: no console window
// flashing up, and no filename mangled by a second round of parsing.
pub fn open_with_default(path: &Path) -> Result<(), CmdError> {
    let mut file: Vec<u16> = path.as_os_str().encode_wide().collect();
    file.push(0);
    let verb: Vec<u16> = "open\0".encode_utf16().collect();

    // Success is a value above 32; at or below it, the value is the error code.
    let code = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;

    match code {
        c if c > 32 => Ok(()),
        31 => Err(CmdError::plain("no_app_for_type")),
        5 => Err(CmdError::plain("permission")),
        2 | 3 => Err(CmdError::plain("item_missing")),
        c => Err(CmdError::with_detail(
            "open_failed",
            format!("Windows error {}", c),
        )),
    }
}

// Identifies the storage device holding `path`: the volume it starts from
// (drive letter, or UNC share). Same heuristic weight as st_dev on Linux — it
// cannot see two volumes carved out of one physical disk. None on a relative
// path, which has no volume to name.
pub fn device_id(path: &Path) -> Option<String> {
    let prefix = match path.components().next() {
        Some(Component::Prefix(p)) => p,
        _ => return None,
    };
    match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
            Some((letter as char).to_ascii_uppercase().to_string())
        }
        _ => Some(prefix.as_os_str().to_string_lossy().into_owned()),
    }
}

// The name of this system, for the few texts whose wording depends on it.
pub const SYSTEM_NAME: &str = "windows";

// The device the health tool must be pointed at for a user path: the whole
// physical disk, never the volume — the same constraint as on the other system.
// Both steps are asked of the system rather than guessed from the letter: a
// folder can be a volume of its own, and a volume can sit on a disk whose
// number has nothing to do with its letter.
pub fn device_for_path(path: &str) -> Result<String, CmdError> {
    let volume = volume_root(path)?;
    let number = physical_drive_number(&volume)?;
    Ok(format!("\\\\.\\PhysicalDrive{}", number))
}

// The volume a path belongs to, e.g. "D:\" — or "\\server\share\" when the
// path lives on the network.
fn volume_root(path: &str) -> Result<String, CmdError> {
    let file = wide(path);
    let mut buf = [0u16; 260];
    let ok = unsafe { GetVolumePathNameW(file.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if ok == 0 {
        return Err(CmdError::with_detail("disk_not_resolved", path));
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Ok(String::from_utf16_lossy(&buf[..len]))
}

// A volume can span more than one disk (a spanned or striped set): the extent
// list is sized for that case, and the first extent is the one we report.
#[repr(C)]
struct DiskExtents {
    header: VOLUME_DISK_EXTENTS,
    _room_for_more: [DISK_EXTENT; 15],
}

fn physical_drive_number(volume_root: &str) -> Result<u32, CmdError> {
    let trimmed = volume_root.trim_end_matches('\\');
    // A network share has no physical disk behind it that this machine can ask
    // about, and neither has a path with no volume at all.
    if trimmed.is_empty() || trimmed.starts_with("\\\\") {
        return Err(CmdError::plain("no_physical_disk"));
    }

    // Zero desired access asks about the volume without opening its data, which
    // is what keeps this call working without administrator rights.
    let name = wide(&format!("\\\\.\\{}", trimmed));
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(CmdError::with_detail("disk_not_resolved", trimmed));
    }

    let mut extents: DiskExtents = unsafe { std::mem::zeroed() };
    let mut returned: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
            std::ptr::null(),
            0,
            &mut extents as *mut DiskExtents as *mut std::ffi::c_void,
            std::mem::size_of::<DiskExtents>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    unsafe { CloseHandle(handle) };

    if ok == 0 || extents.header.NumberOfDiskExtents == 0 {
        return Err(CmdError::with_detail("disk_not_resolved", trimmed));
    }
    Ok(extents.header.Extents[0].DiskNumber)
}

// Where the health tool is, or None if it is not installed. PATH first, then
// the directory the official installer uses — its "add to PATH" box is easy to
// miss, and missing it must not look like the program is absent.
pub fn smartctl_path() -> Option<PathBuf> {
    if let Some(found) = in_path("smartctl.exe") {
        return Some(found);
    }
    ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .iter()
        .filter_map(|var| std::env::var_os(var))
        .map(|root| {
            Path::new(&root)
                .join("smartmontools")
                .join("bin")
                .join("smartctl.exe")
        })
        .find(|candidate| candidate.is_file())
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
    let target = wide(url);
    let verb = wide("open");
    let code = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(CmdError::with_detail(
            "no_browser",
            format!("Windows error {}", code),
        ))
    }
}

