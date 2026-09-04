use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{FALSE, HWND, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, FindWindowExW, GetWindowThreadProcessId, IsIconic, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};
use windows_sys::core::w;

use crate::pipe;

pub(crate) enum Error {
    SpawnFailed(std::io::Error),
    Exited,
    WaitFailed,
    StartupTimedOut,
}

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL_MILLISECONDS: u32 = 5;

pub(crate) fn launch(mpv_path: &Path, file: &str) -> Result<(), Error> {
    let mpv_process = Command::new(mpv_path)
        .arg(format!("--input-ipc-server={}", pipe::path()))
        .arg("--")
        .arg(file)
        .spawn()
        .map_err(Error::SpawnFailed)?;
    unsafe { AllowSetForegroundWindow(mpv_process.id()) };
    wait_for_ipc_server(&mpv_process)
}

fn wait_for_ipc_server(mpv_process: &Child) -> Result<(), Error> {
    let timeout_at = Instant::now() + STARTUP_TIMEOUT;
    loop {
        if pipe::server_exists() {
            return Ok(());
        }
        if Instant::now() >= timeout_at {
            return Err(Error::StartupTimedOut);
        }
        match unsafe {
            WaitForSingleObject(mpv_process.as_raw_handle(), POLL_INTERVAL_MILLISECONDS)
        } {
            WAIT_TIMEOUT => {}
            WAIT_OBJECT_0 => return Err(Error::Exited),
            _ => return Err(Error::WaitFailed),
        }
    }
}

const MPV_WINDOW_CLASS_NAME: *const u16 = w!("mpv");

fn find_window(pid: u32) -> Option<HWND> {
    let mut hwnd: HWND = std::ptr::null_mut();
    loop {
        hwnd = unsafe {
            FindWindowExW(
                std::ptr::null_mut(),
                hwnd,
                MPV_WINDOW_CLASS_NAME,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return None;
        }
        let mut window_pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &raw mut window_pid) };
        if window_pid == pid {
            return Some(hwnd);
        }
    }
}

pub(crate) fn activate_window(pid: u32) {
    let Some(hwnd) = find_window(pid) else {
        return;
    };
    if unsafe { IsIconic(hwnd) } != FALSE {
        unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }
    unsafe { SetForegroundWindow(hwnd) };
}
