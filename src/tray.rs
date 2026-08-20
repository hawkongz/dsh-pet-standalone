//! 系统托盘（Shell_NotifyIconW）。
use windows_sys::Win32::{
    Foundation::{HWND, WPARAM},
    UI::Shell::{
        NOTIFYICONDATAW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
        NIM_MODIFY, NOTIFYICON_VERSION, NOTIFYICON_VERSION_4,
    },
    UI::WindowsAndMessaging::{CreatePopupMenu, AppendMenuW, DestroyMenu, TrackPopupMenu, MF_STRING, MF_SEPARATOR, MF_CHECKED, MF_POPUP, WM_LBUTTONDBLCLK, WM_RBUTTONUP, TPM_LEFTALIGN, TPM_RIGHTBUTTON, TPM_RETURNCMD},
};

use crate::app::App;

pub const WM_TRAY: u32 = 0x0400 + 100; // 需与 app.rs 一致

pub struct Tray {
    added: bool,
}

impl Tray {
    pub fn new() -> Tray {
        Tray { added: false }
    }

    /// 添加托盘图标。icon 为 HICON。返回是否成功。
    pub fn add(&mut self, hwnd: HWND, icon: isize, tooltip: &str) -> bool {
        if icon == 0 {
            return false;
        }
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = icon as _;
        let tip: Vec<u16> = tooltip.encode_utf16().chain(std::iter::once(0)).collect();
        for (i, c) in tip.iter().take(127).enumerate() {
            nid.szTip[i] = *c;
        }
        let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
        self.added = ok != 0;
        ok != 0
    }

    pub fn remove(&mut self, hwnd: HWND) {
        if !self.added {
            return;
        }
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
        self.added = false;
    }
}

impl Default for Tray {
    fn default() -> Self {
        Tray::new()
    }
}

/// 托盘右键菜单。
pub fn show_menu(app: &mut App) {
    let menu = unsafe { CreatePopupMenu() };
    unsafe {
        AppendMenuW(menu, MF_STRING, 1001, wide("显示/隐藏"));
        AppendMenuW(menu, MF_STRING, 1002, wide("开机自启"));
        AppendMenuW(menu, MF_STRING, 1003, wide("生成新桌宠"));
    }
    // 切换角色子菜单
    let chars = crate::role::list_characters();
    if chars.len() > 1 {
        let sub_role = unsafe { CreatePopupMenu() };
        for (i, cid) in chars.iter().enumerate() {
            let checked = app.current_character == *cid;
            unsafe {
                AppendMenuW(
                    sub_role,
                    if checked { MF_STRING | MF_CHECKED } else { MF_STRING },
                    1100 + i,
                    wide(cid),
                )
            };
        }
        unsafe { AppendMenuW(menu, MF_STRING | MF_POPUP, sub_role as usize, wide("切换角色")) };
    }
    unsafe {
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, 1004, wide("退出"));
    }
    let mut p: windows_sys::Win32::Foundation::POINT = unsafe { std::mem::zeroed() };
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) };
    let hwnd = app.primary_hwnd();
    let cmd = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            p.x,
            p.y,
            0,
            hwnd,
            std::ptr::null(),
        )
    };
    unsafe { DestroyMenu(menu) };
    match cmd {
        1001 => app.toggle_all_visible(),
        1002 => {
            let on = !crate::autostart::is_enabled();
            crate::autostart::set_enabled(on);
        }
        1003 => app.spawn_pet(),
        1004 => app.quit_all(),
        _ => {
            if cmd >= 1100 && cmd < 1100 + 100 {
                let i = (cmd - 1100) as usize;
                if let Some(cid) = chars.get(i) {
                    app.switch_character(cid);
                }
            }
        }
    }
}

fn wide(s: &str) -> *const u16 {
    let v: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    let ptr = v.as_ptr();
    std::mem::forget(v);
    ptr
}
