//! 多桌宠生成验证：创建 App → spawn_pet → 检查窗口数量。
#![allow(dead_code, non_snake_case)]

#[path = "../assets.rs"]
mod assets;
#[path = "../state.rs"]
mod state;
#[path = "../webm.rs"]
mod webm;
#[path = "../vpx.rs"]
mod vpx;
#[path = "../clip.rs"]
mod clip;
#[path = "../config.rs"]
mod config;
#[path = "../monitor.rs"]
mod monitor;
#[path = "../autostart.rs"]
mod autostart;
#[path = "../win32.rs"]
mod win32;
#[path = "../tray.rs"]
mod tray;
#[path = "../pet.rs"]
mod pet;
#[path = "../role.rs"]
mod role;
#[path = "../app.rs"]
mod app;

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{PeekMessageW, TranslateMessage, DispatchMessageW, PM_REMOVE, MSG};
use std::time::{Duration, Instant};

static mut PET_WIN_COUNT: usize = 0;

extern "system" fn enum_cb(hwnd: windows_sys::Win32::Foundation::HWND, _lp: windows_sys::Win32::Foundation::LPARAM) -> i32 {
    let mut cls = [0u16; 256];
    let n = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, cls.as_mut_ptr(), 256) };
    let name = String::from_utf16_lossy(&cls[..n as usize]);
    if name == "dsh_pet_win" {
        unsafe { PET_WIN_COUNT += 1 };
    }
    1
}

/// 统计 dsh_pet_win 窗口数量。
fn count_pet_windows() -> usize {
    unsafe {
        PET_WIN_COUNT = 0;
        windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows(Some(enum_cb), 0);
        PET_WIN_COUNT
    }
}

/// 跑有限时长的消息循环（处理 WM_CLOSE 等）。
fn pump(ms: u64) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn main() {
    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(-4isize as _);
    }
    state::init_random();
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) } as windows_sys::Win32::Foundation::HINSTANCE;

    let mut app = app::App::new(instance);
    println!("初始宠物数: {}, 窗口数: {}", app.pets.len(), count_pet_windows());

    // 生成 2 只新宠物
    app.spawn_pet();
    app.spawn_pet();
    pump(300);
    println!("生成 2 只后宠物数: {}, 窗口数: {}", app.pets.len(), count_pet_windows());

    // 删除第 2 只宠物，检查窗口是否消失
    let hwnd2 = app.pets[1].win.hwnd;
    app.quit_pet(hwnd2);
    pump(500); // 让消息循环处理 WM_CLOSE
    println!("删除 pet[1] 后宠物数: {}, 窗口数: {}", app.pets.len(), count_pet_windows());

    // 删除全部，检查窗口是否全消失
    while !app.pets.is_empty() {
        let h = app.pets[0].win.hwnd;
        app.quit_pet(h);
    }
    pump(500);
    println!("删除全部后宠物数: {}, 窗口数: {}", app.pets.len(), count_pet_windows());

    println!("\n=== 多桌宠删除测试完成 ===");
    std::process::exit(0);
}
