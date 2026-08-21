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

mod diff;
mod ops;
mod diagnosis;
mod sys;

use std::sync::{Arc, Mutex};
use tauri::Manager;

// What a command hands back when it cannot do its job: a stable key, and the
// system's own words as the technical tail. No sentence for the user is
// composed on this side — one written here would never follow the language
// toggle, which is the same reason the per-file copy errors work this way.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdError {
    pub code: String,
    pub detail: String,
}

impl CmdError {
    pub fn plain(code: &str) -> Self {
        CmdError { code: code.to_string(), detail: String::new() }
    }

    pub fn with_detail(code: &str, detail: impl Into<String>) -> Self {
        CmdError { code: code.to_string(), detail: detail.into() }
    }

    pub fn from_io(code: &str, e: &std::io::Error) -> Self {
        // Same rule as the per-file failures: where the key already names the
        // cause, the system's words would only say it a second time.
        const SELF_EXPLAINING: &[&str] = &[
            "permission",
            "item_missing",
            "no_app_for_type",
            "not_a_web_address",
            "invalid_device",
            "no_physical_disk",
            "symlink_not_copyable",
            "source_kind_not_supported",
        ];
        let detail = if SELF_EXPLAINING.contains(&code) {
            String::new()
        } else {
            system_text(e)
        };
        CmdError { code: code.to_string(), detail }
    }
}

// What the system said, without the number the language runtime adds to it:
// "Permission denied (os error 13)" is two things, and the second is for
// whoever wrote the program, not for whoever is using it.
pub fn system_text(e: &std::io::Error) -> String {
    let full = e.to_string();
    match full.rfind(" (os error ") {
        Some(cut) if full.ends_with(')') => full[..cut].to_string(),
        _ => full,
    }
}

// On NVIDIA GPUs the WebKitGTK DMABUF renderer fails to create its buffer and
// the window comes up empty (seen on Debian 13, KDE Plasma 6). Turning that
// renderer off falls back to the classic path, harmless where it was not
// needed. Set only if the user has not set it, so an external override wins.
#[cfg(target_os = "linux")]
fn configure_webkit_env() {
  if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
  }
}

#[cfg(not(target_os = "linux"))]
fn configure_webkit_env() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // The elevated second life reads the disks and dies: no window, no commands,
  // it never reaches anything below this line.
  if diagnosis::run_helper_if_requested() {
    return;
  }
  configure_webkit_env();
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      let app_handle = app.handle().clone();
      if let Some(window) = app.get_webview_window("main") {
        window.on_window_event(move |event| {
          if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let handle = app_handle.clone();
            std::thread::spawn(move || {
              diagnosis::kill_deep_check(&handle);
              if let Some(win) = handle.get_webview_window("main") {
                let _ = win.destroy();
              }
            });
          }
        });
      }
      Ok(())
    })
    .manage(diagnosis::DeepCheckPid(Arc::new(Mutex::new(None))))
    .invoke_handler(tauri::generate_handler![
      diff::start_compare,
      diff::stop_compare,
      ops::check_exists,
      ops::copy_item,
      ops::stop_op,
      ops::submit_error_choice,
      ops::delete_item,
      ops::open_item,
      ops::browse_dir,
      ops::get_home_dir,
      ops::get_root_path,
      ops::list_devices,
      ops::get_device_for_path,
      ops::open_url,
      diagnosis::check_smartctl,
      diagnosis::run_disk_diagnosis,
      diagnosis::run_deep_check,
      diagnosis::cancel_deep_check
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_runtimes_error_number_never_reaches_the_user() {
        let e = std::io::Error::from_raw_os_error(13);
        assert!(e.to_string().contains("os error"));
        assert!(!system_text(&e).contains("os error"));
        assert!(!system_text(&e).is_empty());
    }

    #[test]
    fn a_key_that_already_names_the_cause_carries_no_tail() {
        let e = std::io::Error::from_raw_os_error(13);
        assert_eq!(CmdError::from_io("permission", &e).detail, "");
        assert!(!CmdError::from_io("delete_failed", &e).detail.is_empty());
    }
}
