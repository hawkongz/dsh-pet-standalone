//! 嵌入素材（build.rs 生成 assets.pak + assets_gen.rs）。
include!(concat!(env!("OUT_DIR"), "/assets_gen.rs"));

/// 按动画名取 webm 字节。
pub fn asset(name: &str) -> Option<&'static [u8]> {
    for (n, start, len) in ANIMS {
        if *n == name {
            return Some(&ASSET_PAK[*start..*start + len]);
        }
    }
    None
}
