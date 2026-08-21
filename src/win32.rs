//! Win32 原生窗口：透明无边框置顶窗口 + UpdateLayeredWindow 逐像素渲染 + 鼠标穿透。
#![allow(non_snake_case, clippy::too_many_arguments)]

use std::ffi::c_void;
use windows_sys::Win32::{
    Foundation::{HINSTANCE, HWND, LRESULT, LPARAM, POINT, RECT, SIZE, WPARAM},
    Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, CombineRgn, CreateCompatibleDC,
        CreateDIBSection, CreateRectRgn, DeleteDC, DeleteObject, DIB_RGB_COLORS, SelectObject,
        AC_SRC_ALPHA, AC_SRC_OVER, RGN_OR, SetWindowRgn,
    },
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowRect,
        IsWindowVisible, RegisterClassExW, SetTimer, SetWindowPos, ShowWindow, TranslateMessage,
        UpdateLayeredWindow, CS_HREDRAW, CS_VREDRAW, MSG, SW_HIDE, SW_SHOW, SHOW_WINDOW_CMD,
        WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, HWND_TOPMOST, HWND_NOTOPMOST,
    },
};

/// 窗口消息回调（App 实现）。hwnd 用于区分多窗口。
pub trait WindowCallback {
    /// 返回 Some(lresult) 表示已处理；None 走默认处理。
    fn on_wnd_message(
        &mut self,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT>;
}

// 全局回调指针（应用级）。存 Box<&mut dyn WindowCallback> 的瘦指针。
static mut CB: *mut c_void = std::ptr::null_mut();

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let raw = CB as *mut &mut dyn WindowCallback;
    if !raw.is_null() {
        let cb: &mut &mut dyn WindowCallback = unsafe { &mut *raw };
        if let Some(r) = cb.on_wnd_message(hwnd, msg, wparam, lparam) {
            return r;
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub struct PetWindow {
    pub hwnd: HWND,
    hdc: isize,
    bmp: isize,
    old_bmp: isize,
    pub width: i32,
    pub height: i32,
    bits: *mut u8,
    pub alpha: Vec<u8>,
    /// 当前已应用区域的矩形缓存（窗口坐标，(l,t,r,b)，逐行 RLE）。
    /// 若帧的掩码与缓存一致则跳过重建。
    hits: Vec<(i32, i32, i32, i32)>,
}

pub const FRAME_TIMER: usize = 1;

/// 设置窗口消息回调（App 指针）。
pub fn set_global_callback(cb: &mut dyn WindowCallback) {
    let boxed: Box<&mut dyn WindowCallback> = Box::new(cb);
    unsafe { CB = Box::into_raw(boxed) as *mut c_void };
}

impl PetWindow {
    /// 创建透明置顶窗口。
    pub fn create(instance: HINSTANCE, on_top: bool) -> Option<PetWindow> {
        let class_name: Vec<u16> = "dsh_pet_win\0".encode_utf16().collect();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        unsafe { RegisterClassExW(&wc) };

        let mut ex_style = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        if on_top {
            ex_style |= WS_EX_TOPMOST;
        }
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            return None;
        }
        let hdc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
        if hdc.is_null() {
            return None;
        }
        let mut pet = PetWindow {
            hwnd,
            hdc: hdc as isize,
            bmp: 0,
            old_bmp: 0,
            width: 1,
            height: 1,
            bits: std::ptr::null_mut(),
            alpha: Vec::new(),
            hits: Vec::new(),
        };
        pet.resize(1, 1);
        Some(pet)
    }

    /// 重建位图（缩放档位改变时）。
    pub fn resize(&mut self, width: i32, height: i32) {
        if width < 1 || height < 1 {
            return;
        }
        if self.bmp != 0 {
            unsafe {
                SelectObject(self.hdc as _, self.old_bmp as _);
                DeleteObject(self.bmp as _);
            }
            self.bmp = 0;
        }
        let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut c_void = std::ptr::null_mut();
        let bmp = unsafe {
            CreateDIBSection(
                self.hdc as _,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            )
        };
        if bmp.is_null() {
            return;
        }
        let old = unsafe { SelectObject(self.hdc as _, bmp as _) };
        self.bmp = bmp as isize;
        self.old_bmp = old as isize;
        self.bits = bits as *mut u8;
        self.width = width;
        self.height = height;
        self.alpha = vec![0u8; (width * height) as usize];
        self.hits.clear();
    }

    pub fn show(&self) {
        unsafe { ShowWindow(self.hwnd, SW_SHOW) };
    }

    pub fn hide(&self) {
        unsafe { ShowWindow(self.hwnd, SW_HIDE) };
    }

    pub fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) != 0 }
    }

    pub fn move_to(&self, x: i32, y: i32) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    }

    pub fn set_topmost(&self, on: bool) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                if on { HWND_TOPMOST } else { HWND_NOTOPMOST },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }

    pub fn get_rect(&self) -> (i32, i32, i32, i32) {
        let mut r: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetWindowRect(self.hwnd, &mut r) };
        (r.left, r.top, r.right, r.bottom)
    }

    /// 渲染：把 src（sw×sh BGRA）缩放到窗口并 UpdateLayeredWindow。
    pub fn present(&mut self, src: &[u8], sw: usize, sh: usize, mirror: bool) {
        let dw = self.width as usize;
        let dh = self.height as usize;
        if self.bits.is_null() || src.len() < sw * sh * 4 || dw == 0 || dh == 0 {
            return;
        }
        let stride = dw * 4;
        unsafe {
            let dst = std::slice::from_raw_parts_mut(self.bits, stride * dh);
            scale_bgra(src, sw, sh, dst, dw, dh, stride, mirror);
            for y in 0..dh {
                let row = y * stride;
                for x in 0..dw {
                    self.alpha[y * dw + x] = dst[row + x * 4 + 3];
                }
            }
        }
        let size = SIZE {
            cx: self.width,
            cy: self.height,
        };
        let pt_src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            // pptDst 传 NULL：保持窗口当前位置不变（否则会把窗口移到 pptDst）
            UpdateLayeredWindow(
                self.hwnd,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
                self.hdc as _,
                &pt_src,
                0,
                &blend,
                2, // ULW_ALPHA
            );
        }
        // 每帧同步命中区域：树懒/走路等动画掩码会变，区域必须跟着变，
        // 否则旧形状以外的新帧像素点不动、旧帧位置透传错位。
        self.update_hit_region();
    }

    /// 命中测试：像素 alpha<128 穿透。
    pub fn hit_test_alpha(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return false;
        }
        self.alpha[(y * self.width + x) as usize] >= 128
    }

    /// 按当前帧 alpha 掩码重建窗口区域（SetWindowRgn）。
    ///
    /// 为什么必须做：分层窗口的命中测试把整个窗口矩形都当作"可命中"，
    /// 透明像素也一样（只要 alpha 非 0）。WM_NCHITTEST 返回 HTTRANSPARENT
    /// 只把点击转给"同一线程"的底层窗口 —— 底下是资源管理器/其他进程时
    /// 点击会被大肥鱼窗口吞掉，表现为大肥鱼周围一大片透明区域点不动文件。
    /// 用窗口区域把形状真正限死：区域外的像素在系统层面就不属于本窗口，
    /// 点击会直通到下层窗口（任意线程/进程），区域内像素走正常命中。
    /// 阈值与 hit_test_alpha 保持一致（>=128），区域按行 RLE 逐段
    /// CreateRectRgn + CombineRgn（ExtCreateRegion 在本机有兼容问题，勿用）。
    fn update_hit_region(&mut self) {
        let w = self.width as usize;
        let h = self.height as usize;
        if w == 0 || h == 0 {
            return;
        }
        let m = &self.alpha;
        let mut runs: Vec<(i32, i32, i32, i32)> = Vec::new();
        for y in 0..h {
            let row = &m[y * w..(y + 1) * w];
            let mut x = 0usize;
            while x < w {
                if row[x] < 128 {
                    x += 1;
                    continue;
                }
                let s = x;
                while x < w && row[x] >= 128 {
                    x += 1;
                }
                runs.push((s as i32, y as i32, x as i32, y as i32 + 1));
            }
        }
        if runs == self.hits {
            return; // 掩码没变（如定格帧），区域不用重建
        }
        self.hits = runs;
        if self.hits.is_empty() {
            return; // 全透明帧：保留上一个区域，等掩码恢复
        }
        unsafe {
            let mut acc: *mut c_void = std::ptr::null_mut();
            for r in &self.hits {
                let rr = CreateRectRgn(r.0, r.1, r.2, r.3);
                if rr.is_null() {
                    break;
                }
                if acc.is_null() {
                    acc = rr;
                } else {
                    CombineRgn(acc, acc, rr, RGN_OR);
                    DeleteObject(rr as _);
                }
            }
            if !acc.is_null() {
                // 成功后窗口接管 acc（由系统释放，之后不得再删除）；
                // 失败则删掉避免泄漏（区域很小，极端内存压力下才可能发生）。
                // bRedraw=0：分层窗口由本帧 UpdateLayeredWindow 负责重绘，
                // 让系统再重绘一次纯属浪费（实测 CPU 高约 20%）。
                if SetWindowRgn(self.hwnd, acc, 0) == 0 {
                    DeleteObject(acc as _);
                }
            }
        }
    }

    pub fn start_frame_timer(&self, interval_ms: u32) {
        unsafe { SetTimer(self.hwnd, FRAME_TIMER, interval_ms, None) };
    }

    pub fn set_frame_timer_interval(&self, interval_ms: u32) {
        unsafe { SetTimer(self.hwnd, FRAME_TIMER, interval_ms, None) };
    }
}

impl Drop for PetWindow {
    fn drop(&mut self) {
        if self.bmp != 0 {
            unsafe {
                SelectObject(self.hdc as _, self.old_bmp as _);
                DeleteObject(self.bmp as _);
            }
        }
        if self.hdc != 0 {
            unsafe { DeleteDC(self.hdc as _) };
        }
    }
}

/// BGRA 缩放 + 水平镜像。
fn scale_bgra(
    src: &[u8],
    sw: usize,
    sh: usize,
    dst: &mut [u8],
    dw: usize,
    dh: usize,
    dst_stride: usize,
    mirror: bool,
) {
    for y in 0..dh {
        let sy = (y as u64 * sh as u64 / dh as u64) as usize;
        let srow = sy * sw * 4;
        let drow = y * dst_stride;
        for x in 0..dw {
            let sx = if mirror {
                sw - 1 - (x as u64 * sw as u64 / dw as u64) as usize
            } else {
                (x as u64 * sw as u64 / dw as u64) as usize
            };
            let s = srow + sx * 4;
            let d = drow + x * 4;
            dst[d] = src[s];
            dst[d + 1] = src[s + 1];
            dst[d + 2] = src[s + 2];
            dst[d + 3] = src[s + 3];
        }
    }
}

/// 消息循环。
pub fn message_loop() -> i32 {
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    0
}
