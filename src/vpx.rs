//! libvpx 解码器 FFI 绑定（静态链接 vpxmd.lib，仅 VP9 解码器）。
#![allow(non_camel_case_types, non_snake_case, dead_code)]

use std::ffi::{c_char, c_int, c_long, c_uint, c_void};

#[link(name = "vpxmd", kind = "static")]
extern "C" {
    pub fn vpx_codec_vp9_dx() -> *const vpx_codec_iface_t;
    pub fn vpx_codec_dec_init_ver(
        ctx: *mut vpx_codec_ctx_t,
        iface: *const vpx_codec_iface_t,
        cfg: *const vpx_codec_dec_cfg_t,
        flags: c_uint,
        ver: c_uint,
    ) -> vpx_codec_err_t;
    pub fn vpx_codec_decode(
        ctx: *mut vpx_codec_ctx_t,
        data: *const u8,
        data_sz: usize,
        user_priv: *mut c_void,
        deadline: c_long,
    ) -> vpx_codec_err_t;
    pub fn vpx_codec_get_frame(
        ctx: *mut vpx_codec_ctx_t,
        iter: *mut vpx_codec_iter_t,
    ) -> *mut vpx_image_t;
    pub fn vpx_codec_destroy(ctx: *mut vpx_codec_ctx_t) -> vpx_codec_err_t;
}

pub type vpx_codec_err_t = c_int;
pub const VPX_CODEC_OK: vpx_codec_err_t = 0;
/// VPX_DECODER_ABI_VERSION = 3 + VPX_CODEC_ABI_VERSION = 3 + (4 + VPX_IMAGE_ABI_VERSION=5) = 12
pub const VPX_DECODER_ABI_VERSION: c_uint = 12;

/// 迭代器（实际是 `const void *`）。
pub type vpx_codec_iter_t = *const c_void;

pub type vpx_codec_iface_t = c_void;

#[repr(C)]
pub struct vpx_codec_dec_cfg_t {
    pub threads: c_uint,
    pub w: c_uint,
    pub h: c_uint,
}

#[repr(C)]
pub struct vpx_codec_ctx_t {
    pub name: *const c_char,
    pub iface: *const vpx_codec_iface_t,
    pub err: vpx_codec_err_t,
    pub err_detail: *const c_char,
    pub init_flags: c_uint,
    pub config: *const c_void, // union { dec / raw }
    pub priv_: *mut c_void,
}

pub type vpx_img_fmt_t = c_int;
pub const VPX_IMG_FMT_NONE: vpx_img_fmt_t = 0;
pub const VPX_IMG_FMT_PLANAR: vpx_img_fmt_t = 0x100;
pub const VPX_IMG_FMT_I420: vpx_img_fmt_t = 0x100 | 2;
pub const VPX_IMG_FMT_HAS_ALPHA: vpx_img_fmt_t = 0x400;
pub const VPX_IMG_FMT_I420A: vpx_img_fmt_t = 0x100 | 2 | 0x400;
pub const VPX_IMG_FMT_I444: vpx_img_fmt_t = 0x100 | 6;

pub const VPX_PLANE_Y: usize = 0;
pub const VPX_PLANE_U: usize = 1;
pub const VPX_PLANE_V: usize = 2;
pub const VPX_PLANE_ALPHA: usize = 3;

/// 解码线程数：必须为 1。vendor libvpx (v1.14.1) 多线程（row-mt job queue）解码存在数据竞争，
/// 同一视频两次解码会在运动边缘随机产生不同的错误颜色；1 线程消除该问题。
/// 任何地方新建解码器都必须使用本常量（含 seek() 重建路径）。
pub const VPX_THREADS: c_uint = 1;

#[repr(C)]
pub struct vpx_image_t {
    pub fmt: vpx_img_fmt_t,
    pub cs: c_int,
    pub range: c_int,
    pub w: c_uint,
    pub h: c_uint,
    pub bit_depth: c_uint,
    pub d_w: c_uint,
    pub d_h: c_uint,
    pub r_w: c_uint,
    pub r_h: c_uint,
    pub x_chroma_shift: c_uint,
    pub y_chroma_shift: c_uint,
    pub planes: [*mut u8; 4],
    pub stride: [c_int; 4],
    pub bps: c_int,
    pub user_priv: *mut c_void,
    pub img_data: *mut u8,
    pub img_data_owner: c_int,
    pub self_allocd: c_int,
    pub fb_priv: *mut c_void,
}

/// 高层解码器封装。
pub struct Decoder {
    ctx: vpx_codec_ctx_t,
    ready: bool,
}

impl Decoder {
    pub fn new(threads: c_uint) -> Option<Decoder> {
        let mut ctx: vpx_codec_ctx_t = unsafe { std::mem::zeroed() };
        let iface = unsafe { vpx_codec_vp9_dx() };
        if iface.is_null() {
            return None;
        }
        let cfg = vpx_codec_dec_cfg_t {
            threads,
            w: 640,
            h: 360,
        };
        let err = unsafe { vpx_codec_dec_init_ver(&mut ctx, iface, &cfg, 0, VPX_DECODER_ABI_VERSION) };
        if err != VPX_CODEC_OK {
            return None;
        }
        Some(Decoder { ctx, ready: true })
    }

    /// 解码一帧（VP9 单帧数据）。成功返回 Some(image)。
    pub fn decode<'a>(&'a mut self, data: &[u8]) -> Option<&'a vpx_image_t> {
        if !self.ready || data.is_empty() {
            return None;
        }
        let err = unsafe {
            vpx_codec_decode(
                &mut self.ctx,
                data.as_ptr(),
                data.len(),
                std::ptr::null_mut(),
                0,
            )
        };
        if err != VPX_CODEC_OK {
            return None;
        }
        let mut iter: vpx_codec_iter_t = std::ptr::null();
        let img = unsafe { vpx_codec_get_frame(&mut self.ctx, &mut iter) };
        if img.is_null() {
            return None;
        }
        // SAFETY: libvpx 保证该帧在本 decode 调用后仍有效（下一次 decode 前）。
        unsafe { Some(&*img) }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        if self.ready {
            unsafe { vpx_codec_destroy(&mut self.ctx) };
            self.ready = false;
        }
    }
}

unsafe impl Send for Decoder {}
