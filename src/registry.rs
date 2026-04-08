use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
use windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW;

use crate::encode_wide_string;

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

fn get_executable_path() -> String {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "umpv.exe".to_string())
}

fn set_registry_value(key: HKEY, sub_key: &str, name: Option<&str>, value: &str) -> bool {
    let sub_key_wide = encode_wide_string(sub_key);
    let value_wide = encode_wide_string(value);
    let name_wide;
    let name_pointer = match name {
        Some(name_string) => {
            name_wide = encode_wide_string(name_string);
            name_wide.as_ptr()
        }
        None => std::ptr::null(),
    };
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
            return false;
        }
        let success = RegSetValueExW(
            opened_key,
            name_pointer,
            0,
            REG_SZ,
            value_wide.as_ptr() as *const u8,
            (value_wide.len() * 2) as u32,
        ) == 0;
        RegCloseKey(opened_key);
        success
    }
}

fn enumerate_registry_values(key: HKEY, sub_key: &str) -> Vec<(String, String)> {
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
            let mut data_length: u32 = 2048;
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
            );

            if status != 0 {
                break;
            }

            if value_type == REG_SZ && name_length > 0 {
                let name = String::from_utf16_lossy(&name_buffer[..name_length as usize]);
                if name.starts_with('.') && name.len() > 1 {
                    let data_char_count = data_length as usize / 2;
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

fn delete_registry_tree(key: HKEY, sub_key: &str) {
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
    let associations =
        enumerate_registry_values(HKEY_CURRENT_USER, KEY_CAPABILITIES_FILE_ASSOCIATIONS);
    if associations.is_empty() {
        show_message_box("No mpv file associations found.\nRun 'mpv.exe --register' first.");
        std::process::exit(1);
    }

    let umpv_path = get_executable_path();
    let loadfile = loadfile.unwrap_or("replace");
    let command = format!("\"{}\" --loadfile={} -- \"%L\"", umpv_path, loadfile);
    let command_key = format!("{}\\shell\\open\\command", KEY_UMPV_PROG_ID);
    set_registry_value(HKEY_CURRENT_USER, KEY_UMPV_PROG_ID, None, "");
    set_registry_value(HKEY_CURRENT_USER, &command_key, None, &command);

    let count = associations
        .iter()
        .filter(|(extension, _)| {
            set_registry_value(
                HKEY_CURRENT_USER,
                KEY_CAPABILITIES_FILE_ASSOCIATIONS,
                Some(extension),
                UMPV_PROG_ID,
            )
        })
        .count();

    notify_shell_change();
    show_message_box(&format!(
        "umpv registered for {} file extension(s).\nloadfile: {}",
        count, loadfile
    ));
}

pub fn unregister() {
    let associations =
        enumerate_registry_values(HKEY_CURRENT_USER, KEY_CAPABILITIES_FILE_ASSOCIATIONS);

    if associations.is_empty() {
        delete_registry_tree(HKEY_CURRENT_USER, KEY_UMPV_PROG_ID);
        notify_shell_change();
        show_message_box("Nothing to unregister.");
        return;
    }

    let count = associations
        .iter()
        .filter(|(_, value)| value == UMPV_PROG_ID)
        .filter(|(extension, _)| {
            set_registry_value(
                HKEY_CURRENT_USER,
                KEY_CAPABILITIES_FILE_ASSOCIATIONS,
                Some(extension),
                MPV_PROG_ID,
            )
        })
        .count();

    delete_registry_tree(HKEY_CURRENT_USER, KEY_UMPV_PROG_ID);

    notify_shell_change();
    show_message_box(
        &format!("umpv unregistered.\n{} extension(s) restored to mpv.", count),
    );
}
