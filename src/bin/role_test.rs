//! 多角色验证：list_characters / load_role / build_categories。
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

fn main() {
    // 1. 可用角色
    let chars = role::list_characters();
    println!("可用角色: {:?}", chars);

    // 2. 内置 shenshen
    if let Some(r) = role::load_role("shenshen") {
        let ff = if r.folder_files.is_empty() { None } else { Some(&r.folder_files) };
        let cats = state::build_categories(&r.names, r.manifest.as_ref(), ff);
        println!("\nshenshen: {} 个动画", r.names.len());
        println!("  idle={:?} idles={:?}", cats.idle, cats.idles);
        println!("  turn={:?} turns={:?}", cats.turn, cats.turns);
        println!("  moves={:?}", cats.moves);
        println!("  clicks={:?}", cats.clicks);
        println!("  drag={:?}", cats.drag);
        println!("  acts={} 个: {:?}", cats.acts.len(), &cats.acts[..cats.acts.len().min(5)]);
        assert!(!cats.idles.is_empty(), "shenshen 应有待机");
        assert!(cats.moves.len() == 3, "shenshen 应有 3 个移动");
        assert!(cats.acts.len() >= 40, "shenshen 应有 40+ 随机动作");
    } else {
        println!("shenshen 加载失败!");
    }

    // 3. 外部 test 角色（子目录分类）
    if let Some(r) = role::load_role("test") {
        let ff = if r.folder_files.is_empty() { None } else { Some(&r.folder_files) };
        let cats = state::build_categories(&r.names, r.manifest.as_ref(), ff);
        println!("\ntest: {} 个动画", r.names.len());
        println!("  folder_files={:?}", r.folder_files);
        println!("  idle={:?}", cats.idle);
        println!("  turn={:?}", cats.turn);
        println!("  acts={:?}", cats.acts);
        assert_eq!(cats.idle.as_deref(), Some("待机呼吸休闲"), "test 待机来自 idle 子目录");
        assert_eq!(cats.turn.as_deref(), Some("东张西望"), "test 转向来自 turn 子目录");
        assert!(cats.acts.contains(&"写代码".to_string()), "test 动作来自 random 子目录");
        println!("\n=== 角色测试通过 ===");
    } else {
        println!("test 角色加载失败!");
    }
}
