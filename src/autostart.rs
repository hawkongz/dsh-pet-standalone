//! 开机自启：Windows HKCU\Software\Microsoft\Windows\CurrentVersion\Run 注册表键。
use windows_sys::Win32::{
    Foundation::GetLastError,
    System::{
        LibraryLoader::GetModuleFileNameW,
        Registry::{
            RegOpenKeyExW, RegSetValueExW, RegDeleteValueW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
            KEY_SET_VALUE, KEY_QUERY_VALUE, REG_SZ,
        },
    },
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "dsh-pet-standalone";

fn run_key_path() -> Vec<u16> {
    RUN_KEY.encode_utf16().chain(std::iter::once(0)).collect()
}

fn value_name() -> Vec<u16> {
    VALUE_NAME.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 当前 exe 路径。
fn exe_path() -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    let mut buf = vec![0u16; 4096];
    let len = unsafe { GetModuleFileNameW(std::ptr::null_mut(), buf.as_mut_ptr(), 4096) };
    buf.truncate(len as usize);
    let os = OsString::from_wide(&buf);
    os.to_string_lossy().to_string()
}

pub fn is_enabled() -> bool {
    let path = run_key_path();
    let mut key: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut key,
        )
    };
    if status != 0 {
        return false;
    }
    let name = value_name();
    let mut buf = [0u16; 1024];
    let mut size: u32 = (buf.len() * 2) as u32;
    let r = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut u8,
            &mut size,
        )
    };
    let _ = unsafe { windows_sys::Win32::System::Registry::RegCloseKey(key) };
    r == 0
}

pub fn enable() {
    let path = run_key_path();
    let mut key: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key)
    };
    if status != 0 {
        return;
    }
    let cmd: Vec<u16> = format!("\"{}\"", exe_path())
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        RegSetValueExW(
            key,
            value_name().as_ptr(),
            0,
            REG_SZ,
            cmd.as_ptr() as *const u8,
            (cmd.len() * 2) as u32,
        );
        windows_sys::Win32::System::Registry::RegCloseKey(key);
    }
}

pub fn disable() {
    let path = run_key_path();
    let mut key: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key)
    };
    if status != 0 {
        return;
    }
    unsafe {
        RegDeleteValueW(key, value_name().as_ptr());
        windows_sys::Win32::System::Registry::RegCloseKey(key);
    }
    let _ = GetLastError;
}

pub fn set_enabled(on: bool) {
    if on {
        enable();
    } else {
        disable();
    }
}
