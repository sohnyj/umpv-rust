use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use windows_sys::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

use crate::pipe;
use crate::encode_wide_string;

fn is_url(string: &str) -> bool {
    string.contains("://")
}

pub fn resolve_file_path(arg: &str) -> String {
    if is_url(arg) {
        return arg.to_string();
    }
    match std::path::absolute(arg) {
        Ok(path) => path.to_string_lossy().into_owned(),
        Err(_) => arg.to_string(),
    }
}

fn resolve_mpv_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("mpv.exe")))
}

pub fn launch_mpv() {
    let Some(mpv_path) = resolve_mpv_path() else {
        std::process::exit(1);
    };

    if Command::new(&mpv_path)
        .arg(format!("--input-ipc-server={}", pipe::PIPE_PATH))
        .creation_flags(CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .is_err()
    {
        std::process::exit(1);
    }
}

pub fn activate_mpv_window() {
    let class_name = encode_wide_string("mpv");
    unsafe {
        let hwnd = FindWindowW(class_name.as_ptr(), std::ptr::null());
        if !hwnd.is_null() {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd);
        }
    }
}
