//! 桌宠应用：管理多只桌宠（窗口/状态机/交互）+ 角色切换 + 托盘 + 菜单分发 + 开机自启。
#![allow(non_snake_case, dead_code)]

use std::rc::Rc;

use windows_sys::Win32::{
    Foundation::{HWND, LRESULT, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, PostQuitMessage, WM_CLOSE, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP},
};

use crate::config::{Config, PetConfig};
use crate::pet::{self, Pet};
use crate::role::{self, RoleAssets};
use crate::state;
use crate::tray::{self, WM_TRAY};
use crate::win32::{PetWindow, WindowCallback};

pub struct App {
    pub pets: Vec<Pet>,
    /// 当前角色素材集（所有宠物共享解码器源）。
    pub role: Rc<RoleAssets>,
    pub current_character: String,
    pub cfg: Config,
    pub quitting: bool,
    next_pet_id: usize,
    pub instance: windows_sys::Win32::Foundation::HINSTANCE,
}

impl App {
    pub fn new(instance: windows_sys::Win32::Foundation::HINSTANCE) -> App {
        let cfg = Config::load();
        // 加载配置中的角色（失败回退默认）
        let character = cfg.character.clone();
        let role = match role::load_role(&character) {
            Some(r) => Rc::new(r),
            None => match role::load_role(state::DEFAULT_CHARACTER) {
                Some(r) => Rc::new(r),
                None => {
                    // 极端情况：无任何素材，返回空（主流程会因 pets 为空退出）
                    return App {
                        pets: Vec::new(),
                        role: Rc::new(RoleAssets {
                            id: state::DEFAULT_CHARACTER.to_string(),
                            videos: std::collections::HashMap::new(),
                            folder_files: std::collections::HashMap::new(),
                            manifest: None,
                            names: Vec::new(),
                        }),
                        current_character: state::DEFAULT_CHARACTER.to_string(),
                        cfg,
                        quitting: false,
                        next_pet_id: 1,
                        instance,
                    };
                }
            },
        };
        let mut app = App {
            pets: Vec::new(),
            role,
            current_character: character,
            cfg,
            quitting: false,
            next_pet_id: 1,
            instance,
        };
        // 创建所有已保存的宠物（至少一只）
        let count = app.cfg.pets.len().max(1);
        for i in 0..count {
            let pc = app.cfg.pets.get(i).cloned().unwrap_or_default();
            app.spawn_pet_from_cfg(i + 1, &pc);
        }
        app
    }

    /// 用已有配置创建宠物（按 id 追加）。
    fn spawn_pet_from_cfg(&mut self, id: usize, pc: &PetConfig) {
        if let Some(win) = PetWindow::create(self.instance, pc.on_top) {
            let mut pet = Pet::new(id, win, &self.role, pc);
            pet.restore_position(pc);
            self.pets.push(pet);
            if self.next_pet_id <= id {
                self.next_pet_id = id + 1;
            }
        }
    }

    /// 生成一只新桌宠。
    pub fn spawn_pet(&mut self) {
        let id = self.next_pet_id;
        self.next_pet_id += 1;
        let mut pc = PetConfig::default();
        if let Some(first) = self.pets.first() {
            if let Some(p0) = self.cfg.pets.first() {
                pc.rx = p0.rx.map(|v| v + 0.02);
                pc.ry = p0.ry.map(|v| v + 0.02);
            }
            let _ = first;
        }
        self.spawn_pet_from_cfg(id, &pc);
        self.cfg.pets = self.pets.iter().map(|_| PetConfig::default()).collect();
        self.save_all_positions();
    }

    /// 切换角色：加载新角色素材，重建所有宠物。
    pub fn switch_character(&mut self, id: &str) {
        if id == self.current_character {
            return;
        }
        let role = match role::load_role(id) {
            Some(r) => Rc::new(r),
            None => return,
        };
        self.current_character = id.to_string();
        self.cfg.character = id.to_string();
        self.cfg.save();
        self.role = role;
        for pet in &mut self.pets {
            pet.apply_role(&self.role);
        }
    }

    /// 退出当前宠物（最后一只则退出整个应用）。
    pub fn quit_pet(&mut self, hwnd: HWND) {
        if let Some(idx) = self.pets.iter().position(|p| p.win.hwnd == hwnd) {
            // 先异步发送 WM_CLOSE 销毁窗口（不能同步 DestroyWindow：
            // 当前正处在窗口过程调用栈中，同步销毁会重入 wnd_proc 导致双重 &mut self）
            unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            self.pets.remove(idx);
            if idx < self.cfg.pets.len() {
                self.cfg.pets.remove(idx);
            }
            self.cfg.save();
        }
        if self.pets.is_empty() {
            self.quit_all();
        }
    }

    /// 保存某只宠物的位置。
    pub fn save_pet_position(&mut self, hwnd: HWND) {
        if let Some(idx) = self.pets.iter().position(|p| p.win.hwnd == hwnd) {
            while self.cfg.pets.len() <= idx {
                self.cfg.pets.push(PetConfig::default());
            }
            let pc = self.cfg.pets.get_mut(idx).unwrap();
            self.pets[idx].save_position(pc);
            self.cfg.save();
        }
    }

    /// 保存所有宠物位置。
    pub fn save_all_positions(&mut self) {
        while self.cfg.pets.len() < self.pets.len() {
            self.cfg.pets.push(PetConfig::default());
        }
        for i in 0..self.pets.len() {
            self.pets[i].save_position(&mut self.cfg.pets[i]);
        }
        self.cfg.save();
    }

    pub fn quit_all(&mut self) {
        self.quitting = true;
        self.save_all_positions();
        unsafe { PostQuitMessage(0) };
    }

    pub fn primary_hwnd(&self) -> HWND {
        self.pets.first().map(|p| p.win.hwnd).unwrap_or(std::ptr::null_mut())
    }

    pub fn toggle_all_visible(&mut self) {
        for pet in &mut self.pets {
            pet.toggle_visible();
        }
    }

    /// 处理宠物菜单命令（全局命令 + 宠物命令）。
    fn handle_pet_command(&mut self, hwnd: HWND, cmd: usize) {
        match cmd {
            pet::MID_SPAWN => {
                self.spawn_pet();
            }
            pet::MID_QUIT_PET => {
                self.quit_pet(hwnd);
            }
            pet::MID_AUTOSTART => {
                let on = !crate::autostart::is_enabled();
                crate::autostart::set_enabled(on);
            }
            _ => {
                if cmd >= pet::MID_ROLE_BASE && cmd < pet::MID_ROLE_BASE + 100 {
                    // 切换角色
                    let chars = role::list_characters();
                    let i = cmd - pet::MID_ROLE_BASE;
                    if let Some(cid) = chars.get(i) {
                        self.switch_character(cid);
                    }
                } else if let Some(pet) = self.pets.iter_mut().find(|p| p.win.hwnd == hwnd) {
                    pet.apply_command(cmd);
                }
                self.save_all_positions();
            }
        }
    }
}

impl WindowCallback for App {
    fn on_wnd_message(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_RBUTTONUP => {
                if let Some(pet) = self.pets.iter_mut().find(|p| p.win.hwnd == hwnd) {
                    let cmd = pet.on_context_menu(lparam);
                    if cmd != 0 {
                        self.handle_pet_command(hwnd, cmd);
                    }
                }
                Some(0)
            }
            WM_LBUTTONUP => {
                let r = self
                    .pets
                    .iter_mut()
                    .find(|p| p.win.hwnd == hwnd)
                    .and_then(|p| p.on_wnd_message(msg, wparam, lparam));
                // 拖拽/点击后保存位置
                self.save_pet_position(hwnd);
                r
            }
            WM_TRAY => {
                let msg2 = (lparam & 0xFFFF) as u32;
                if msg2 == WM_LBUTTONDBLCLK {
                    self.toggle_all_visible();
                } else if msg2 == WM_RBUTTONUP {
                    tray::show_menu(self);
                }
                Some(0)
            }
            _ => {
                self.pets
                    .iter_mut()
                    .find(|p| p.win.hwnd == hwnd)
                    .and_then(|p| p.on_wnd_message(msg, wparam, lparam))
            }
        }
    }
}
