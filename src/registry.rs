use windows_sys::Win32::Foundation::ERROR_NO_MORE_ITEMS;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
use windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW;

use crate::{encode_wide_string, DEFAULT_LOADFILE};

const KEY_CAPABILITIES_FILE_ASSOCIATIONS: &str =
    r"Software\Clients\Media\mpv\Capabilities\FileAssociations";
const KEY_UMPV_PROG_ID: &str = r"Software\Classes\io.mpv.umpv";
const UMPV_PROG_ID: &str = "io.mpv.umpv";
const MPV_PROG_ID: &str = "io.mpv.file";

fn show_message_box(text: &str) {
    let text_wide = encode_wide_string(text);
    let caption_wide = encode_wide_string("umpv");
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text_wide.as_ptr(),
            caption_wide.as_ptr(),
            0,
        );
    }
}

fn get_exe_path() -> String {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "umpv.exe".to_string())
}

fn create_or_open_key(key: HKEY, sub_key: &str) -> Option<HKEY> {
    let sub_key_wide = encode_wide_string(sub_key);
    unsafe {
        let mut opened_key: HKEY = std::ptr::null_mut();
        if RegCreateKeyExW(
            key,
            sub_key_wide.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null(),
            &mut opened_key,
            std::ptr::null_mut(),
        ) != 0
        {
            return None;
        }
        Some(opened_key)
    }
}

fn write_value(opened_key: HKEY, name: Option<&str>, data: &str) -> bool {
    let data_wide = encode_wide_string(data);
    let name_wide;
    let name_ptr = match name {
        Some(name_string) => {
            name_wide = encode_wide_string(name_string);
            name_wide.as_ptr()
        }
        None => std::ptr::null(),
    };
    unsafe {
        RegSetValueExW(
            opened_key,
            name_ptr,
            0,
            REG_SZ,
            data_wide.as_ptr() as *const u8,
            (data_wide.len() * std::mem::size_of::<u16>()) as u32,
        ) == 0
    }
}

fn set_value(key: HKEY, sub_key: &str, name: Option<&str>, data: &str) -> bool {
    let Some(opened_key) = create_or_open_key(key, sub_key) else {
        return false;
    };
    let success = write_value(opened_key, name, data);
    unsafe { RegCloseKey(opened_key) };
    success
}

fn enum_values(key: HKEY, sub_key: &str) -> Vec<(String, String)> {
    let sub_key_wide = encode_wide_string(sub_key);
    let mut results = Vec::new();
    unsafe {
        let mut opened_key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(key, sub_key_wide.as_ptr(), 0, KEY_READ, &mut opened_key) != 0 {
            return results;
        }

        let mut index: u32 = 0;
        loop {
            let mut name_buffer = [0u16; 256];
            let mut name_length: u32 = 256;
            let mut data_buffer = [0u16; 1024];
            let mut data_length = std::mem::size_of_val(&data_buffer) as u32;
            let mut value_type: u32 = 0;

            let status = RegEnumValueW(
                opened_key,
                index,
                name_buffer.as_mut_ptr(),
                &mut name_length,
                std::ptr::null_mut(),
                &mut value_type,
                data_buffer.as_mut_ptr() as *mut u8,
                &mut data_length,
            ) as u32;

            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            if status != 0 {
                index += 1;
                continue;
            }

            if value_type == REG_SZ && name_length > 0 {
                let name = String::from_utf16_lossy(&name_buffer[..name_length as usize]);
                if name.starts_with('.') && name.len() > 1 {
                    let data_char_count = data_length as usize / std::mem::size_of::<u16>();
                    let data = if data_char_count > 0 && data_buffer[data_char_count - 1] == 0 {
                        String::from_utf16_lossy(&data_buffer[..data_char_count - 1])
                    } else {
                        String::from_utf16_lossy(&data_buffer[..data_char_count])
                    };
                    results.push((name, data));
                }
            }
            index += 1;
        }
        RegCloseKey(opened_key);
    }
    results
}

fn set_assocs(exts: impl IntoIterator<Item = impl AsRef<str>>, prog_id: &str) -> usize {
    let Some(opened_key) = create_or_open_key(HKEY_CURRENT_USER, KEY_CAPABILITIES_FILE_ASSOCIATIONS)
    else {
        return 0;
    };
    let mut count = 0;
    for ext in exts {
        if write_value(opened_key, Some(ext.as_ref()), prog_id) {
            count += 1;
        }
    }
    unsafe { RegCloseKey(opened_key) };
    count
}

fn delete_tree(key: HKEY, sub_key: &str) {
    let sub_key_wide = encode_wide_string(sub_key);
    unsafe { RegDeleteTreeW(key, sub_key_wide.as_ptr()) };
}

fn notify_shell_change() {
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

pub fn register(loadfile: Option<&str>) {
    let assocs =
        enum_values(HKEY_CURRENT_USER, KEY_CAPABILITIES_FILE_ASSOCIATIONS);
    if assocs.is_empty() {
        show_message_box("No mpv file associations found.\nRun 'mpv.exe --register' first.");
        std::process::exit(1);
    }

    let umpv_path = get_exe_path();
    let loadfile = loadfile.unwrap_or(DEFAULT_LOADFILE);

    if !matches!(
        loadfile,
        "replace"
            | "append"
            | "append+play"
            | "append-play"
            | "insert-next"
            | "insert-next+play"
            | "insert-next-play"
    ) {
        show_message_box(&format!("Unsupported loadfile flag: {}", loadfile));
        std::process::exit(1);
    }

    if matches!(loadfile, "append-play" | "insert-next-play") {
        let replacement = loadfile.replacen("-play", "+play", 1);
        show_message_box(&format!(
            "Warning: '{}' is deprecated since mpv 0.42.\nUse '{}' instead.",
            loadfile, replacement
        ));
    }
    let command = format!("\"{}\" --loadfile={} -- \"%L\"", umpv_path, loadfile);
    let command_key = format!("{}\\shell\\open\\command", KEY_UMPV_PROG_ID);
    set_value(HKEY_CURRENT_USER, KEY_UMPV_PROG_ID, None, "");
    set_value(HKEY_CURRENT_USER, &command_key, None, &command);

    let count = set_assocs(assocs.iter().map(|(ext, _)| ext), UMPV_PROG_ID);

    notify_shell_change();
    show_message_box(&format!(
        "umpv registered for {} file extension(s).\nloadfile: {}",
        count, loadfile
    ));
}

pub fn unregister() {
    let assocs =
        enum_values(HKEY_CURRENT_USER, KEY_CAPABILITIES_FILE_ASSOCIATIONS);

    let umpv_assocs: Vec<_> = assocs
        .iter()
        .filter(|(_, data)| data == UMPV_PROG_ID)
        .collect();

    if umpv_assocs.is_empty() {
        show_message_box("Nothing to unregister.");
        return;
    }

    let count = set_assocs(umpv_assocs.iter().map(|(ext, _)| ext), MPV_PROG_ID);

    delete_tree(HKEY_CURRENT_USER, KEY_UMPV_PROG_ID);

    notify_shell_change();
    show_message_box(&format!(
        "umpv unregistered for {} file extension(s).",
        count
    ));
}
