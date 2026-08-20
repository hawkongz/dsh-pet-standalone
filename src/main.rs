//! dsh-pet 独立桌宠 —— Windows 原生 Win32 + libvpx 静态链接，单 exe。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod autostart;
mod clip;
mod config;
mod monitor;
mod pet;
mod role;
mod state;
mod tray;
mod vpx;
mod webm;
mod win32;

use std::ffi::c_void;

use windows_sys::Win32::{
    Foundation::HINSTANCE,
    Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CreateCompatibleDC, CreateDIBSection,
        DeleteDC, DeleteObject, DIB_RGB_COLORS, SelectObject,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::HiDpi::SetProcessDpiAwarenessContext,
    UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO},
};

fn main() {
    // 感知 DPI（物理像素坐标，避免缩放失真）
    unsafe {
        SetProcessDpiAwarenessContext(-4isize as _); // PER_MONITOR_AWARE_V2
    }
    state::init_random();

    let instance: HINSTANCE = unsafe { GetModuleHandleW(std::ptr::null()) } as HINSTANCE;
    let mut app = app::App::new(instance);
    if app.pets.is_empty() {
        return;
    }

    // 回调绑定
    win32::set_global_callback(&mut app);

    // 托盘图标：从当前帧生成
    let icon = generate_icon(&mut app);
    let mut tray = tray::Tray::new();
    let _ = tray.add(app.primary_hwnd(), icon, "dsh-pet 独立桌宠");

    // 消息循环
    let _ = win32::message_loop();

    app.save_all_positions();
    tray.remove(app.primary_hwnd());
}

/// 从当前渲染帧生成 32×32 托盘图标（CreateIconIndirect）。
fn generate_icon(app: &mut app::App) -> isize {
    let size = 32i32;
    let (dw, dh) = (size as usize, size as usize);

    let pet = match app.pets.first() {
        Some(p) => p,
        None => return 0,
    };
    // 取当前帧 BGRA 缩略（从 render_buf）
    let (sw, sh) = (pet.render_buf.len() / ((clip::H + 30) * 4), clip::H + 30);
    let thumb = make_thumb(&pet.render_buf, sw, sh, dw, dh);

    let hdc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
    if hdc.is_null() {
        return 0;
    }

    // 彩色 32bpp 位图
    let mut cbmi: BITMAPINFO = unsafe { std::mem::zeroed() };
    cbmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    cbmi.bmiHeader.biWidth = size;
    cbmi.bmiHeader.biHeight = -size;
    cbmi.bmiHeader.biPlanes = 1;
    cbmi.bmiHeader.biBitCount = 32;
    cbmi.bmiHeader.biCompression = BI_RGB;
    let mut color_bits: *mut c_void = std::ptr::null_mut();
    let color_bmp = unsafe {
        CreateDIBSection(hdc, &cbmi, DIB_RGB_COLORS, &mut color_bits, std::ptr::null_mut(), 0)
    };

    // 1bpp mask 位图
    let mut mbmi: BITMAPINFO = unsafe { std::mem::zeroed() };
    mbmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    mbmi.bmiHeader.biWidth = size;
    mbmi.bmiHeader.biHeight = -size;
    mbmi.bmiHeader.biPlanes = 1;
    mbmi.bmiHeader.biBitCount = 1;
    let mut mask_bits: *mut c_void = std::ptr::null_mut();
    let mask_bmp = unsafe {
        CreateDIBSection(hdc, &mbmi, DIB_RGB_COLORS, &mut mask_bits, std::ptr::null_mut(), 0)
    };

    if color_bmp.is_null() || mask_bmp.is_null() {
        if !color_bmp.is_null() {
            unsafe { DeleteObject(color_bmp) };
        }
        if !mask_bmp.is_null() {
            unsafe { DeleteObject(mask_bmp) };
        }
        unsafe { DeleteDC(hdc) };
        return 0;
    }

    // 填充彩色位图（BGRA premultiplied）
    unsafe {
        let cb = std::slice::from_raw_parts_mut(color_bits as *mut u8, dw * dh * 4);
        for (i, px) in thumb.iter().enumerate() {
            cb[i] = *px;
        }
        // 填充 mask：alpha>0 → 位 0（不透明）
        let stride = ((size + 31) / 32 * 4) as usize;
        let mb = std::slice::from_raw_parts_mut(mask_bits as *mut u8, stride * dh);
        for y in 0..dh {
            for x in 0..dw {
                let a = thumb[(y * dw + x) * 4 + 3];
                if a >= 128 {
                    let byte = y * stride + x / 8;
                    mb[byte] |= 0x80 >> (x % 8);
                }
            }
        }
    }

    let info = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask_bmp,
        hbmColor: color_bmp,
    };
    let icon = unsafe { CreateIconIndirect(&info) };

    unsafe {
        DeleteObject(color_bmp);
        DeleteObject(mask_bmp);
        DeleteDC(hdc);
    }
    icon as isize
}

/// 双线性近似缩略图（BGRA）。
fn make_thumb(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    for y in 0..dh {
        let sy = (y as u64 * sh as u64 / dh as u64) as usize;
        for x in 0..dw {
            let sx = (x as u64 * sw as u64 / dw as u64) as usize;
            let s = sy * sw * 4 + sx * 4;
            let d = y * dw * 4 + x * 4;
            out[d] = src[s];
            out[d + 1] = src[s + 1];
            out[d + 2] = src[s + 2];
            out[d + 3] = src[s + 3];
        }
    }
    out
}
