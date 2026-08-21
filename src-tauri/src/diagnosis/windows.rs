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

// Windows diagnosis. Two constraints shape everything here: the health tool
// only reaches a disk from an elevated process, and an elevated process cannot
// hand its output back to the unelevated one that started it — no pipe survives
// that boundary if the child sets it up. So GEMINUS relaunches itself, hidden
// and elevated, and the two halves meet on a channel the parent creates first
// and the child dials into. One consent prompt, no console window, nothing left
// behind on disk.
//
// Two jobs go this way. The quick read of both disks asks its questions and
// dies. The extended check instead lives as long as the test does: the test
// runs inside the disk, so somebody privileged has to keep asking whether it is
// over, and that somebody is the elevated half. It stays on the channel until
// the end, and the closing of that channel is how it learns to stop.

use super::{build_disk, worst_level, DeepCheckResult, DeepCheckStarted, DiagnosisResult};
use crate::CmdError;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_CANCELLED, ERROR_NO_DATA,
    ERROR_PIPE_LISTENING, GENERIC_WRITE, INVALID_HANDLE_VALUE, SYSTEMTIME, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, OPEN_EXISTING, PIPE_ACCESS_INBOUND,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_NOWAIT, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{
    ShellExecuteExW, FOLDERID_Downloads, SHGetKnownFolderPath, SEE_MASK_NOCLOSEPROCESS,
    SHELLEXECUTEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

// The hidden arguments that turn a normal start into the elevated half: one
// per job, because the two do not answer the same questions.
const HELPER_FLAG: &str = "--disk-report";
const LONGTEST_FLAG: &str = "--disk-longtest";

// The child does its work and dies; a disk that never answers must not leave
// the app waiting forever behind a consent prompt the user has already granted.
// The extended check has no such limit: it is the disk that says when it is
// done, and it can take hours.
const HELPER_TIMEOUT_MS: u32 = 120_000;
const PIPE_BUFFER: u32 = 1024 * 1024;

// How often the disk is asked whether the test is over — and how quickly the
// elevated half notices that the app is not there any more.
const POLL_SECONDS: u64 = 10;

// When the disk does not say how long its test will take, the wait still needs
// an end.
const FALLBACK_MINUTES: u32 = 60;

// Separators the tool's own output cannot contain: they are the only structure
// the channel carries.
const SECTION: &str = "\n===GEMINUS-DISK===\n";
const REPORT_MARK: &str = "\n===GEMINUS-REPORT===\n";
const ERROR_MARK: &str = "\n===GEMINUS-ERROR===\n";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ── Elevated half ────────────────────────────────────────────────────────────

// Called before anything else starts: when one of the hidden arguments is
// there, this process is the elevated half and must never become a window.
pub fn run_helper_if_requested() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == HELPER_FLAG) {
        read_both_disks(&args[i + 1..]);
        return true;
    }
    if let Some(i) = args.iter().position(|a| a == LONGTEST_FLAG) {
        run_long_test(&args[i + 1..]);
        return true;
    }
    false
}

// Channel first, then one entry per disk. A malformed call writes nothing and
// still refuses to become a window.
fn read_both_disks(args: &[String]) {
    let channel = match args.first() {
        Some(c) => c,
        None => return,
    };
    let mut report = String::new();
    for device in &args[1..] {
        report.push_str(SECTION);
        report.push_str(&run_tool(&["-a", device]).0);
    }
    let _ = send_on_channel(channel, report.as_bytes());
}

// Asks the tool one question about one disk. A failure is not an error to
// report upwards: the text it printed is itself the answer, and the shared
// reader decides what it means. The exit code comes along for the one caller
// that needs it — the code is a set of flags, not a number.
fn run_tool(args: &[&str]) -> (String, i32) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let tool = match crate::sys::smartctl_path() {
        Some(p) => p,
        None => return ("NO_DEVICE".to_string(), -1),
    };
    match Command::new(tool)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            if text.trim().is_empty() {
                text = String::from_utf8_lossy(&out.stderr).into_owned();
            }
            (text, out.status.code().unwrap_or(-1))
        }
        Err(e) => (format!("NO_DEVICE ({})", e), -1),
    }
}

// The write end of the channel. The quick read opens it once at the end; the
// extended check keeps it open for the whole test, because losing it is the
// signal to stop.
struct Channel(*mut std::ffi::c_void);

impl Channel {
    fn open(name: &str) -> Result<Self, CmdError> {
        let wide_name = wide(name);
        let handle = unsafe {
            CreateFileW(
                wide_name.as_ptr(),
                GENERIC_WRITE,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(CmdError::plain("channel_not_open"));
        }
        Ok(Channel(handle))
    }

    // False means the other end is gone. For the elevated half that is the only
    // news that ever arrives from the app.
    fn write(&self, data: &[u8]) -> bool {
        let mut written: u32 = 0;
        let ok = unsafe {
            WriteFile(
                self.0,
                data.as_ptr(),
                data.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            )
        };
        ok != 0 && written as usize == data.len()
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn send_on_channel(name: &str, data: &[u8]) -> Result<(), CmdError> {
    let channel = Channel::open(name)?;
    if channel.write(data) {
        Ok(())
    } else {
        Err(CmdError::plain("channel_write_failed"))
    }
}

// Starts the long test and stays with it. Channel first, then the disk.
fn run_long_test(args: &[String]) {
    let (channel, device) = match (args.first(), args.get(1)) {
        (Some(c), Some(d)) => (c, d),
        _ => return,
    };
    let link = match Channel::open(channel) {
        Ok(c) => c,
        Err(_) => return,
    };

    // The app checks for the tool before offering the check at all; getting
    // here without it means it went away in between, and saying so plainly
    // beats letting every later step fail one by one.
    if crate::sys::smartctl_path().is_none() {
        let _ = link.write(format!("{}tool not available", ERROR_MARK).as_bytes());
        return;
    }

    // How long the disk says it will take: the first thing the app hears, and
    // the only thing it has to show until the end.
    let (caps, _) = run_tool(&["-c", device]);
    let estimate = super::parse_extended_test_minutes(&caps);
    if !link.write(format!("EST:{}\n", estimate).as_bytes()) {
        return;
    }

    // Handing the test over to the disk firmware returns at once. Only the low
    // three bits of the code mean "the command did not go through": the others
    // report the disk's health, and a disk in bad shape is exactly the one
    // worth testing.
    let (start, code) = run_tool(&["-t", "long", device]);
    if code < 0 || code & 0b111 != 0 {
        let _ = link.write(format!("{}{}", ERROR_MARK, first_line(&start)).as_bytes());
        return;
    }

    let budget = (if estimate > 0 { estimate } else { FALLBACK_MINUTES }) as u64 * 2 * 60;
    let mut waited: u64 = 0;
    let mut finished = false;
    while waited < budget {
        std::thread::sleep(Duration::from_secs(POLL_SECONDS));
        waited += POLL_SECONDS;
        // The heartbeat carries nothing: it is here to fail. A write that fails
        // means the app closed the channel, which is how both a cancellation
        // and a closed app arrive at this end.
        if !link.write(b".") {
            let _ = run_tool(&["-X", device]);
            return;
        }
        let (status, _) = run_tool(&["-c", device]);
        // The first answer is not trusted to end anything: a disk that has not
        // started yet still reports the previous test, and that would pass for
        // a test finished in ten seconds.
        if waited > POLL_SECONDS && !self_test_running(&status) {
            finished = true;
            break;
        }
    }
    // Out of budget: a disk that never says it is done — or that says it in a
    // form this reader does not know — must not keep the app waiting forever.
    if !finished {
        let _ = run_tool(&["-X", device]);
    }

    let (full, _) = run_tool(&["-a", device]);
    let (log, _) = run_tool(&["-l", "selftest", device]);
    let _ = link.write(
        format!(
            "{}=== smartctl -a ===\n{}\n=== smartctl -l selftest ===\n{}",
            REPORT_MARK, full, log
        )
        .as_bytes(),
    );
}

// "Self-test execution status: ( 249)" — the top half of the value says the
// test is still going; anything below 240 is an outcome, good or bad. An answer
// this does not recognise counts as still going: the budget ends the wait, and
// stopping a test that had finished costs nothing.
fn self_test_running(status: &str) -> bool {
    for line in status.lines() {
        if !line.contains("Self-test execution status") {
            continue;
        }
        let (open, close) = match (line.find('('), line.find(')')) {
            (Some(o), Some(c)) if c > o => (o, c),
            _ => continue,
        };
        if let Ok(value) = line[open + 1..close].trim().parse::<u32>() {
            return value >= 240;
        }
    }
    true
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("Extended Check Did Not Start")
        .to_string()
}

// ── Unelevated half ──────────────────────────────────────────────────────────

// One name per request: two diagnoses in the same session must not meet.
fn channel_name() -> String {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    format!(
        "\\\\.\\pipe\\geminus-disk-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

// The listening end, waiting for the elevated half to dial in. The mode is the
// caller's business: one job reads once at the end and can afford to block, the
// other has to stay awake for hours.
fn create_channel(name: &str, mode: u32) -> Result<*mut std::ffi::c_void, CmdError> {
    let wide_name = wide(name);
    let pipe = unsafe {
        CreateNamedPipeW(
            wide_name.as_ptr(),
            PIPE_ACCESS_INBOUND,
            mode,
            1,
            PIPE_BUFFER,
            PIPE_BUFFER,
            0,
            std::ptr::null(),
        )
    };
    if pipe == INVALID_HANDLE_VALUE {
        return Err(CmdError::plain("channel_not_created"));
    }
    Ok(pipe)
}

// Runs the elevated half over the devices and returns what it wrote, one entry
// per device in the order asked.
fn ask_elevated(devices: &[String]) -> Result<Vec<String>, CmdError> {
    let name = channel_name();
    let pipe = create_channel(&name, PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT)?;

    let process = match launch_elevated(HELPER_FLAG, &name, devices) {
        Ok(p) => p,
        Err(e) => {
            unsafe { CloseHandle(pipe) };
            return Err(e);
        }
    };

    // Waiting on the child first, not on the channel: a child that dies before
    // dialing in would leave a wait on the channel hanging with nothing to
    // wake it. What it wrote stays in the channel until read, so nothing is
    // lost by reading afterwards.
    let waited = unsafe { WaitForSingleObject(process, HELPER_TIMEOUT_MS) };
    let mut code: u32 = 0;
    unsafe { GetExitCodeProcess(process, &mut code) };
    unsafe { CloseHandle(process) };

    if waited != WAIT_OBJECT_0 {
        unsafe { CloseHandle(pipe) };
        return Err(CmdError::plain("diagnosis_timed_out"));
    }

    // No wait for a connection here: by now the child has written and died, and
    // what it wrote stays in the channel until read. Asking to connect at this
    // point would fail precisely because the connection already happened.
    let mut raw: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let mut read: u32 = 0;
        let ok = unsafe {
            ReadFile(
                pipe,
                chunk.as_mut_ptr(),
                chunk.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            // The child closed its end: everything it wrote has been read.
            if err == ERROR_BROKEN_PIPE {
                break;
            }
            unsafe { CloseHandle(pipe) };
            // Nobody ever dialed in: the elevated half died before writing.
            if err == ERROR_PIPE_LISTENING {
                return Err(CmdError::with_detail(
                    "elevated_step_failed",
                    format!("exit {}", code),
                ));
            }
            return Err(CmdError::with_detail(
                "channel_read_failed",
                format!("Windows error {}", err),
            ));
        }
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read as usize]);
    }
    unsafe { CloseHandle(pipe) };

    let text = String::from_utf8_lossy(&raw).into_owned();
    let mut sections: Vec<String> = text.split(SECTION).skip(1).map(|s| s.to_string()).collect();
    sections.resize(devices.len(), String::new());
    Ok(sections)
}

// Asks Windows to start this same program elevated, hidden, with the arguments
// that turn it into the reading half. Consent refused arrives as a specific
// error, and is not a failure to report as one.
fn launch_elevated(
    flag: &str,
    channel: &str,
    devices: &[String],
) -> Result<*mut std::ffi::c_void, CmdError> {
    let exe = std::env::current_exe()
        .map_err(|e| CmdError::from_io("own_path_unknown", &e))?
        .to_string_lossy()
        .into_owned();

    // Device names here are ours, not the user's: they never carry a space or
    // a quote, so the arguments cannot be broken apart by what they contain.
    let params = format!("{} {} {}", flag, channel, devices.join(" "));

    let file = wide(&exe);
    let verb = wide("runas");
    let args = wide(&params);

    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = args.as_ptr();
    info.nShow = SW_HIDE as i32;

    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        if err == ERROR_CANCELLED {
            // The same word the other system uses when the user says no: the
            // frontend already knows what to do with it.
            return Err(CmdError::plain("auth_dismissed"));
        }
        return Err(CmdError::with_detail(
            "elevation_failed",
            format!("Windows error {}", err),
        ));
    }
    if info.hProcess.is_null() {
        return Err(CmdError::plain("elevation_failed"));
    }
    Ok(info.hProcess)
}

// The name the health tool uses for a disk. It has its own numbering, and given
// that name it works out the kind of disk by itself; given the system's name it
// asks to be told. The two numberings line up, disk by disk.
fn tool_name(device: &str) -> String {
    let number: Option<u32> = device
        .rsplit("PhysicalDrive")
        .next()
        .and_then(|n| n.parse().ok());
    match number {
        Some(n) if n < 26 => format!("/dev/sd{}", (b'a' + n as u8) as char),
        _ => device.to_string(),
    }
}

pub fn run_disk_diagnosis(path_a: &str, path_b: &str) -> Result<DiagnosisResult, CmdError> {
    // If the tool vanished between the check and now, no separate error is
    // needed: the elevated half writes that the disk did not answer, and the
    // shared reader turns that into "health not readable" — a true sentence the
    // user already has translated.
    let dev_a = crate::sys::device_for_path(path_a).unwrap_or_default();
    let dev_b = crate::sys::device_for_path(path_b).unwrap_or_default();

    // Two folders on one disk are one disk: asking twice would mean two reads
    // of the same drive and one wasted wait.
    let same = !dev_a.is_empty() && dev_a == dev_b;
    let mut wanted: Vec<String> = Vec::new();
    if !dev_a.is_empty() { wanted.push(tool_name(&dev_a)); }
    if !dev_b.is_empty() && !same { wanted.push(tool_name(&dev_b)); }

    let reports = if wanted.is_empty() {
        Vec::new()
    } else {
        ask_elevated(&wanted)?
    };

    let empty = String::new();
    let report_a = if dev_a.is_empty() { &empty } else { reports.first().unwrap_or(&empty) };
    let report_b = if dev_b.is_empty() {
        &empty
    } else if same {
        report_a
    } else {
        let index = if dev_a.is_empty() { 0 } else { 1 };
        reports.get(index).unwrap_or(&empty)
    };

    let disk_a = build_disk(&dev_a, report_a);
    let disk_b = build_disk(&dev_b, report_b);
    let overall = worst_level(&disk_a.level, &disk_b.level);

    Ok(DiagnosisResult { disk_a, disk_b, overall_level: overall })
}

// ── Extended check, unelevated half ──────────────────────────────────────────

// Cancelling and closing the app are the same event here: both end with the
// channel closed, which is the only thing the elevated half is watching. There
// is no process to kill — a process this one started elevated is beyond its
// reach anyway.
static DEEP_CANCELLED: AtomicBool = AtomicBool::new(false);

enum Outcome {
    Report(String),
    Cancelled,
    Failed(CmdError),
}

// Answers on the same channel as the other system: the outcome of a check
// always reaches the frontend as a `deep_check_result` event, never as the
// return value.
pub fn run_deep_check(
    app: AppHandle,
    device: &str,
    mount_path: &str,
    header: &str,
) -> Result<(), CmdError> {
    if !device.starts_with(r"\\.\PhysicalDrive") {
        return Err(CmdError::plain("invalid_device"));
    }
    let downloads = downloads_dir()?;
    let label = sanitize_label(mount_path);
    let tool_device = tool_name(device);
    let header = header.to_string();
    DEEP_CANCELLED.store(false, Ordering::Relaxed);

    std::thread::spawn(move || {
        let result = match watch_long_test(&app, &tool_device) {
            Outcome::Cancelled => DeepCheckResult {
                success: false,
                cancelled: true,
                saved_path: String::new(),
                error: None,
            },
            Outcome::Failed(error) => DeepCheckResult {
                success: false,
                cancelled: false,
                saved_path: String::new(),
                error: Some(error),
            },
            Outcome::Report(body) => match save_report(&downloads, &label, &header, &body) {
                Ok(path) => DeepCheckResult {
                    success: true,
                    cancelled: false,
                    saved_path: path,
                    error: None,
                },
                Err(error) => DeepCheckResult {
                    success: false,
                    cancelled: false,
                    saved_path: String::new(),
                    error: Some(error),
                },
            },
        };
        let _ = app.emit("deep_check_result", result);
    });
    Ok(())
}

// Follows the elevated half from the consent prompt to the report. Nothing in
// here waits for long: the loop has to stay free to notice a cancellation,
// which arrives as a flag and leaves as a closed channel.
fn watch_long_test(app: &AppHandle, tool_device: &str) -> Outcome {
    let name = channel_name();
    let pipe = match create_channel(&name, PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT) {
        Ok(p) => p,
        Err(e) => return Outcome::Failed(e),
    };
    let process = match launch_elevated(LONGTEST_FLAG, &name, &[tool_device.to_string()]) {
        Ok(p) => p,
        Err(e) => {
            unsafe { CloseHandle(pipe) };
            return Outcome::Failed(e);
        }
    };

    let mut raw: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    let mut announced = false;
    let outcome = loop {
        if DEEP_CANCELLED.load(Ordering::Relaxed) {
            break Outcome::Cancelled;
        }
        let mut read: u32 = 0;
        let ok = unsafe {
            ReadFile(
                pipe,
                chunk.as_mut_ptr(),
                chunk.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            )
        };
        if ok != 0 && read > 0 {
            raw.extend_from_slice(&chunk[..read as usize]);
            if !announced {
                if let Some(minutes) = estimate_from(&raw) {
                    let _ = app.emit(
                        "deep_check_started",
                        DeepCheckStarted {
                            device: tool_device.to_string(),
                            estimated_minutes: minutes,
                        },
                    );
                    announced = true;
                }
            }
            continue;
        }
        if ok == 0 {
            let err = unsafe { GetLastError() };
            // The elevated half closed its end: everything it wrote has been
            // read, and there is nothing more coming.
            if err == ERROR_BROKEN_PIPE {
                break read_outcome(&raw);
            }
            // Nobody has dialed in yet — normal while the elevated half is
            // still starting up, final once it is gone.
            if err == ERROR_PIPE_LISTENING {
                if exited(process) {
                    break Outcome::Failed(CmdError::plain("elevated_step_failed"));
                }
            } else if err != ERROR_NO_DATA {
                break Outcome::Failed(CmdError::with_detail(
                    "channel_read_failed",
                    format!("Windows error {}", err),
                ));
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    };

    unsafe { CloseHandle(pipe) };
    unsafe { CloseHandle(process) };
    outcome
}

fn exited(process: *mut std::ffi::c_void) -> bool {
    unsafe { WaitForSingleObject(process, 0) == WAIT_OBJECT_0 }
}

// The estimate is the first line, and the app wants it as soon as it is whole:
// half a line is a number that is still arriving.
fn estimate_from(raw: &[u8]) -> Option<u32> {
    let text = String::from_utf8_lossy(raw);
    let (line, _) = text.split_once('\n')?;
    line.strip_prefix("EST:")?.trim().parse().ok()
}

fn read_outcome(raw: &[u8]) -> Outcome {
    let text = String::from_utf8_lossy(raw);
    if let Some(at) = text.find(REPORT_MARK) {
        return Outcome::Report(text[at + REPORT_MARK.len()..].to_string());
    }
    if let Some(at) = text.find(ERROR_MARK) {
        return Outcome::Failed(CmdError::with_detail(
            "deep_check_failed",
            text[at + ERROR_MARK.len()..].trim(),
        ));
    }
    Outcome::Failed(CmdError::plain("deep_check_no_report"))
}

// The report is this half's to write, not the elevated one's: the folder
// belongs to this user, and consent may well have been given as somebody else.
// The opening lines arrive already written from the frontend, in the language
// the user is reading; the file name stays neutral, it travels between systems.
fn save_report(downloads: &str, label: &str, header: &str, body: &str) -> Result<String, CmdError> {
    let now = local_time();
    let dir = std::path::Path::new(downloads);
    std::fs::create_dir_all(dir).map_err(|e| CmdError::from_io("report_not_saved", &e))?;
    let file = dir.join(format!(
        "geminus_health_{}_{:04}{:02}{:02}_{:02}{:02}{:02}.txt",
        label, now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
    ));
    let when = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
    );
    let text = format!("{}\n\n{}", header.replace("{date}", &when), body);
    std::fs::write(&file, text).map_err(|e| CmdError::from_io("report_not_saved", &e))?;
    Ok(file.to_string_lossy().into_owned())
}

fn local_time() -> SYSTEMTIME {
    let mut now: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut now) };
    now
}

// Where the user's downloads go, asked of the system: the folder can be moved,
// and a path built by hand would keep pointing at where it used to be.
fn downloads_dir() -> Result<String, CmdError> {
    let mut raw: *mut u16 = std::ptr::null_mut();
    let hr =
        unsafe { SHGetKnownFolderPath(&FOLDERID_Downloads, 0, std::ptr::null_mut(), &mut raw) };
    if hr < 0 || raw.is_null() {
        return Err(CmdError::plain("report_not_saved"));
    }
    let len = (0..).take_while(|&i| unsafe { *raw.add(i) } != 0).count();
    let path = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(raw, len) });
    unsafe { CoTaskMemFree(raw as *const std::ffi::c_void) };
    Ok(path)
}

// A name for the file that says which disk it is about: the folder the user
// picked, or the drive letter when they picked its root.
fn sanitize_label(mount_path: &str) -> String {
    let trimmed = mount_path.trim_end_matches(['\\', '/']);
    let base = trimmed
        .rsplit(['\\', '/'])
        .find(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .trim_end_matches(':');
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if cleaned.is_empty() {
        "disk".to_string()
    } else {
        cleaned
    }
}

pub fn kill_deep_check(_app: &AppHandle) {
    DEEP_CANCELLED.store(true, Ordering::Relaxed);
}
