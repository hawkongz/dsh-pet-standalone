//! 屏幕工作区查询。
use windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;
use windows_sys::Win32::Foundation::RECT;

pub const SPI_GETWORKAREA: u32 = 0x0030;

/// 主屏工作区 (left, top, right, bottom)。
pub fn primary_work_area() -> (i32, i32, i32, i32) {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut _ as *mut _, 0);
    }
    (r.left, r.top, r.right, r.bottom)
}
