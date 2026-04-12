use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, GENERIC_WRITE, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, WriteFile, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING, SECURITY_IDENTIFICATION,
    SECURITY_SQOS_PRESENT,
};
use windows_sys::Win32::System::Pipes::WaitNamedPipeW;
use windows_sys::Win32::System::Threading::{
    CreateMutexW, ReleaseMutex, WaitForSingleObject,
};

use crate::encode_wide_string;

pub const PIPE_PATH: &str = r"\\.\pipe\umpv";
const MUTEX_NAME: &str = "umpv_mutex";
const RETRY_INTERVAL_MS: u64 = 100;
const RETRY_MAX_ATTEMPTS: u32 = 50;
const MUTEX_WAIT_TIMEOUT_MS: u32 = 10_000;

fn open_pipe_handle(pipe_path_wide: &[u16]) -> HANDLE {
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

pub fn open_pipe() -> Result<HANDLE, u32> {
    let pipe_path_wide = encode_wide_string(PIPE_PATH);
    let handle = open_pipe_handle(&pipe_path_wide);
    if handle != INVALID_HANDLE_VALUE {
        return Ok(handle);
    }

    let error = unsafe { GetLastError() };
    if error == ERROR_PIPE_BUSY {
        const PIPE_BUSY_TIMEOUT_MS: u32 = 5000;
        if unsafe { WaitNamedPipeW(pipe_path_wide.as_ptr(), PIPE_BUSY_TIMEOUT_MS) } != 0 {
            let handle = open_pipe_handle(&pipe_path_wide);
            if handle != INVALID_HANDLE_VALUE {
                return Ok(handle);
            }
        }
        return Err(unsafe { GetLastError() });
    }

    Err(error)
}

pub fn open_pipe_retry() -> Result<HANDLE, u32> {
    for _ in 0..RETRY_MAX_ATTEMPTS {
        match open_pipe() {
            Ok(handle) => return Ok(handle),
            Err(ERROR_FILE_NOT_FOUND) => {
                std::thread::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS));
            }
            Err(error) => return Err(error),
        }
    }
    Err(ERROR_FILE_NOT_FOUND)
}

fn write_pipe(handle: HANDLE, data: &[u8]) -> bool {
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

pub fn send_file_commands(handle: HANDLE, files: &[String], loadfile: &str) -> Result<(), ()> {
    let mut buffer = String::new();
    for file in files {
        buffer.clear();
        buffer.push_str("raw loadfile \"");
        for character in file.chars() {
            match character {
                '\\' => buffer.push_str("\\\\"),
                '"' => buffer.push_str("\\\""),
                '\n' => buffer.push_str("\\n"),
                _ => buffer.push(character),
            }
        }
        buffer.push_str("\" ");
        buffer.push_str(loadfile);
        buffer.push('\n');
        if !write_pipe(handle, buffer.as_bytes()) {
            unsafe { CloseHandle(handle) };
            return Err(());
        }
    }
    unsafe { CloseHandle(handle) };
    Ok(())
}

pub fn acquire_global_mutex() -> HANDLE {
    let mutex_name_wide = encode_wide_string(MUTEX_NAME);
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, mutex_name_wide.as_ptr());
        if handle.is_null() {
            std::process::exit(1);
        }
        let wait_result = WaitForSingleObject(handle, MUTEX_WAIT_TIMEOUT_MS);
        if wait_result != WAIT_OBJECT_0 && wait_result != WAIT_ABANDONED {
            CloseHandle(handle);
            std::process::exit(1);
        }
        handle
    }
}

pub fn release_global_mutex(handle: HANDLE) {
    unsafe {
        ReleaseMutex(handle);
        CloseHandle(handle);
    }
}
