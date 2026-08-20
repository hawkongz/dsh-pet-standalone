//! 单只桌宠：窗口 + 动画状态机 + 交互 + 渲染。
#![allow(non_snake_case, dead_code)]

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use windows_sys::Win32::{
    Foundation::{POINT, LPARAM, LRESULT, WPARAM},
    UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
    UI::WindowsAndMessaging::{
        CreatePopupMenu, GetCursorPos, TrackPopupMenu, AppendMenuW,
        DestroyMenu, PostQuitMessage,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONUP, HTCLIENT, HTTRANSPARENT,
        TPM_RIGHTBUTTON, TPM_RETURNCMD, MF_POPUP, MF_STRING, MF_CHECKED, MF_SEPARATOR,
        WM_NCHITTEST, WM_TIMER,
    },
};

use crate::clip::{ClipDecoder, W, H};
use crate::config::PetConfig;
use crate::role::RoleAssets;
use crate::state::{self, Category};
use crate::win32::{PetWindow, FRAME_TIMER};

// 菜单 ID
pub const MID_ACT_BASE: usize = 100;
pub const MID_MOVE_BASE: usize = 150;
pub const MID_CLICK_BASE: usize = 160;
pub const MID_IDLE_BASE: usize = 170; // 待机二级菜单（多视频）
pub const MID_TURN_BASE: usize = 180; // 转向二级菜单（多视频）
pub const MID_CORNER: usize = 190;
pub const MID_ONTOP: usize = 191;
pub const MID_NOMOVE: usize = 192;
pub const MID_AUTOSTART: usize = 193;
pub const MID_SCALE_BASE: usize = 200;
pub const MID_ROLE_BASE: usize = 210; // 切换角色子菜单
pub const MID_SPAWN: usize = 300;
pub const MID_QUIT_PET: usize = 301;

#[derive(Clone)]
struct MovePlan {
    start_x: i32,
    target_x: i32,
    y: i32,
    duration_ms: u64,
}

pub struct Pet {
    pub id: usize,
    pub win: PetWindow,
    pub role_id: String,
    pub cats: Category,
    pub clips: HashMap<String, ClipDecoder>,
    pub cur_anim: String,
    pub facing_right: bool,
    pub scale: f64,
    pub no_move: bool,
    pub win_topmost: bool,
    pub visible: bool,
    pub render_buf: Vec<u8>,
    pub frame_accum_ms: u64,
    pub anim_ended_fired: bool,

    press_global: Option<(i32, i32)>,
    grab_offset: Option<(i32, i32)>,
    dragging: bool,
    just_dragged: bool,

    move_plan: Option<MovePlan>,
    move_accum_ms: u64,

    last_tick: Instant,
}

/// 从角色素材集构建解码器 + 动态分类。
fn build_from_role(role: &RoleAssets) -> (HashMap<String, ClipDecoder>, Category) {
    let mut clips: HashMap<String, ClipDecoder> = HashMap::new();
    for (name, wm) in &role.videos {
        if let Some(dec) = ClipDecoder::new(wm.clone()) {
            clips.insert(name.clone(), dec);
        }
    }
    // 无子目录（flat，如内置 shenshen）时 folder_files 传 None → 走"无子目录"分类分支
    let folder_files = if role.folder_files.is_empty() {
        None
    } else {
        Some(&role.folder_files)
    };
    let cats = state::build_categories(&role.names, role.manifest.as_ref(), folder_files);
    (clips, cats)
}

impl Pet {
    pub fn new(
        id: usize,
        win: PetWindow,
        role: &Rc<RoleAssets>,
        pc: &PetConfig,
    ) -> Pet {
        let (clips, cats) = build_from_role(role);
        let mut pet = Pet {
            id,
            win,
            role_id: role.id.clone(),
            cats,
            clips,
            cur_anim: String::new(),
            facing_right: pc.facing == "right",
            scale: pc.scale,
            no_move: pc.no_move,
            win_topmost: pc.on_top,
            visible: true,
            render_buf: vec![0u8; W * (H + 30) * 4],
            frame_accum_ms: 0,
            anim_ended_fired: false,
            press_global: None,
            grab_offset: None,
            dragging: false,
            just_dragged: false,
            move_plan: None,
            move_accum_ms: 0,
            last_tick: Instant::now(),
        };
        let (w, h) = pet.window_size();
        pet.win.resize(w, h);
        pet.win.set_topmost(pc.on_top);
        let idle = pet.cats.idle.clone().unwrap_or_default();
        pet.switch_anim(&idle);
        pet.win.show();
        pet.win.start_frame_timer(10);
        pet
    }

    /// 切换角色：重建解码器 + 分类，回到待机。
    pub fn apply_role(&mut self, role: &Rc<RoleAssets>) {
        let (clips, cats) = build_from_role(role);
        self.role_id = role.id.clone();
        self.clips = clips;
        self.cats = cats;
        let idle = self.cats.idle.clone().unwrap_or_default();
        self.switch_anim(&idle);
    }

    pub fn window_size(&self) -> (i32, i32) {
        let w = (state::CANVAS_W * self.scale).round() as i32;
        let h = ((state::CANVAS_H + state::PAD) * self.scale).round() as i32;
        (w.max(1), h.max(1))
    }

    pub fn switch_anim(&mut self, name: &str) {
        if self.cur_anim == name {
            // 同名动画：若当前 clip 尚未播完则不打断；若已播完（cur 越界）
            // 必须回卷重播，否则会永久定格在最后一帧
            let still_playing = self
                .clips
                .get(&self.cur_anim)
                .map(|c| c.cur < c.frame_count())
                .unwrap_or(false);
            if still_playing {
                return;
            }
        }
        self.cancel_move();
        self.cur_anim = name.to_string();
        if let Some(clip) = self.clips.get_mut(&self.cur_anim) {
            clip.seek(0);
        }
        self.frame_accum_ms = 0;
        self.anim_ended_fired = false;
        self.render_current();
    }

    pub fn render_current(&mut self) {
        let (dw, dh) = self.window_size();
        let pad = (state::PAD * self.scale).round() as usize;
        if let Some(clip) = self.clips.get_mut(&self.cur_anim) {
            let w = clip.webm.width as usize;
            let h = clip.webm.height as usize;
            let idx = clip.cur;
            if idx >= clip.frame_count() {
                return;
            }
            if let Some(frame) = clip.next_frame() {
                clip.cur = idx; // 不推进
                let dst_h = self.render_buf.len() / (w * 4);
                self.render_buf.fill(0);
                for y in 0..h {
                    if pad + y >= dst_h {
                        break;
                    }
                    let s = y * w * 4;
                    let d = (pad + y) * w * 4;
                    self.render_buf[d..d + w * 4].copy_from_slice(&frame[s..s + w * 4]);
                }
                self.win.resize(dw, dh);
                self.win.present(&self.render_buf, w, dst_h, self.facing_right);
            }
        }
    }

    fn on_anim_ended(&mut self) {
        let name = self.cur_anim.clone();
        let drag = self.cats.drag.clone().unwrap_or_default();
        if name == drag && self.dragging {
            if let Some(clip) = self.clips.get_mut(&self.cur_anim) {
                clip.seek(0);
            }
            self.anim_ended_fired = false;
            return;
        }
        if self.cats.turns.contains(&name) {
            self.facing_right = !self.facing_right;
        }
        if name == drag || self.cats.clicks.contains(&name) {
            // 交互动画 → 待机缓冲
            self.switch_anim(&self.cats.idle.clone().unwrap_or_default());
            return;
        }
        let can_move = self.try_plan_move();
        let next = state::pick_next(&self.cats, &name, self.no_move, can_move);
        self.switch_anim(&next);
    }

    fn try_plan_move(&mut self) -> bool {
        if self.no_move || self.move_plan.is_some() {
            return false;
        }
        let (lx, _ly, rx, _ry) = self.avail_rect();
        let (wx, _) = self.window_size();
        let (x, _, x2, _) = self.win.get_rect();
        let cx = (x + x2) as f64 / 2.0;
        let dir_sign = if self.facing_right { 1.0 } else { -1.0 };
        let distance = state::MOVE_MIN_PX + (state::MOVE_MAX_PX - state::MOVE_MIN_PX) * state::rand_f64();
        let target_cx = cx + dir_sign * distance;
        let half_w = wx as f64 / 2.0;
        if target_cx < lx as f64 + state::MOVE_MARGIN + half_w || target_cx > rx as f64 - state::MOVE_MARGIN - half_w {
            return false;
        }
        let move_name = state::pick(&self.cats.moves, None);
        let duration_ms = self.clips.get(&move_name).map(|c| c.duration_ms()).unwrap_or(2400);
        self.switch_anim(&move_name);
        self.move_plan = Some(MovePlan {
            start_x: x,
            target_x: (target_cx - half_w).round() as i32,
            y: self.win.get_rect().1,
            duration_ms,
        });
        self.move_accum_ms = 0;
        true
    }

    pub fn trigger_move(&mut self, name: &str) {
        self.cancel_move();
        if !self.try_plan_move_named(name) {
            self.switch_anim(name);
        }
    }

    fn try_plan_move_named(&mut self, name: &str) -> bool {
        let (lx, _ly, rx, _ry) = self.avail_rect();
        let (wx, _) = self.window_size();
        let (x, _, x2, _) = self.win.get_rect();
        let cx = (x + x2) as f64 / 2.0;
        let dir_sign = if self.facing_right { 1.0 } else { -1.0 };
        let distance = state::MOVE_MIN_PX + (state::MOVE_MAX_PX - state::MOVE_MIN_PX) * state::rand_f64();
        let target_cx = cx + dir_sign * distance;
        let half_w = wx as f64 / 2.0;
        if target_cx < lx as f64 + state::MOVE_MARGIN + half_w || target_cx > rx as f64 - state::MOVE_MARGIN - half_w {
            return false;
        }
        let duration_ms = self.clips.get(name).map(|c| c.duration_ms()).unwrap_or(2400);
        self.switch_anim(name);
        self.move_plan = Some(MovePlan {
            start_x: x,
            target_x: (target_cx - half_w).round() as i32,
            y: self.win.get_rect().1,
            duration_ms,
        });
        self.move_accum_ms = 0;
        true
    }

    fn cancel_move(&mut self) {
        self.move_plan = None;
        self.move_accum_ms = 0;
    }

    pub fn go_corner(&mut self) {
        let (lx, ly, rx, ry) = self.avail_rect();
        let (wx, wh) = self.window_size();
        let x = rx - wx - state::CORNER_MARGIN.round() as i32;
        let y = ry - wh;
        self.win.move_to(x, y);
        // 位置保存由 App 统一处理
    }

    pub fn restore_position(&mut self, pc: &PetConfig) {
        let (lx, ly, rx, ry) = self.avail_rect();
        let (wx, wh) = self.window_size();
        match (pc.rx, pc.ry) {
            (Some(rxv), Some(ryv)) => {
                let x = lx + (rxv * (rx - lx) as f64) as i32 - wx / 2;
                let y = ly + (ryv * (ry - ly) as f64) as i32 - wh / 2;
                let x = x.clamp(lx, rx - wx);
                let y = y.clamp(ly, ry - wh);
                self.win.move_to(x, y);
            }
            _ => self.go_corner(),
        }
    }

    pub fn save_position(&mut self, pc: &mut PetConfig) {
        let (lx, ly, rx, ry) = self.avail_rect();
        let (wx, wh) = self.window_size();
        let (x, y, x2, y2) = self.win.get_rect();
        let cx = (x + x2) as f64 / 2.0;
        let cy = (y + y2) as f64 / 2.0;
        if rx > lx && ry > ly {
            pc.rx = Some((cx - lx as f64) / (rx - lx) as f64);
            pc.ry = Some((cy - ly as f64) / (ry - ly) as f64);
        }
        pc.facing = if self.facing_right { "right" } else { "left" }.to_string();
        pc.scale = self.scale;
        pc.on_top = self.win_topmost;
        pc.no_move = self.no_move;
    }

    fn avail_rect(&self) -> (i32, i32, i32, i32) {
        crate::monitor::primary_work_area()
    }

    pub fn change_scale(&mut self, s: f64) {
        if (s - self.scale).abs() < 1e-6 {
            return;
        }
        let old_bottom = self.win.get_rect().3;
        self.scale = s;
        let (wx, wh) = self.window_size();
        self.win.resize(wx, wh);
        self.win.move_to(self.win.get_rect().0, old_bottom - wh + 1);
        self.render_current();
    }

    pub fn set_no_move(&mut self, on: bool) {
        self.no_move = on;
        if on && self.move_plan.is_some() {
            self.switch_anim(&self.cats.idle.clone().unwrap_or_default());
        }
    }

    pub fn set_topmost(&mut self, on: bool) {
        self.win_topmost = on;
        self.win.set_topmost(on);
    }

    pub fn toggle_visible(&mut self) {
        if self.visible {
            self.win.hide();
            self.visible = false;
        } else {
            self.win.show();
            self.visible = true;
        }
    }

    /// 每 tick（10ms）驱动：帧推进 + 移动插值。
    fn on_tick(&mut self) {
        let dt = self.last_tick.elapsed();
        self.last_tick = Instant::now();
        let dt_ms = dt.as_millis() as u64;
        if dt_ms == 0 {
            return;
        }

        let frame_ms = self
            .clips
            .get(&self.cur_anim)
            .map(|c| c.webm.frame_ms())
            .unwrap_or(state::FRAME_MS as u64);
        self.frame_accum_ms += dt_ms;
        if self.frame_accum_ms >= frame_ms {
            self.frame_accum_ms = 0;
            self.advance_frame();
        }

        if let Some(plan) = self.move_plan.clone() {
            self.move_accum_ms += dt_ms;
            let t = self.move_accum_ms as f64 / 1000.0;
            let dur = plan.duration_ms as f64 / 1000.0;
            let (lead, tail) = (state::MOVE_LEAD_SEC, state::MOVE_TAIL_SEC);
            let x = if t <= lead {
                plan.start_x as f64
            } else if t >= dur - tail {
                plan.target_x as f64
            } else {
                let progress = (t - lead) / (dur - lead - tail).max(0.1);
                plan.start_x as f64 + (plan.target_x - plan.start_x) as f64 * progress
            };
            self.win.move_to(x.round() as i32, plan.y);
            if t >= dur - tail {
                self.cancel_move();
            }
        }
    }

    fn advance_frame(&mut self) {
        let (dw, dh) = self.window_size();
        let pad = (state::PAD * self.scale).round() as usize;
        let anim = self.cur_anim.clone();
        let frame: Option<(Vec<u8>, usize, usize)> = match self.clips.get_mut(&anim) {
            Some(clip) => match clip.next_frame() {
                Some(f) => Some((f, clip.webm.width as usize, clip.webm.height as usize)),
                None => None,
            },
            None => None,
        };
        match frame {
            Some((frame, w, h)) => {
                let dst_h = self.render_buf.len() / (w * 4);
                self.render_buf.fill(0);
                for y in 0..h {
                    if pad + y >= dst_h {
                        break;
                    }
                    let s = y * w * 4;
                    let d = (pad + y) * w * 4;
                    self.render_buf[d..d + w * 4].copy_from_slice(&frame[s..s + w * 4]);
                }
                self.win.resize(dw, dh);
                self.win.present(&self.render_buf, w, dst_h, self.facing_right);
            }
            None => {
                if !self.anim_ended_fired {
                    self.anim_ended_fired = true;
                    self.on_anim_ended();
                }
            }
        }
    }

    // ---------------- 交互（含 SetCapture 拖拽修复） ----------------
    fn on_lbutton_down(&mut self, lparam: LPARAM) {
        // 捕获鼠标：快速拖拽时鼠标移出窗口仍能收到事件（原版 Qt 自动捕获）
        unsafe { SetCapture(self.win.hwnd) };
        let (cx, cy) = client_pos(lparam);
        let (wx, wy, _, _) = self.win.get_rect();
        let (sx, sy) = (wx + cx, wy + cy);
        self.press_global = Some((sx, sy));
        self.grab_offset = Some((sx - wx, sy - wy));
        self.dragging = false;
        self.cancel_move();
    }

    fn on_mouse_move(&mut self, lparam: LPARAM) {
        let _ = lparam;
        if self.press_global.is_none() {
            return;
        }
        // 用全局光标位置：鼠标可能已移出窗口边界（捕获后），lparam 的 client 坐标不可靠
        let (sx, sy) = cursor_pos();
        if sx == i32::MIN {
            return;
        }
        let (px, py) = self.press_global.unwrap();
        let dx = sx - px;
        let dy = sy - py;
        let dist = ((dx * dx + dy * dy) as f64).sqrt();
        if !self.dragging {
            if dist < state::DRAG_THRESHOLD * self.scale {
                return;
            }
            self.dragging = true;
            self.switch_anim(&self.cats.drag.clone().unwrap_or_default());
        }
        if let Some(off) = self.grab_offset {
            self.win.move_to(sx - off.0, sy - off.1);
        }
    }

    fn on_lbutton_up(&mut self) {
        let was_dragging = self.dragging;
        // 释放鼠标捕获
        unsafe { ReleaseCapture() };
        let (sx, sy) = cursor_pos();
        if was_dragging {
            self.just_dragged = true;
            if let Some(off) = self.grab_offset {
                self.win.move_to(sx - off.0, sy - off.1);
            }
            // 位置保存由 App 在拖拽结束后统一处理
            self.switch_anim(&self.cats.idle.clone().unwrap_or_default());
        } else {
            self.on_click();
        }
        self.dragging = false;
        self.press_global = None;
        self.grab_offset = None;
    }

    fn on_click(&mut self) {
        if self.just_dragged {
            self.just_dragged = false;
            return;
        }
        if self.cats.idle.as_deref() != Some(self.cur_anim.as_str()) {
            return;
        }
        self.cancel_move();
        let c = self.cats.clicks.clone();
        let pick = state::pick(&c, None);
        self.switch_anim(&pick);
    }

    /// 窗口消息处理（App 按 hwnd 分发到这里）。
    pub fn on_wnd_message(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_NCHITTEST => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
                let (wx, wy, _, _) = self.win.get_rect();
                let hit = self.win.hit_test_alpha(x - wx, y - wy);
                if hit {
                    Some(HTCLIENT as LRESULT)
                } else {
                    Some(HTTRANSPARENT as LRESULT)
                }
            }
            WM_TIMER if (wparam as usize) == FRAME_TIMER => {
                self.on_tick();
                Some(0)
            }
            WM_LBUTTONDOWN => {
                self.on_lbutton_down(lparam);
                Some(0)
            }
            WM_MOUSEMOVE => {
                self.on_mouse_move(lparam);
                Some(0)
            }
            WM_LBUTTONUP => {
                self.on_lbutton_up();
                Some(0)
            }
            _ => None,
        }
    }

    /// 右键菜单。返回选中的菜单命令（0 = 无）。由 App 统一执行。
    pub fn on_context_menu(&mut self, lparam: LPARAM) -> usize {
        let (sx, sy) = if lparam == 0xFFFF_FFFF {
            cursor_pos()
        } else {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let (wx, wy, _, _) = self.win.get_rect();
            (wx + x, wy + y)
        };

        let h = unsafe { CreatePopupMenu() };

        // 动画·待机（多视频二级菜单）
        if !self.cats.idles.is_empty() {
            let sub_idle = unsafe { CreatePopupMenu() };
            for (i, name) in self.cats.idles.iter().enumerate() {
                unsafe { AppendMenuW(sub_idle, MF_STRING, MID_IDLE_BASE + i, wide(name)) };
            }
            unsafe { AppendMenuW(h, MF_STRING | MF_POPUP, sub_idle as usize, wide("动画 · 待机")) };
        }
        // 动画·转向（多视频二级菜单）
        if !self.cats.turns.is_empty() {
            let sub_turn = unsafe { CreatePopupMenu() };
            for (i, name) in self.cats.turns.iter().enumerate() {
                unsafe { AppendMenuW(sub_turn, MF_STRING, MID_TURN_BASE + i, wide(name)) };
            }
            unsafe { AppendMenuW(h, MF_STRING | MF_POPUP, sub_turn as usize, wide("动画 · 转向")) };
        }
        // 动画·移动
        if !self.cats.moves.is_empty() {
            let sub_move = unsafe { CreatePopupMenu() };
            for (i, name) in self.cats.moves.iter().enumerate() {
                unsafe { AppendMenuW(sub_move, MF_STRING, MID_MOVE_BASE + i, wide(name)) };
            }
            unsafe { AppendMenuW(h, MF_STRING | MF_POPUP, sub_move as usize, wide("动画 · 移动")) };
        }
        // 动画·点击回应
        if !self.cats.clicks.is_empty() {
            let sub_click = unsafe { CreatePopupMenu() };
            for (i, name) in self.cats.clicks.iter().enumerate() {
                unsafe { AppendMenuW(sub_click, MF_STRING, MID_CLICK_BASE + i, wide(name)) };
            }
            unsafe { AppendMenuW(h, MF_STRING | MF_POPUP, sub_click as usize, wide("动画 · 点击回应")) };
        }
        // 动画·随机动作
        if !self.cats.acts.is_empty() {
            let sub_acts = unsafe { CreatePopupMenu() };
            for (i, name) in self.cats.acts.iter().enumerate() {
                unsafe { AppendMenuW(sub_acts, MF_STRING, MID_ACT_BASE + i, wide(name)) };
            }
            unsafe { AppendMenuW(h, MF_STRING | MF_POPUP, sub_acts as usize, wide("动画 · 随机动作")) };
        }

        // 切换角色（多角色时显示）
        let chars = crate::role::list_characters();
        if chars.len() > 1 {
            let sub_role = unsafe { CreatePopupMenu() };
            for (i, cid) in chars.iter().enumerate() {
                let checked = self.role_id == *cid;
                unsafe {
                    AppendMenuW(
                        sub_role,
                        if checked { MF_STRING | MF_CHECKED } else { MF_STRING },
                        MID_ROLE_BASE + i,
                        wide(cid),
                    )
                };
            }
            unsafe { AppendMenuW(h, MF_STRING | MF_POPUP, sub_role as usize, wide("切换角色")) };
        }

        unsafe { AppendMenuW(h, MF_SEPARATOR, 0, std::ptr::null()) };
        unsafe { AppendMenuW(h, MF_STRING, MID_CORNER, wide("回到右下角")) };
        unsafe {
            AppendMenuW(h, if self.win_topmost { MF_STRING | MF_CHECKED } else { MF_STRING }, MID_ONTOP, wide("窗口置顶"))
        };
        unsafe {
            AppendMenuW(h, if self.no_move { MF_STRING | MF_CHECKED } else { MF_STRING }, MID_NOMOVE, wide("不移动"))
        };
        let auto = crate::autostart::is_enabled();
        unsafe {
            AppendMenuW(h, if auto { MF_STRING | MF_CHECKED } else { MF_STRING }, MID_AUTOSTART, wide("开机自启"))
        };
        let sub_scale = unsafe { CreatePopupMenu() };
        for (i, s) in state::SCALE_STEPS.iter().enumerate() {
            let px = (state::CANVAS_W * s).round() as i32;
            let checked = (self.scale - s).abs() < 0.02;
            unsafe {
                AppendMenuW(sub_scale, if checked { MF_STRING | MF_CHECKED } else { MF_STRING }, MID_SCALE_BASE + i, wide(&format!("{}px", px)))
            };
        }
        unsafe { AppendMenuW(h, MF_STRING | MF_POPUP, sub_scale as usize, wide("大小")) };
        unsafe { AppendMenuW(h, MF_SEPARATOR, 0, std::ptr::null()) };
        unsafe { AppendMenuW(h, MF_STRING, MID_SPAWN, wide("生成新桌宠")) };
        unsafe { AppendMenuW(h, MF_STRING, MID_QUIT_PET, wide("删除此桌宠")) };

        let cmd = unsafe {
            TrackPopupMenu(h, TPM_RETURNCMD | TPM_RIGHTBUTTON, sx, sy, 0, self.win.hwnd, std::ptr::null())
        };
        unsafe { DestroyMenu(h) };

        cmd as usize
    }

    /// 执行宠物自身的命令（动画/位置/大小/置顶/不移动）。全局命令（角色切换/生成/删除）由 App 处理。
    pub fn apply_command(&mut self, id: usize) {
        match id {
            MID_CORNER => self.go_corner(),
            MID_ONTOP => {
                let on = !self.win_topmost;
                self.set_topmost(on);
            }
            MID_NOMOVE => {
                let on = !self.no_move;
                self.set_no_move(on);
            }
            _ => {
                if id >= MID_IDLE_BASE && id < MID_IDLE_BASE + 50 {
                    let i = id - MID_IDLE_BASE;
                    if i < self.cats.idles.len() {
                        self.switch_anim(&self.cats.idles[i].clone());
                    }
                } else if id >= MID_TURN_BASE && id < MID_TURN_BASE + 50 {
                    let i = id - MID_TURN_BASE;
                    if i < self.cats.turns.len() {
                        self.switch_anim(&self.cats.turns[i].clone());
                    }
                } else if id >= MID_ACT_BASE && id < MID_ACT_BASE + 50 {
                    let i = id - MID_ACT_BASE;
                    if i < self.cats.acts.len() {
                        self.switch_anim(&self.cats.acts[i].clone());
                    }
                } else if id >= MID_MOVE_BASE && id < MID_MOVE_BASE + 20 {
                    let i = id - MID_MOVE_BASE;
                    if i < self.cats.moves.len() {
                        self.trigger_move(&self.cats.moves[i].clone());
                    }
                } else if id >= MID_CLICK_BASE && id < MID_CLICK_BASE + 20 {
                    let i = id - MID_CLICK_BASE;
                    if i < self.cats.clicks.len() {
                        self.switch_anim(&self.cats.clicks[i].clone());
                    }
                } else if id >= MID_SCALE_BASE && id < MID_SCALE_BASE + 4 {
                    let i = id - MID_SCALE_BASE;
                    if i < state::SCALE_STEPS.len() {
                        self.change_scale(state::SCALE_STEPS[i]);
                    }
                }
            }
        }
    }
}

fn client_pos(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam & 0xFFFF) as i16 as i32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

fn cursor_pos() -> (i32, i32) {
    let mut p: POINT = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetCursorPos(&mut p) };
    if ok != 0 {
        (p.x, p.y)
    } else {
        (i32::MIN, i32::MIN)
    }
}

fn wide(s: &str) -> *const u16 {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    let ptr = v.as_ptr();
    std::mem::forget(v);
    ptr
}
