//! 配置持久化：%APPDATA%\dsh-pet-standalone\config.json（手写极简 JSON，零依赖）。
//! 支持多桌宠：pets 数组，每只宠物独立记录位置/朝向/大小等。
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct PetConfig {
    pub rx: Option<f64>,
    pub ry: Option<f64>,
    pub facing: String,
    pub scale: f64,
    pub on_top: bool,
    pub no_move: bool,
}

impl PetConfig {
    pub fn new() -> PetConfig {
        PetConfig {
            rx: None,
            ry: None,
            facing: "left".to_string(),
            scale: 0.72,
            on_top: true,
            no_move: false,
        }
    }
}

impl Default for PetConfig {
    fn default() -> Self {
        PetConfig::new()
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub dir: PathBuf,
    pub pets: Vec<PetConfig>,
}

impl Config {
    pub fn load() -> Config {
        let base = env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| env::var("USERPROFILE").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(".")));
        let dir = base.join("dsh-pet-standalone");
        let path = dir.join("config.json");
        let mut cfg = Config {
            dir,
            pets: vec![PetConfig::default()],
        };
        if let Ok(text) = fs::read_to_string(&path) {
            cfg.parse(&text);
        }
        if cfg.pets.is_empty() {
            cfg.pets.push(PetConfig::default());
        }
        cfg
    }

    /// 极简 JSON 解析（只读需要的字段）。
    fn parse(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let n = bytes.len();
        let mut i = 0usize;
        // 找到 "pets" 数组
        while i < n {
            skip_ws(bytes, &mut i, n);
            if i >= n {
                break;
            }
            if bytes[i] == b'"' {
                i += 1;
                let key_start = i;
                while i < n && bytes[i] != b'"' {
                    i += 1;
                }
                let key = String::from_utf8_lossy(&bytes[key_start..i]).to_string();
                i += 1;
                skip_ws(bytes, &mut i, n);
                if i < n && bytes[i] == b':' {
                    i += 1;
                }
                if key == "pets" {
                    self.parse_pets(bytes, &mut i, n);
                } else {
                    skip_value(bytes, &mut i, n);
                }
            } else {
                i += 1;
            }
        }
    }

    /// 解析 pets 数组：[{...}, {...}]
    fn parse_pets(&mut self, bytes: &[u8], i: &mut usize, n: usize) {
        skip_ws(bytes, i, n);
        if *i >= n || bytes[*i] != b'[' {
            return;
        }
        *i += 1;
        let mut pets: Vec<PetConfig> = Vec::new();
        loop {
            skip_ws(bytes, i, n);
            if *i >= n {
                break;
            }
            if bytes[*i] == b']' {
                *i += 1;
                break;
            }
            if bytes[*i] == b'{' {
                if let Some(pc) = parse_pet_object(bytes, i, n) {
                    pets.push(pc);
                }
            } else {
                *i += 1;
            }
        }
        if !pets.is_empty() {
            self.pets = pets;
        }
    }

    pub fn save(&self) {
        let _ = fs::create_dir_all(&self.dir);
        let mut s = String::from("{\n  \"version\": 2,\n  \"pets\": [\n");
        for (idx, p) in self.pets.iter().enumerate() {
            s.push_str(&format!(
                "    {{\"rx\": {}, \"ry\": {}, \"facing\": \"{}\", \"scale\": {}, \"on_top\": {}, \"no_move\": {}}}",
                p.rx.map(|v| format!("{:.6}", v)).unwrap_or_else(|| "null".into()),
                p.ry.map(|v| format!("{:.6}", v)).unwrap_or_else(|| "null".into()),
                p.facing,
                p.scale,
                p.on_top,
                p.no_move,
            ));
            if idx + 1 < self.pets.len() {
                s.push(',');
            }
            s.push('\n');
        }
        s.push_str("  ]\n}\n");
        let _ = fs::write(self.dir.join("config.json"), s);
    }
}

/// 解析单个宠物对象 {...}
fn parse_pet_object(bytes: &[u8], i: &mut usize, n: usize) -> Option<PetConfig> {
    // 跳过 '{'
    if *i >= n || bytes[*i] != b'{' {
        return None;
    }
    *i += 1;
    let mut pc = PetConfig::default();
    loop {
        skip_ws(bytes, i, n);
        if *i >= n {
            break;
        }
        if bytes[*i] == b'}' {
            *i += 1;
            break;
        }
        if bytes[*i] != b'"' {
            *i += 1;
            continue;
        }
        *i += 1;
        let ks = *i;
        while *i < n && bytes[*i] != b'"' {
            *i += 1;
        }
        let key = String::from_utf8_lossy(&bytes[ks..*i]).to_string();
        *i += 1;
        skip_ws(bytes, i, n);
        if *i < n && bytes[*i] == b':' {
            *i += 1;
        }
        skip_ws(bytes, i, n);
        let (val, next) = read_value(bytes, *i, n);
        *i = next;
        match key.as_str() {
            "facing" => pc.facing = val.to_string(),
            "scale" => pc.scale = val.trim().parse().unwrap_or(0.72),
            "on_top" => pc.on_top = val.trim() == "true",
            "no_move" => pc.no_move = val.trim() == "true",
            "rx" => pc.rx = val.trim().parse().ok(),
            "ry" => pc.ry = val.trim().parse().ok(),
            _ => {}
        }
    }
    Some(pc)
}

/// 读取一个值（字符串或数字/布尔），返回 (字符串形式, 新位置)。
fn read_value(bytes: &[u8], mut i: usize, n: usize) -> (String, usize) {
    skip_ws(bytes, &mut i, n);
    if i < n && bytes[i] == b'"' {
        i += 1;
        let s = i;
        while i < n && bytes[i] != b'"' {
            i += 1;
        }
        let val = String::from_utf8_lossy(&bytes[s..i]).to_string();
        return (val, i + 1);
    }
    let s = i;
    while i < n && bytes[i] != b',' && bytes[i] != b'}' && bytes[i] != b']' {
        i += 1;
    }
    (String::from_utf8_lossy(&bytes[s..i]).to_string(), i)
}

/// 跳过 JSON 值（用于非 pets 的键，如旧版单宠物字段）。
fn skip_value(bytes: &[u8], i: &mut usize, n: usize) {
    skip_ws(bytes, i, n);
    let mut depth = 0;
    while *i < n {
        match bytes[*i] {
            b'[' | b'{' => depth += 1,
            b']' | b'}' => {
                if depth == 0 {
                    return;
                }
                depth -= 1;
            }
            b',' if depth == 0 => {
                *i += 1;
                return;
            }
            _ => {}
        }
        *i += 1;
    }
}

fn skip_ws(bytes: &[u8], i: &mut usize, n: usize) {
    while *i < n && (bytes[*i] == b' ' || bytes[*i] == b'\n' || bytes[*i] == b'\t' || bytes[*i] == b'\r') {
        *i += 1;
    }
}
