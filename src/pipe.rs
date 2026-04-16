use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, GENERIC_WRITE, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, WriteFile, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING, SECURITY_IDENTIFICATION,
    SECURITY_SQOS_PRESENT,
};
use windows_sys::Win32::System::Pipes::{GetNamedPipeServerProcessId, WaitNamedPipeW};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, ReleaseMutex, WaitForSingleObject,
};

use crate::encode_wide;

pub const PIPE_PATH: &str = r"\\.\pipe\umpv";
const PIPE_BUSY_TIMEOUT_MS: u32 = 5000;
const RETRY_MAX_ATTEMPTS: u32 = 50;
const RETRY_INTERVAL_MS: u64 = 100;
const MUTEX_NAME: &str = "umpv_mutex";
const MUTEX_TIMEOUT_MS: u32 = 10_000;

fn open_handle(pipe_path_wide: &[u16]) -> HANDLE {
    unsafe {
        CreateFileW(
            pipe_path_wide.as_ptr(),
            GENERIC_WRITE,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
            std::ptr::null_mut(),
        )
    }
}

fn connect(retry: bool) -> Result<HANDLE, u32> {
    let pipe_path_wide = encode_wide(PIPE_PATH);
    let max_attempts = if retry { RETRY_MAX_ATTEMPTS } else { 1 };

    for attempt in 0..max_attempts {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS));
        }

        let handle = open_handle(&pipe_path_wide);
        if handle != INVALID_HANDLE_VALUE {
            return Ok(handle);
        }

        let error = unsafe { GetLastError() };
        if error == ERROR_PIPE_BUSY {
            if unsafe { WaitNamedPipeW(pipe_path_wide.as_ptr(), PIPE_BUSY_TIMEOUT_MS) } != 0 {
                let handle = open_handle(&pipe_path_wide);
                if handle != INVALID_HANDLE_VALUE {
                    return Ok(handle);
                }
            }
            return Err(unsafe { GetLastError() });
        }

        if error != ERROR_FILE_NOT_FOUND {
            return Err(error);
        }
    }

    Err(ERROR_FILE_NOT_FOUND)
}

fn write_bytes(handle: HANDLE, data: &[u8]) -> bool {
    unsafe {
        let mut bytes_written: u32 = 0;
        WriteFile(
            handle,
            data.as_ptr(),
            data.len() as u32,
            &mut bytes_written,
            std::ptr::null_mut(),
        ) != 0
    }
}

fn get_server_pid(handle: HANDLE) -> u32 {
    let mut pid: u32 = 0;
    unsafe { GetNamedPipeServerProcessId(handle, &mut pid) };
    pid
}

fn write_commands(handle: HANDLE, files: &[String], loadfile: &str) -> bool {
    let mut buffer = String::new();
    for file in files {
        buffer.push_str("raw loadfile \"");
        for ch in file.chars() {
            match ch {
                '\\' => buffer.push_str("\\\\"),
                '"' => buffer.push_str("\\\""),
                '\n' => buffer.push_str("\\n"),
                _ => buffer.push(ch),
            }
        }
        buffer.push_str("\" ");
        buffer.push_str(loadfile);
        buffer.push('\n');
    }
    write_bytes(handle, buffer.as_bytes())
}

pub fn send_files(files: &[String], loadfile: &str, retry: bool) -> Result<u32, u32> {
    let handle = connect(retry)?;
    let pid = get_server_pid(handle);
    let ok = write_commands(handle, files, loadfile);
    unsafe { CloseHandle(handle) };
    if ok { Ok(pid) } else { Err(0) }
}

pub fn acquire_mutex() -> HANDLE {
    let mutex_name_wide = encode_wide(MUTEX_NAME);
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, mutex_name_wide.as_ptr());
        if handle.is_null() {
            std::process::exit(1);
        }
        let wait_result = WaitForSingleObject(handle, MUTEX_TIMEOUT_MS);
        if wait_result != WAIT_OBJECT_0 && wait_result != WAIT_ABANDONED {
            CloseHandle(handle);
            std::process::exit(1);
        }
        handle
    }
}

pub fn release_mutex(handle: HANDLE) {
    unsafe {
        ReleaseMutex(handle);
        CloseHandle(handle);
    }
}
