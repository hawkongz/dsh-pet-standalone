//! 动画目录 + 动画链状态机（1:1 移植原 dsh-pet client.js / window.py 行为）。
use crate::assets;

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

/// 动画目录：从素材表构建。
pub struct Catalog {
    pub idle: String,
    pub turn: String,
    pub moves: Vec<String>,
    pub clicks: Vec<String>,
    pub drag: String,
    pub acts: Vec<String>,
    pub names: Vec<String>,
}

impl Catalog {
    pub fn from_assets() -> Catalog {
        let names: Vec<String> = assets::ANIMS.iter().map(|(n, _, _)| n.to_string()).collect();
        let idle = "待机呼吸休闲".to_string();
        let turn = "东张西望".to_string();
        let moves = vec![
            "螃蟹走路".to_string(),
            "原地漂浮踏步".to_string(),
            "原地左转奔跑".to_string(),
        ];
        let clicks = vec![
            "点击回应 - 开心跃动".to_string(),
            "点击回应 - 害羞惊讶".to_string(),
            "点击回应 - 傲娇生气（侧身展示）".to_string(),
        ];
        let drag = "被鼠标拖拽悬空反馈".to_string();
        let special: Vec<&str> = vec![
            "待机呼吸休闲",
            "东张西望",
            "螃蟹走路",
            "原地漂浮踏步",
            "原地左转奔跑",
            "点击回应 - 开心跃动",
            "点击回应 - 害羞惊讶",
            "点击回应 - 傲娇生气（侧身展示）",
            "被鼠标拖拽悬空反馈",
        ];
        let acts: Vec<String> = names
            .iter()
            .filter(|n| !special.contains(&n.as_str()))
            .cloned()
            .collect();
        Catalog {
            idle,
            turn,
            moves,
            clicks,
            drag,
            acts,
            names,
        }
    }

    /// 从名称池随机选（排除 exclude）。
    pub fn pick(pool: &[String], exclude: Option<&str>) -> String {
        if pool.is_empty() {
            return String::new();
        }
        let mut idx = (rand_u32() as usize) % pool.len();
        if let Some(e) = exclude {
            // 避免选中与当前相同（池 > 1 时）
            if pool.len() > 1 && pool[idx] == e {
                idx = (idx + 1) % pool.len();
            }
        }
        pool[idx].clone()
    }
}

/// 动画链：30% 待机 / 10% 转向 / 40% 动作 / 20% 移动。
/// no_move 时移动概率并入动作。
pub fn pick_next(catalog: &Catalog, current: &str, no_move: bool, can_move: bool) -> String {
    let roll = rand_f64();
    if roll < P_IDLE {
        return catalog.idle.clone();
    }
    if roll < P_TURN {
        return catalog.turn.clone();
    }
    if roll < P_ACTS {
        return Catalog::pick(&catalog.acts, Some(current));
    }
    // 移动分支
    if !no_move && can_move {
        return Catalog::pick(&catalog.moves, None);
    }
    Catalog::pick(&catalog.acts, Some(current))
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
