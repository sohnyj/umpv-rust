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

fn parse_loadfile_arg(arguments: &[String]) -> Option<&str> {
    arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("--loadfile="))
}

fn main() {
    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwareness(
            windows_sys::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE,
        )
    };

    let arguments: Vec<String> = env::args().skip(1).collect();

    match arguments.first().map(String::as_str) {
        Some("--register") => {
            registry::register(parse_loadfile_arg(&arguments));
            return;
        }
        Some("--unregister") => {
            registry::unregister();
            return;
        }
        _ => {}
    }

    if arguments.is_empty() {
        return;
    }

    let loadfile = parse_loadfile_arg(&arguments).unwrap_or(DEFAULT_LOADFILE);

    let files: Vec<String> = arguments
        .iter()
        .filter(|argument| argument.as_str() != "--" && !argument.starts_with("--loadfile="))
        .map(|argument| mpv::resolve_file_path(argument))
        .collect();

    let Some(expected_mpv) = mpv::resolve_mpv_path() else {
        process::exit(1);
    };

    let mutex = pipe::acquire_global_mutex();

    let mut existing = false;
    let result = match pipe::open_pipe() {
        Ok(handle) => {
            if !pipe::verify_pipe_server(handle, &expected_mpv) {
                pipe::close_pipe(handle);
                pipe::release_global_mutex(mutex);
                process::exit(1);
            }
            existing = true;
            pipe::send_file_commands(handle, &files, loadfile)
        }
        Err(ERROR_FILE_NOT_FOUND) => {
            mpv::launch_mpv();
            match pipe::open_pipe_retry() {
                Ok(handle) => {
                    if !pipe::verify_pipe_server(handle, &expected_mpv) {
                        pipe::close_pipe(handle);
                        Err(())
                    } else {
                        pipe::send_file_commands(handle, &files, loadfile)
                    }
                }
                Err(_) => Err(()),
            }
        }
        Err(_) => {
            pipe::release_global_mutex(mutex);
            process::exit(1);
        }
    };

    pipe::release_global_mutex(mutex);

    if result.is_err() {
        process::exit(1);
    }

    if existing {
        mpv::activate_mpv_window();
    }
}
