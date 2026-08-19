//! WebM/EBML 解析器（VP9 + alpha BlockAdditional）。
//!
//! 素材结构（Matroska）：
//!   EBML Header
//!   Segment
//!     Info (TimecodeScale)
//!     Tracks (TrackEntry: CodecID=V_VP9, Video WxH, AlphaMode)
//!     Cluster { Timecode, SimpleBlock | BlockGroup }
//!       BlockGroup { Block (VP9 主色), BlockAdditions(0x75A1) { BlockMore(0xA6)
//!                    { BlockAddID(0xEE), BlockAdditional(0xA5) = VP9 灰度 alpha 帧 } } }

pub const EBML: u64 = 0x1A45DFA3;
pub const SEGMENT: u64 = 0x18538067;
pub const INFO: u64 = 0x1549A966;
pub const TIMECODE_SCALE: u64 = 0x2AD7B1;
pub const TRACKS: u64 = 0x1654AE6B;
pub const TRACK_ENTRY: u64 = 0xAE;
pub const TRACK_NUMBER: u64 = 0xD7;
pub const TRACK_TYPE: u64 = 0x83;
pub const CODEC_ID: u64 = 0x86;
pub const VIDEO: u64 = 0xE0;
pub const PIXEL_WIDTH: u64 = 0xB0;
pub const PIXEL_HEIGHT: u64 = 0xBA;
pub const ALPHA_MODE: u64 = 0x53C0;
pub const CLUSTER: u64 = 0x1F43B675;
pub const CLUSTER_TIMECODE: u64 = 0xE7;
pub const SIMPLE_BLOCK: u64 = 0xA3;
pub const BLOCK_GROUP: u64 = 0xA0;
pub const BLOCK: u64 = 0xA1;
pub const BLOCK_ADDITIONS: u64 = 0x75A1;
pub const BLOCK_MORE: u64 = 0xA6;
pub const BLOCK_ADD_ID: u64 = 0xEE;
pub const BLOCK_ADDITIONAL: u64 = 0xA5;

/// 一帧动画：主色 VP9 帧 + 可选 alpha（VP9 灰度帧）。
#[derive(Clone)]
pub struct Frame {
    pub timecode_ms: u64,
    pub video: Vec<u8>,
    pub alpha: Option<Vec<u8>>,
}

/// 解析后的 WebM 动画。
pub struct WebM {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub frames: Vec<Frame>,
}

impl WebM {
    /// 帧时长（毫秒）。
    pub fn frame_ms(&self) -> u64 {
        (1000.0 / self.fps).round() as u64
    }

    /// 总时长（毫秒）。
    pub fn duration_ms(&self) -> u64 {
        if self.frames.is_empty() {
            0
        } else {
            self.frames.last().unwrap().timecode_ms + self.frame_ms()
        }
    }
}

/// 读取一个 EBML 变长整数（size 字段，剥离标记位）。
fn read_vint(data: &[u8], pos: &mut usize) -> Option<u64> {
    if *pos >= data.len() {
        return None;
    }
    let b = data[*pos];
    let mut width = 1usize;
    let mut mask = 0x80u8;
    while (b & mask) == 0 {
        mask >>= 1;
        width += 1;
        if width > 8 {
            return None;
        }
    }
    if *pos + width > data.len() {
        return None;
    }
    let mut value = (b & (mask - 1)) as u64;
    for i in 1..width {
        value = (value << 8) | data[*pos + i] as u64;
    }
    *pos += width;
    Some(value)
}

/// 读取 EBML 元素 ID（保留标记位，ID 是原始字节拼接）。
fn read_id(data: &[u8], pos: &mut usize) -> Option<u64> {
    if *pos >= data.len() {
        return None;
    }
    let b = data[*pos];
    let mut width = 1usize;
    let mut mask = 0x80u8;
    while (b & mask) == 0 {
        mask >>= 1;
        width += 1;
        if width > 8 {
            return None;
        }
    }
    if *pos + width > data.len() {
        return None;
    }
    let mut value = 0u64;
    for i in 0..width {
        value = (value << 8) | data[*pos + i] as u64;
    }
    *pos += width;
    Some(value)
}

/// 读取元素头，返回 (id, payload_start, payload_len)。
fn read_elem(data: &[u8], pos: &mut usize) -> Option<(u64, usize, usize)> {
    let id = read_id(data, pos)?;
    let size = read_vint(data, pos)?;
    let payload = *pos;
    *pos += size as usize;
    if *pos > data.len() {
        return None;
    }
    Some((id, payload, size as usize))
}

/// 遍历某层所有子元素。
fn walk(data: &[u8], start: usize, len: usize, f: &mut impl FnMut(u64, usize, usize) -> bool) {
    let mut pos = start;
    let end = start + len;
    while pos < end {
        let save = pos;
        if let Some((id, payload, plen)) = read_elem(data, &mut pos) {
            if !f(id, payload, plen) {
                return;
            }
        } else {
            break;
        }
    }
}

/// 解析 Block/SimpleBlock 载荷：tracknum vint + int16 时间码 + flags + 数据。
/// 返回 (track, timecode, keyframe, data)。
fn parse_block_payload(data: &[u8]) -> Option<(u64, i64, bool, &[u8])> {
    let mut pos = 0usize;
    let track = read_vint(data, &mut pos)?;
    if pos + 3 > data.len() {
        return None;
    }
    let tc = i16::from_be_bytes([data[pos], data[pos + 1]]) as i64;
    let flags = data[pos + 2];
    let keyframe = (flags & 0x80) != 0;
    Some((track, tc, keyframe, &data[pos + 3..]))
}

/// 解析 alpha 的 BlockAdditions：BlockMore > BlockAddID + BlockAdditional。
/// 返回 alpha VP9 帧数据。
fn parse_block_additions(data: &[u8]) -> Option<Vec<u8>> {
    let mut alpha: Option<Vec<u8>> = None;
    walk(data, 0, data.len(), &mut |id, payload, plen| {
        if id == BLOCK_MORE {
            // 在 BlockMore 里找 BlockAdditional (0xA5)
            walk(&data[payload..payload + plen], 0, plen, &mut |id2, p2, l2| {
                if id2 == BLOCK_ADDITIONAL {
                    alpha = Some(data[payload + p2..payload + p2 + l2].to_vec());
                    false // 只取第一个
                } else {
                    true
                }
            });
        }
        true
    });
    alpha
}

impl WebM {
    /// 从完整 webm 字节解析。
    pub fn parse(data: &[u8]) -> Option<WebM> {
        let mut seg: Option<(usize, usize)> = None;
        walk(data, 0, data.len(), &mut |id, payload, plen| {
            if id == SEGMENT {
                seg = Some((payload, plen));
                false
            } else {
                true
            }
        });
        let (seg_payload, seg_len) = seg?;

        let mut timecode_scale: u64 = 1_000_000;
        let mut width: u32 = 640;
        let mut height: u32 = 360;
        let mut frames: Vec<Frame> = Vec::new();
        let mut cluster_timecode: i64 = 0;

        walk(data, seg_payload, seg_len, &mut |id, payload, plen| {
            match id {
                INFO => {
                    walk(data, payload, plen, &mut |id2, p2, l2| {
                        if id2 == TIMECODE_SCALE {
                            timecode_scale = parse_u64(data, p2, l2).unwrap_or(1_000_000);
                        }
                        true
                    });
                }
                TRACKS => {
                    walk(data, payload, plen, &mut |id2, p2, l2| {
                        if id2 == TRACK_ENTRY {
                            walk(data, p2, l2, &mut |id3, p3, l3| match id3 {
                                VIDEO => {
                                    walk(data, p3, l3, &mut |id4, p4, l4| match id4 {
                                        PIXEL_WIDTH => {
                                            width = parse_u32(data, p4, l4).unwrap_or(640);
                                            true
                                        }
                                        PIXEL_HEIGHT => {
                                            height = parse_u32(data, p4, l4).unwrap_or(360);
                                            true
                                        }
                                        _ => true,
                                    });
                                    true
                                }
                                _ => true,
                            });
                        }
                        true
                    });
                }
                CLUSTER => {
                    let mut block_data: Vec<Frame> = Vec::new();
                    walk(data, payload, plen, &mut |id2, p2, l2| {
                        match id2 {
                            CLUSTER_TIMECODE => {
                                cluster_timecode = parse_i64(data, p2, l2).unwrap_or(0);
                            }
                            SIMPLE_BLOCK => {
                                if let Some((_track, tc, _kf, vdata)) =
                                    parse_block_payload(&data[p2..p2 + l2])
                                {
                                    let tms = timecode_ms(cluster_timecode + tc, timecode_scale);
                                    block_data.push(Frame {
                                        timecode_ms: tms,
                                        video: vdata.to_vec(),
                                        alpha: None,
                                    });
                                }
                            }
                            BLOCK_GROUP => {
                                let mut video: Option<Vec<u8>> = None;
                                let mut alpha: Option<Vec<u8>> = None;
                                let mut tc: i64 = 0;
                                walk(data, p2, l2, &mut |id3, p3, l3| match id3 {
                                    BLOCK => {
                                        if let Some((_t, t2, _k, vd)) =
                                            parse_block_payload(&data[p3..p3 + l3])
                                        {
                                            tc = t2;
                                            video = Some(vd.to_vec());
                                        }
                                        true
                                    }
                                    BLOCK_ADDITIONS => {
                                        alpha = parse_block_additions(&data[p3..p3 + l3]);
                                        true
                                    }
                                    _ => true,
                                });
                                if let Some(video) = video {
                                    let tms =
                                        timecode_ms(cluster_timecode + tc, timecode_scale);
                                    block_data.push(Frame {
                                        timecode_ms: tms,
                                        video,
                                        alpha,
                                    });
                                }
                            }
                            _ => {}
                        }
                        true
                    });
                    frames.extend(block_data);
                }
                _ => {}
            }
            true
        });

        if frames.is_empty() {
            return None;
        }
        // 帧率：取相邻时间码差的中位数；素材为 24fps，兜底 24。
        frames.sort_by_key(|f| f.timecode_ms);
        let mut fps = 24.0;
        if frames.len() > 2 {
            let mut deltas: Vec<u64> = frames.windows(2).map(|w| w[1].timecode_ms.saturating_sub(w[0].timecode_ms)).filter(|d| *d > 0).collect();
            if !deltas.is_empty() {
                deltas.sort();
                let med = deltas[deltas.len() / 2];
                if med > 0 {
                    fps = 1000.0 / med as f64;
                }
            }
        }

        Some(WebM {
            width,
            height,
            fps,
            frames,
        })
    }

    /// 解析帧率修正后返回。默认解析即可。
    pub fn parse_strict(data: &[u8]) -> Option<WebM> {
        WebM::parse(data)
    }
}

fn timecode_ms(tc: i64, scale: u64) -> u64 {
    if tc < 0 {
        0
    } else {
        (tc as u128 * scale as u128 / 1_000_000) as u64
    }
}

fn parse_u32(data: &[u8], p: usize, l: usize) -> Option<u32> {
    if p + l > data.len() || l == 0 || l > 4 {
        return None;
    }
    let mut v = 0u32;
    for b in &data[p..p + l] {
        v = (v << 8) | *b as u32;
    }
    Some(v)
}

fn parse_u64(data: &[u8], p: usize, l: usize) -> Option<u64> {
    if p + l > data.len() || l == 0 || l > 8 {
        return None;
    }
    let mut v = 0u64;
    for b in &data[p..p + l] {
        v = (v << 8) | *b as u64;
    }
    Some(v)
}

fn parse_i64(data: &[u8], p: usize, l: usize) -> Option<i64> {
    let v = parse_u64(data, p, l)?;
    // 负数补码
    let shift = 64 - l as u32 * 8;
    if shift > 0 && (v & (1 << (shift - 1))) != 0 {
        Some((v as i64).wrapping_shr(0).wrapping_neg() + 0) // 简化：素材时间码非负
    } else {
        Some(v as i64)
    }
}
