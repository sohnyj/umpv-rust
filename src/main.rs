#![windows_subsystem = "windows"]

use std::env;
use std::path::PathBuf;
use std::process;

use windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW;
use windows_sys::core::w;

mod lock;
mod mpv;
mod pipe;
mod registry;

fn encode_wide(string: &str) -> Vec<u16> {
    string.encode_utf16().chain(std::iter::once(0)).collect()
}

fn show_message(text: &str) {
    let text_wide = encode_wide(text);
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text_wide.as_ptr(), w!("umpv"), 0);
    }
}

fn show_information(text: &str) {
    show_message(&format!("Info\n{text}"));
}

fn error_exit(text: &str) -> ! {
    show_message(&format!("Error\n{text}"));
    process::exit(1);
}

enum Command {
    Register,
    Unregister,
}

fn parse_command(option: &str) -> Option<Command> {
    match option {
        "--register" => Some(Command::Register),
        "--unregister" => Some(Command::Unregister),
        _ => None,
    }
}

fn find_command(options: &[String]) -> Option<Command> {
    options.iter().find_map(|option| parse_command(option))
}

fn is_known_option(option: &str) -> bool {
    parse_command(option).is_some() || option.starts_with(LOADFILE_OPTION_PREFIX)
}

fn find_unknown_option(options: &[String]) -> Option<&str> {
    options
        .iter()
        .map(String::as_str)
        .find(|option| !is_known_option(option))
}

struct Arguments {
    options: Vec<String>,
    files: Vec<String>,
}

fn split_arguments(arguments: impl IntoIterator<Item = String>) -> Arguments {
    let mut options = Vec::new();
    let mut files = Vec::new();
    let mut past_end_of_options = false;

    for argument in arguments {
        if past_end_of_options || !argument.starts_with("--") {
            files.push(argument);
        } else if argument == "--" {
            past_end_of_options = true;
        } else {
            options.push(argument);
        }
    }

    Arguments { options, files }
}

fn has_url_scheme(argument: &str) -> bool {
    let Some((scheme, _)) = argument.split_once("://") else {
        return false;
    };
    scheme.starts_with(|character: char| character.is_ascii_alphabetic())
        && scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

fn absolute_file_path(file: &str) -> String {
    match std::path::absolute(file) {
        Ok(path) => path.to_string_lossy().into_owned(),
        Err(error) => error_exit(&format!("Failed to make the file path absolute: {error}")),
    }
}

fn first_non_empty_file(files: &[String]) -> Option<&str> {
    files
        .iter()
        .map(String::as_str)
        .find(|file| !file.is_empty())
}

const LOADFILE_OPTION_PREFIX: &str = "--loadfile=";
const DEFAULT_LOADFILE_FLAGS: &str = "replace";

fn loadfile_flags_or_default(options: &[String]) -> &str {
    options
        .iter()
        .find_map(|option| option.strip_prefix(LOADFILE_OPTION_PREFIX))
        .unwrap_or(DEFAULT_LOADFILE_FLAGS)
}

fn is_supported_loadfile_flags(loadfile_flags: &str) -> bool {
    matches!(
        loadfile_flags,
        "replace" | "append" | "append+play" | "insert-next" | "insert-next+play"
    )
}

fn umpv_path() -> PathBuf {
    let Ok(path) = env::current_exe() else {
        error_exit("Failed to locate umpv.exe.");
    };
    path
}

fn register(loadfile_flags: &str) {
    let command = format!(
        "\"{}\" {LOADFILE_OPTION_PREFIX}{loadfile_flags} -- \"%L\"",
        umpv_path().display()
    );

    match registry::register(&command) {
        Ok(extension_count) => show_information(&format!(
            "Registered for {extension_count} file extension(s).\nloadfile: {loadfile_flags}"
        )),
        Err(registry::Error::NoAssociations) => {
            error_exit("No mpv file associations found.\nRun 'mpv.exe --register' first.")
        }
        Err(registry::Error::ProgIdWriteFailed) => {
            error_exit("Failed to write umpv ProgID to registry.")
        }
        Err(registry::Error::NoExtensionsRegistered) => {
            error_exit("Failed to register any file associations.")
        }
    }
}

fn unregister() {
    let registry::Unregistered {
        extension_count,
        removed_prog_id,
    } = registry::unregister();

    match (extension_count, removed_prog_id) {
        (0, false) => show_information("Nothing to unregister."),
        (0, true) => {
            show_information("Removed the umpv ProgID.\nNo file extensions were pointing at umpv.")
        }
        _ => show_information(&format!(
            "Unregistered for {extension_count} file extension(s)."
        )),
    }
}

fn launch_mpv(file: &str) {
    let mpv_path = umpv_path().with_file_name("mpv.exe");
    match mpv::launch(&mpv_path, file) {
        Ok(()) => {}
        Err(mpv::Error::SpawnFailed(error)) => {
            error_exit(&format!("Failed to launch mpv.exe: {error}"))
        }
        Err(mpv::Error::Exited) => error_exit("mpv.exe exited before it opened the file."),
        Err(mpv::Error::StartupTimedOut) => error_exit("Timed out waiting for mpv.exe to start."),
    }
}

fn open_in_mpv(file: &str, loadfile_flags: &str) -> Option<u32> {
    let _lock_guard = match lock::acquire() {
        Ok(guard) => guard,
        Err(lock::Error::CreateFailed) => error_exit("Failed to create umpv lock."),
        Err(lock::Error::WaitFailed) => error_exit("Failed to wait for umpv lock."),
        Err(lock::Error::TimedOut) => {
            error_exit("Timed out waiting for umpv lock.\nAnother umpv instance is holding it.")
        }
    };

    match pipe::send_loadfile(file, loadfile_flags) {
        Ok(pid) => pid,
        Err(pipe::Error::NoServer) => {
            launch_mpv(file);
            None
        }
        Err(pipe::Error::ConnectFailed) => error_exit("Failed to connect to mpv."),
        Err(pipe::Error::WriteFailed) => error_exit("Failed to send the file to mpv."),
    }
}

fn open(files: &[String], loadfile_flags: &str) {
    let Some(file) = first_non_empty_file(files) else {
        return;
    };
    if has_url_scheme(file) {
        error_exit("URLs are not supported.\nOnly local files can be opened.");
    }
    let file = absolute_file_path(file);

    if let Some(pid) = open_in_mpv(&file, loadfile_flags) {
        mpv::activate_window(pid);
    }
}

fn main() {
    let arguments = split_arguments(env::args().skip(1));
    if let Some(option) = find_unknown_option(&arguments.options) {
        error_exit(&format!("Unknown option: {option}"));
    }
    let loadfile_flags = loadfile_flags_or_default(&arguments.options);
    if !is_supported_loadfile_flags(loadfile_flags) {
        error_exit(&format!("Unsupported loadfile flags: {loadfile_flags}"));
    }

    match find_command(&arguments.options) {
        Some(Command::Register) => register(loadfile_flags),
        Some(Command::Unregister) => unregister(),
        None => open(&arguments.files, loadfile_flags),
    }
}
