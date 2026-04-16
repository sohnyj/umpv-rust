#![windows_subsystem = "windows"]

use std::env;
use std::os::windows::ffi::OsStrExt;
use std::process;

use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;

mod mpv;
mod pipe;
mod registry;

pub const DEFAULT_LOADFILE: &str = "replace";

pub fn encode_wide_string(string: &str) -> Vec<u16> {
    std::ffi::OsStr::new(string)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn parse_loadfile_arg(args: &[String]) -> Option<&str> {
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
            registry::register(parse_loadfile_arg(&args));
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

    let loadfile = parse_loadfile_arg(&args).unwrap_or(DEFAULT_LOADFILE);

    let files: Vec<String> = args
        .iter()
        .filter(|arg| arg.as_str() != "--" && !arg.starts_with("--loadfile="))
        .map(|arg| mpv::resolve_file_path(arg))
        .collect();

    let mutex = pipe::acquire_mutex();

    let (result, existing) = match pipe::send_files(&files, loadfile, false) {
        ok @ Ok(_) => (ok, true),
        Err(ERROR_FILE_NOT_FOUND) => {
            if mpv::launch_mpv().is_err() {
                pipe::release_mutex(mutex);
                process::exit(1);
            }
            (pipe::send_files(&files, loadfile, true), false)
        }
        Err(_) => {
            pipe::release_mutex(mutex);
            process::exit(1);
        }
    };

    pipe::release_mutex(mutex);

    match result {
        Ok(pid) if existing && pid != 0 => mpv::activate_mpv_window(pid),
        Err(_) => process::exit(1),
        Ok(_) => {}
    }
}
