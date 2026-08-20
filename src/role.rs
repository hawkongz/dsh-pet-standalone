//! 多角色管理：内置 shenshen + 外部扩展角色目录，素材加载与分类。
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::state;
use crate::webm::WebM;

/// 一个角色的动画素材集。
pub struct RoleAssets {
    pub id: String,
    /// 动画名 → 解析后的 WebM（Rc 共享）。
    pub videos: HashMap<String, Rc<WebM>>,
    /// videos/ 子目录 → 动画名列表（用于动态分类）。
    pub folder_files: HashMap<String, Vec<String>>,
    /// manifest.json 内容（可选）。
    pub manifest: Option<serde_json::Value>,
    /// 动画名列表（videos 的键）。
    pub names: Vec<String>,
}

/// 外部可扩展角色根目录（exe 同目录 + 用户数据目录）。
fn external_character_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    // exe 同目录 / 当前工作目录
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("characters"));
        }
    }
    dirs.push(std::path::PathBuf::from("characters"));
    // 用户数据目录
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("dsh-pet-standalone").join("characters"));
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        dirs.push(PathBuf::from(home).join("dsh-pet-standalone").join("characters"));
    }
    dirs
}

/// 可用的角色列表：内置 shenshen + 外部目录中检测到的角色。
pub fn list_characters() -> Vec<String> {
    let mut ids: Vec<String> = vec![state::DEFAULT_CHARACTER.to_string()];
    let mut seen: std::collections::HashSet<String> = ids.iter().cloned().collect();
    for root in external_character_dirs() {
        if !root.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(&root) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let mut children: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        children.sort();
        for child in children {
            if !child.is_dir() {
                continue;
            }
            let video_dir = child.join("videos");
            if !video_dir.is_dir() {
                continue;
            }
            let mut found = Vec::new();
            collect_webm(&video_dir, &mut found);
            if found.is_empty() {
                continue;
            }
            if let Some(name) = child.file_name().and_then(|n| n.to_str()) {
                if seen.insert(name.to_string()) {
                    ids.push(name.to_string());
                }
            }
        }
    }
    ids
}

/// 解析一个角色的素材目录（外部优先，回退内置）。
fn resolve_video_dir(id: &str) -> Option<PathBuf> {
    for root in external_character_dirs() {
        let candidate = root.join(id).join("videos");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

/// 扫描磁盘角色目录，加载全部 webm + 子目录分类 + manifest。
fn load_role_from_disk(id: &str, video_dir: &Path) -> Option<RoleAssets> {
    let mut videos: HashMap<String, Rc<WebM>> = HashMap::new();
    let mut folder_files: HashMap<String, Vec<String>> = HashMap::new();

    // 递归收集 webm
    let mut webm_paths: Vec<PathBuf> = Vec::new();
    collect_webm(video_dir, &mut webm_paths);
    webm_paths.sort();
    if webm_paths.is_empty() {
        return None;
    }

    for path in &webm_paths {
        let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        if name.is_empty() {
            continue;
        }
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let wm = match WebM::parse(&data) {
            Some(w) => w,
            None => continue,
        };
        videos.insert(name.clone(), Rc::new(wm));
        // 相对 video_dir 的子目录分类
        if let Ok(rel) = path.strip_prefix(video_dir) {
            let folder = rel.parent().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !folder.is_empty() {
                folder_files.entry(folder).or_default().push(name);
            }
        }
    }

    // manifest.json（videos/ 内或角色目录根）
    let manifest = load_manifest(video_dir);

    let names: Vec<String> = videos.keys().cloned().collect();
    Some(RoleAssets {
        id: id.to_string(),
        videos,
        folder_files,
        manifest,
        names,
    })
}

fn collect_webm(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_webm(&p, out);
            } else if p.extension().map_or(false, |x| x == "webm") {
                out.push(p);
            }
        }
    }
}

/// 读取 manifest.json（videos/ 内或父目录）。
fn load_manifest(video_dir: &Path) -> Option<serde_json::Value> {
    let candidates = [
        video_dir.join(state::MANIFEST_FILENAME),
        video_dir.parent().unwrap_or(video_dir).join(state::MANIFEST_FILENAME),
    ];
    for p in candidates {
        if p.is_file() {
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v.is_object() {
                        return Some(v);
                    }
                }
            }
            return None;
        }
    }
    None
}

/// 加载角色素材。内置 shenshen 从内嵌素材，其他/外部覆盖从磁盘。
pub fn load_role(id: &str) -> Option<RoleAssets> {
    // 外部目录优先
    if let Some(dir) = resolve_video_dir(id) {
        if let Some(assets) = load_role_from_disk(id, &dir) {
            return Some(assets);
        }
    }
    // 内置 shenshen
    if id == state::DEFAULT_CHARACTER {
        return load_builtin_shenshen();
    }
    None
}

/// 内置 shenshen：从内嵌 ASSET_PAK 加载 51 个 webm（flat 结构，无子目录）。
fn load_builtin_shenshen() -> Option<RoleAssets> {
    let mut videos: HashMap<String, Rc<WebM>> = HashMap::new();
    for (name, start, len) in crate::assets::ANIMS {
        let data = &crate::assets::ASSET_PAK[*start..*start + *len];
        if let Some(wm) = WebM::parse(data) {
            videos.insert(name.to_string(), Rc::new(wm));
        }
    }
    if videos.is_empty() {
        return None;
    }
    let names: Vec<String> = videos.keys().cloned().collect();
    Some(RoleAssets {
        id: state::DEFAULT_CHARACTER.to_string(),
        videos,
        folder_files: HashMap::new(),
        manifest: None,
        names,
    })
}
