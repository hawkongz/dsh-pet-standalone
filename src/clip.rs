//! 动画解码：libvpx 解码 VP9 主色 + alpha，合成 BGRA（premultiplied）。
use std::rc::Rc;

use crate::vpx::{self, vpx_image_t, VPX_PLANE_ALPHA, VPX_PLANE_U, VPX_PLANE_V, VPX_PLANE_Y};
use crate::webm::WebM;

pub const W: usize = 640;
pub const H: usize = 360;

/// 单个动画的逐帧解码器（素材 Rc 共享，解码器各宠物独立）。
pub struct ClipDecoder {
    pub webm: Rc<WebM>,
    color_dec: vpx::Decoder,
    alpha_dec: Option<vpx::Decoder>,
    pub cur: usize,
    // 查找表
    ytab: [i32; 256],
}

impl ClipDecoder {
    pub fn new(webm: Rc<WebM>) -> Option<ClipDecoder> {
        let color_dec = vpx::Decoder::new(4)?;
        let alpha_dec = webm.frames.iter().any(|f| f.alpha.is_some()).then(|| vpx::Decoder::new(2)).flatten();
        // 预计算 YUV->RGB 有限范围 BT.601 亮度系数
        let mut ytab = [0i32; 256];
        for i in 0..256usize {
            let y = (i as i32 - 16).max(0);
            ytab[i] = (298 * y) >> 8;
        }
        Some(ClipDecoder {
            webm,
            color_dec,
            alpha_dec,
            cur: 0,
            ytab,
        })
    }

    /// 跳到指定帧（重播时从 0 开始）。重建解码器以清空参考帧状态。
    pub fn seek(&mut self, idx: usize) {
        if self.cur == idx {
            return;
        }
        self.cur = idx;
        // 重建解码器（清空 VP9 参考帧）
        self.color_dec = vpx::Decoder::new(4).unwrap_or_else(|| vpx::Decoder::new(2).unwrap());
        if self.alpha_dec.is_some() {
            self.alpha_dec = Some(vpx::Decoder::new(2).unwrap());
        }
    }

    pub fn frame_count(&self) -> usize {
        self.webm.frames.len()
    }

    pub fn duration_ms(&self) -> u64 {
        self.webm.duration_ms()
    }

    /// 当前帧时间（秒）。
    pub fn current_time_secs(&self) -> f64 {
        if self.cur == 0 || self.webm.frames.is_empty() {
            return 0.0;
        }
        let idx = (self.cur - 1).min(self.webm.frames.len() - 1);
        self.webm.frames[idx].timecode_ms as f64 / 1000.0
    }

    /// 解码并返回当前帧 BGRA（premultiplied alpha），推进到下一帧。
    /// 返回 None 表示播完。
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        if self.cur >= self.webm.frames.len() {
            return None;
        }
        let f = self.webm.frames[self.cur].clone();
        self.cur += 1;
        let color = self.color_dec.decode(&f.video)?;
        let alpha = match &f.alpha {
            Some(a) => {
                let ad = self.alpha_dec.as_mut()?;
                ad.decode(a)
            }
            None => None,
        };
        Some(compose(color, alpha, &self.ytab))
    }
}

/// 合成 I420 主色 + 可选 alpha（灰度 I420 的 Y 平面）→ BGRA premultiplied。
fn compose(color: &vpx_image_t, alpha: Option<&vpx_image_t>, ytab: &[i32; 256]) -> Vec<u8> {
    // libvpx 输出的 w/h 是存储对齐尺寸，d_w/d_h 才是实际像素尺寸
    let w = color.d_w as usize;
    let h = color.d_h as usize;
    let mut out = vec![0u8; w * h * 4];

    let yp = color.planes[VPX_PLANE_Y];
    let up = color.planes[VPX_PLANE_U];
    let vp = color.planes[VPX_PLANE_V];
    let ys = color.stride[VPX_PLANE_Y] as usize;
    let us = color.stride[VPX_PLANE_U] as usize;
    let vs = color.stride[VPX_PLANE_V] as usize;
    let csx = color.x_chroma_shift as usize;
    let csy = color.y_chroma_shift as usize;

    let (ap, as_): (*const u8, usize) = match alpha {
        Some(a) => {
            if a.planes[VPX_PLANE_ALPHA].is_null() {
                (a.planes[VPX_PLANE_Y], a.stride[VPX_PLANE_Y] as usize)
            } else {
                (a.planes[VPX_PLANE_ALPHA], a.stride[VPX_PLANE_ALPHA] as usize)
            }
        }
        None => (std::ptr::null(), 0),
    };
    let has_alpha = alpha.is_some();

    for y in 0..h {
        let row_off = y * w * 4;
        let y_row = y * ys;
        let u_row = (y >> csy) * us;
        let v_row = (y >> csy) * vs;
        let a_row = if has_alpha { y * as_ } else { 0 };
        for x in 0..w {
            let yv = unsafe { *yp.add(y_row + x) } as usize;
            let u = unsafe { *up.add(u_row + (x >> csx)) } as usize;
            let v = unsafe { *vp.add(v_row + (x >> csx)) } as usize;
            let base = ytab[yv];
            let r = (base + ((409 * (v as i32 - 128) + 128) >> 8)).clamp(0, 255) as u8;
            let g = (base - ((100 * (u as i32 - 128) + 128) >> 8) - ((208 * (v as i32 - 128) + 128) >> 8)).clamp(0, 255) as u8;
            let b = (base + ((516 * (u as i32 - 128) + 128) >> 8)).clamp(0, 255) as u8;
            let mut a = 255u8;
            if has_alpha {
                a = unsafe { *ap.add(a_row + x) };
            }
            // premultiplied alpha
            let o = row_off + x * 4;
            out[o] = ((b as u32 * a as u32) >> 8) as u8;
            out[o + 1] = ((g as u32 * a as u32) >> 8) as u8;
            out[o + 2] = ((r as u32 * a as u32) >> 8) as u8;
            out[o + 3] = a;
        }
    }
    out
}
