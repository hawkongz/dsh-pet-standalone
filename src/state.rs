//! 动画目录 + 动态分类 + 动画链状态机（1:1 移植上游 catalog.py / window.py）。
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

// 几何常量
pub const CANVAS_W: f64 = 640.0;
pub const CANVAS_H: f64 = 360.0;
pub const PAD: f64 = 30.0; // 落地偏移：帧下移让脚踩窗口底线
pub const FRAME_MS: u32 = 40;

// 动画链概率
pub const P_IDLE: f64 = 0.30;
pub const P_TURN: f64 = 0.40;
pub const P_ACTS: f64 = 0.80;

// 移动参数
pub const MOVE_MIN_PX: f64 = 60.0;
pub const MOVE_MAX_PX: f64 = 240.0;
pub const MOVE_MARGIN: f64 = 20.0;
pub const MOVE_LEAD_SEC: f64 = 2.0;
pub const MOVE_TAIL_SEC: f64 = 2.0;

pub const DRAG_THRESHOLD: f64 = 5.0;
pub const DEFAULT_SCALE: f64 = 0.72;
pub const CORNER_MARGIN: f64 = 24.0;
pub const SCALE_STEPS: [f64; 4] = [0.5, 0.72, 0.85, 1.0];

// 多形象
pub const DEFAULT_CHARACTER: &str = "shenshen";
pub const MANIFEST_FILENAME: &str = "manifest.json";
// videos 下的分类子目录
pub const DIR_IDLE: &str = "idle";
pub const DIR_TURN: &str = "turn";
pub const DIR_IDLE_TURN: &str = "idle_turn";
pub const DIR_MOVE: &str = "move";
pub const DIR_CLICK: &str = "click";
pub const DIR_DRAG: &str = "drag";
pub const DIR_RANDOM: &str = "random";

// 内置已知动画名（兜底分类）
pub const IDLE: &str = "待机呼吸休闲";
pub const TURN: &str = "东张西望";
pub const MOVES: [&str; 3] = ["螃蟹走路", "原地漂浮踏步", "原地左转奔跑"];
pub const CLICKS: [&str; 3] = ["点击回应 - 开心跃动", "点击回应 - 害羞惊讶", "点击回应 - 傲娇生气（侧身展示）"];
pub const DRAG: &str = "被鼠标拖拽悬空反馈";

/// 动态分类结果（对应上游 build_categories 返回）。
pub struct Category {
    pub idle: Option<String>,
    pub turn: Option<String>,
    pub idles: Vec<String>,
    pub turns: Vec<String>,
    pub moves: Vec<String>,
    pub clicks: Vec<String>,
    pub drag: Option<String>,
    pub acts: Vec<String>,
}

impl Category {
    pub fn empty() -> Category {
        Category {
            idle: None,
            turn: None,
            idles: Vec::new(),
            turns: Vec::new(),
            moves: Vec::new(),
            clicks: Vec::new(),
            drag: None,
            acts: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.idles.is_empty() && self.turns.is_empty()
            && self.moves.is_empty() && self.clicks.is_empty()
            && self.drag.is_none() && self.acts.is_empty()
    }
}

fn keyword_match(name: &str, keywords: &[&str]) -> bool {
    let low = name.to_lowercase();
    keywords.iter().any(|k| low.contains(&k.to_lowercase()))
}

/// manifest 中的文件名/动画名 → 实际存在的动画名。
fn manifest_name(value: &serde_json::Value, names: &HashSet<String>) -> Option<String> {
    let s = value.as_str()?;
    let stem = s.trim_end_matches(".webm");
    if names.contains(stem) {
        return Some(stem.to_string());
    }
    if names.contains(s) {
        return Some(s.to_string());
    }
    None
}

/// manifest 中的列表字段 → 存在的动画名列表。
fn manifest_names(value: &serde_json::Value, names: &HashSet<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    match value {
        serde_json::Value::Array(arr) => {
            for v in arr {
                if let Some(n) = manifest_name(v, names) {
                    if !out.contains(&n) {
                        out.push(n);
                    }
                }
            }
        }
        other => {
            if let Some(n) = manifest_name(other, names) {
                out.push(n);
            }
        }
    }
    out
}

/// 动态计算动画分类（移植上游 catalog.build_categories）。
/// - folder_files: videos/ 子目录 → 动画名列表（优先）
/// - manifest: manifest.json 内容（补充/覆盖）
/// - 否则按内置已知名 + 文件名关键词自动识别
pub fn build_categories(
    names: &[String],
    manifest: Option<&serde_json::Value>,
    folder_files: Option<&HashMap<String, Vec<String>>>,
) -> Category {
    let name_set: HashSet<String> = names.iter().cloned().collect();
    let mut cat = Category::empty();
    if name_set.is_empty() {
        return cat;
    }

    let mut idles: Vec<String> = Vec::new();
    let mut turns: Vec<String> = Vec::new();
    let mut moves: Vec<String> = Vec::new();
    let mut clicks: Vec<String> = Vec::new();
    let mut drag: Option<String> = None;

    // 1. 子目录分类（folder_files 直接给出 folder -> [names]）
    if let Some(ff) = folder_files {
        if let Some(v) = ff.get(DIR_IDLE) {
            idles = v.clone();
        }
        if let Some(v) = ff.get(DIR_TURN) {
            turns = v.clone();
        }
        let legacy = ff.get(DIR_IDLE_TURN).cloned().unwrap_or_default();
        if idles.is_empty() && !legacy.is_empty() {
            let cands: Vec<String> = legacy
                .iter()
                .filter(|n| *n == IDLE || keyword_match(n, &["待机", "idle", "呼吸"]))
                .cloned()
                .collect();
            idles = if cands.is_empty() {
                legacy[..1.min(legacy.len())].to_vec()
            } else {
                cands
            };
        }
        if turns.is_empty() && !legacy.is_empty() {
            let cands: Vec<String> = legacy
                .iter()
                .filter(|n| *n == TURN || keyword_match(n, &["转向", "转身", "东张西望", "turn", "回头", "转"]))
                .cloned()
                .collect();
            turns = if cands.is_empty() && idles.len() > 1 {
                legacy.iter().filter(|n| n.as_str() != idles[0]).take(1).cloned().collect()
            } else {
                cands
            };
        }
        if let Some(v) = ff.get(DIR_MOVE) {
            moves = v.clone();
        }
        if let Some(v) = ff.get(DIR_CLICK) {
            clicks = v.clone();
        }
        if let Some(v) = ff.get(DIR_DRAG) {
            if let Some(first) = v.first() {
                drag = Some(first.clone());
            }
        }
    }

    // 2. manifest 补充/覆盖
    if let Some(m) = manifest {
        if idles.is_empty() {
            if let Some(n) = m.get("idle").and_then(|v| manifest_name(v, &name_set)) {
                idles = vec![n];
            }
        }
        if turns.is_empty() {
            if let Some(n) = m.get("turn").and_then(|v| manifest_name(v, &name_set)) {
                turns = vec![n];
            }
        }
        if moves.is_empty() {
            if let Some(v) = m.get("moves") {
                moves = manifest_names(v, &name_set);
            }
        }
        if clicks.is_empty() {
            if let Some(v) = m.get("clicks") {
                clicks = manifest_names(v, &name_set);
            }
        }
        if drag.is_none() {
            if let Some(n) = m.get("drag").and_then(|v| manifest_name(v, &name_set)) {
                drag = Some(n);
            }
        }
    }

    // 3. 关键词/内置名兜底
    if idles.is_empty() {
        let m = if name_set.contains(IDLE) {
            Some(IDLE.to_string())
        } else {
            names.iter().find(|n| keyword_match(n, &["待机", "idle", "呼吸"])).cloned()
        };
        idles = m.into_iter().collect();
    }
    if turns.is_empty() {
        let m = if name_set.contains(TURN) {
            Some(TURN.to_string())
        } else {
            names.iter().find(|n| keyword_match(n, &["转向", "转身", "东张西望", "turn", "回头", "转"])).cloned()
        };
        turns = m.into_iter().collect();
    }
    if drag.is_none() {
        let m = if name_set.contains(DRAG) {
            Some(DRAG.to_string())
        } else {
            names.iter().find(|n| keyword_match(n, &["拖拽", "拖", "悬空", "drag", "抓"])).cloned()
        };
        drag = m;
    }
    if moves.is_empty() {
        moves = MOVES.iter().filter(|n| name_set.contains(**n)).map(|s| s.to_string()).collect();
        if moves.is_empty() {
            moves = names.iter().filter(|n| keyword_match(n, &["走", "跑", "移动", "move", "walk", "run", "踏步", "奔跑"])).cloned().collect();
        }
    }
    if clicks.is_empty() {
        clicks = CLICKS.iter().filter(|n| name_set.contains(**n)).map(|s| s.to_string()).collect();
        if clicks.is_empty() {
            clicks = names.iter().filter(|n| keyword_match(n, &["点击", "回应", "click", "response"])).cloned().collect();
        }
    }

    // 无 idle 时安全回退到第一个动画
    if idles.is_empty() {
        if let Some(first) = names.first() {
            idles = vec![first.clone()];
        }
    }

    // 4. 随机动作池
    let mut core: HashSet<&str> = HashSet::new();
    for n in idles.iter().chain(turns.iter()).chain(moves.iter()).chain(clicks.iter()) {
        core.insert(n.as_str());
    }
    if let Some(d) = &drag {
        core.insert(d.as_str());
    }
    let mut acts: Vec<String>;
    if let Some(ff) = folder_files {
        // 子目录模式：random/ 和未知目录 → 随机动作池（允许重复分类）
        let mut seen: HashSet<String> = HashSet::new();
        acts = Vec::new();
        for (folder, ns) in ff {
            if folder != DIR_RANDOM && is_known_folder(folder) {
                continue;
            }
            for n in ns {
                if seen.insert(n.clone()) {
                    acts.push(n.clone());
                }
            }
        }
    } else {
        acts = names.iter().filter(|n| !core.contains(n.as_str())).cloned().collect();
    }
    acts.dedup();

    cat.idle = idles.first().cloned();
    cat.turn = turns.first().cloned();
    cat.idles = idles;
    cat.turns = turns;
    cat.moves = moves;
    cat.clicks = clicks;
    cat.drag = drag;
    cat.acts = acts;
    cat
}

fn is_known_folder(folder: &str) -> bool {
    matches!(folder, DIR_IDLE | DIR_TURN | DIR_MOVE | DIR_CLICK | DIR_DRAG)
}

/// 从名称池随机选（排除 exclude）。
pub fn pick(pool: &[String], exclude: Option<&str>) -> String {
    if pool.is_empty() {
        return String::new();
    }
    let mut idx = (rand_u32() as usize) % pool.len();
    if let Some(e) = exclude {
        if pool.len() > 1 && pool[idx] == e {
            idx = (idx + 1) % pool.len();
        }
    }
    pool[idx].clone()
}

/// 动画链：30% 待机 / 10% 转向 / 40% 动作 / 20% 移动。
/// no_move 时移动概率并入动作；待机/转向支持多视频。
pub fn pick_next(cat: &Category, current: &str, no_move: bool, can_move: bool) -> String {
    let roll = rand_f64();
    if roll < P_IDLE {
        if !cat.idles.is_empty() {
            return pick(&cat.idles, Some(current));
        }
        return pick(&cat.acts, Some(current));
    }
    if roll < P_TURN {
        if !cat.turns.is_empty() {
            return pick(&cat.turns, Some(current));
        }
        return pick(&cat.acts, Some(current));
    }
    if roll < P_ACTS {
        return pick(&cat.acts, Some(current));
    }
    // 移动分支
    if !no_move && can_move && !cat.moves.is_empty() {
        return pick(&cat.moves, None);
    }
    pick(&cat.acts, Some(current))
}

// ---- 简易随机（xorshift + 时间种子，零依赖）----
use std::sync::atomic::{AtomicU64, Ordering};

static SEED: AtomicU64 = AtomicU64::new(0x9E3779B97F4A7C15);

fn next_rand() -> u64 {
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    x
}

fn rand_u32() -> u32 {
    next_rand() as u32
}

pub fn rand_f64() -> f64 {
    (next_rand() >> 11) as f64 / (1u64 << 53) as f64
}

/// 用系统时间做种子，避免每次启动动画序列相同。
pub fn init_random() {
    use std::time::{SystemTime, UNIX_EPOCH};
    if let Ok(d) = SystemTime::now().duration_since(UNIX_EPOCH) {
        let t = d.as_nanos() as u64;
        SEED.store(t ^ 0x9E3779B97F4A7C15, Ordering::Relaxed);
    }
}
