#![windows_subsystem = "windows"]

use std::env;
use std::os::windows::ffi::OsStrExt;
use std::process;

use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW;

use pipe::SendError;

mod mpv;
mod pipe;
mod registry;

pub const DEFAULT_LOADFILE_MODE: &str = "replace";

pub fn encode_wide(string: &str) -> Vec<u16> {
    std::ffi::OsStr::new(string)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn show_message(text: &str) {
    let text_wide = encode_wide(text);
    let caption_wide = encode_wide("umpv");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text_wide.as_ptr(),
            caption_wide.as_ptr(),
            0,
        );
    }
}

fn parse_loadfile_mode(args: &[String]) -> Option<&str> {
    args
        .iter()
        .find_map(|arg| arg.strip_prefix("--loadfile="))
}

fn main() {
    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwareness(
            windows_sys::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE,
        )
    };

    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--register") => {
            registry::register(parse_loadfile_mode(&args));
            return;
        }
        Some("--unregister") => {
            registry::unregister();
            return;
        }
        _ => {}
    }

    if args.is_empty() {
        return;
    }

    let loadfile_mode = parse_loadfile_mode(&args).unwrap_or(DEFAULT_LOADFILE_MODE);

    let files: Vec<String> = args
        .iter()
        .filter(|arg| arg.as_str() != "--" && !arg.starts_with("--loadfile="))
        .map(|arg| mpv::resolve_file_path(arg))
        .collect();

    let Ok(_mutex) = pipe::acquire_mutex() else {
        process::exit(1);
    };

    let (result, existing) = match pipe::send_files(&files, loadfile_mode, false) {
        ok @ Ok(_) => (ok, true),
        Err(SendError::Connect(ERROR_FILE_NOT_FOUND)) => {
            if let Err(err) = mpv::launch_mpv() {
                show_message(&format!("Failed to launch mpv: {}", err));
                process::exit(1);
            }
            (pipe::send_files(&files, loadfile_mode, true), false)
        }
        Err(_) => process::exit(1),
    };

    match result {
        Ok(pid) if existing && pid != 0 => mpv::activate_mpv_window(pid),
        Err(_) => process::exit(1),
        Ok(_) => {}
    }
}
