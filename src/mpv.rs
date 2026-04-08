use std::env;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use windows_sys::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP;
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};

use crate::pipe;
use crate::encode_wide_string;

fn check_url(string: &str) -> bool {
    let Some((prefix, _)) = string.split_once("://") else {
        return false;
    };
    !prefix.is_empty()
        && prefix
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

pub fn resolve_file_path(argument: &str) -> String {
    if check_url(argument) {
        return argument.to_string();
    }
    match std::path::absolute(argument) {
        Ok(path) => path.to_string_lossy().into_owned(),
        Err(_) => argument.to_string(),
    }
}

fn find_mpv_path() -> PathBuf {
    if let Ok(executable) = env::current_exe() {
        if let Some(directory) = executable.parent() {
            return directory.join("mpv.exe");
        }
    }
    PathBuf::from("mpv.exe")
}

pub fn launch_mpv() {
    let mpv_path = find_mpv_path();
    let mut command = Command::new(&mpv_path);
    command.arg("--profile=builtin-pseudo-gui");
    command.arg(format!("--input-ipc-server={}", pipe::PIPE_PATH));

    command.creation_flags(CREATE_NEW_PROCESS_GROUP);

    if command.spawn().is_err() {
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
