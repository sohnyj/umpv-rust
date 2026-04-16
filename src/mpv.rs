use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, ShowWindow,
    SW_RESTORE,
};

use crate::pipe;

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

pub fn launch_mpv() -> Result<(), ()> {
    let mpv_path = resolve_mpv_path().ok_or(())?;
    Command::new(&mpv_path)
        .arg(format!("--input-ipc-server={}", pipe::PIPE_PATH))
        .creation_flags(CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map_err(|_| ())?;
    Ok(())
}

unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let target_pid = lparam as u32;
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid != target_pid {
        return 1;
    }
    let mut class_name = [0u16; 16];
    let len = unsafe { GetClassNameW(hwnd, class_name.as_mut_ptr(), 16) };
    if len == 3 && class_name[..3] == [b'm' as u16, b'p' as u16, b'v' as u16] {
        if unsafe { IsIconic(hwnd) } != 0 {
            unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }
        unsafe { SetForegroundWindow(hwnd) };
        return 0;
    }
    1
}

pub fn activate_mpv_window(pid: u32) {
    unsafe { EnumWindows(Some(enum_window_callback), pid as LPARAM) };
}
