use std::os::windows::process::CommandExt;
use std::process::Command;
use windows_sys::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP;
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};

use crate::pipe;
use crate::encode_wide_string;

fn is_url(string: &str) -> bool {
    string.contains("://")
}

pub fn resolve_file_path(argument: &str) -> String {
    if is_url(argument) {
        return argument.to_string();
    }
    match std::path::absolute(argument) {
        Ok(path) => path.to_string_lossy().into_owned(),
        Err(_) => argument.to_string(),
    }
}

pub fn launch_mpv() {
    let mpv_path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("mpv.exe")))
        .unwrap_or_else(|| "mpv.exe".into());

    let status = Command::new(&mpv_path)
        .arg(format!("--input-ipc-server={}", pipe::PIPE_PATH))
        .creation_flags(CREATE_NEW_PROCESS_GROUP)
        .spawn();

    if status.is_err() {
        std::process::exit(1);
    }
}

pub fn activate_mpv_window() {
    let class_name = encode_wide_string("mpv");
    unsafe {
        let hwnd = FindWindowW(class_name.as_ptr(), std::ptr::null());
        if !hwnd.is_null() {
            SetForegroundWindow(hwnd);
        }
    }
}
