//! 解码管线验证：解析 webm → libvpx 解码 → 合成 BGRA，检查 alpha 正确性。
#[path = "../webm.rs"]
mod webm;
#[path = "../vpx.rs"]
mod vpx;
#[path = "../clip.rs"]
mod clip;

include!(concat!(env!("OUT_DIR"), "/assets_gen.rs"));

fn main() {
    // 1. 全部动画都能解析出帧
    let mut total_frames = 0usize;
    let mut ok = 0;
    let mut fail = 0;
    for (name, start, len) in ANIMS {
        let data = &ASSET_PAK[*start..*start + *len];
        let w = webm::WebM::parse(data);
        match w {
            Some(wm) => {
                ok += 1;
                total_frames += wm.frames.len();
                if wm.frames.is_empty() {
                    println!("[WARN] {} 无帧", name);
                }
            }
            None => {
                fail += 1;
                if fail <= 3 {
                    println!("[FAIL] {} ({}-{}) 解析失败，前16字节: {:02x?}", name, start, len, &data[..16usize.min(*len)]);
                }
            }
        }
    }
    println!("=== 解析成功 {} / {}，总帧数 {} ===", ok, ANIMS.len(), total_frames);

    // 2. 待机动画解码第一帧，验证 alpha
    let target = "待机呼吸休闲";
    let (_, start, len) = ANIMS.iter().find(|(n, _, _)| *n == target).unwrap();
    let data = &ASSET_PAK[*start..*start + *len];
    let wm = webm::WebM::parse(data).unwrap();
    println!("{}: {} 帧, fps={:.1}, 尺寸 {}x{}", target, wm.frames.len(), wm.fps, wm.width, wm.height);

    let mut dec = clip::ClipDecoder::new(std::rc::Rc::new(wm)).unwrap();

    // 直接解码 image 结构检查
    {
        let wm2 = webm::WebM::parse(data).unwrap();
        let f0 = &wm2.frames[0];
        // 固定 1 线程：多线程 (row-mt) 解码存在数据竞争，会导致同帧两次解码像素不同。
        let mut vdec = vpx::Decoder::new(vpx::VPX_THREADS).unwrap();
        let img = vdec.decode(&f0.video).unwrap();
        println!("image: w={} h={} d_w={} d_h={} fmt=0x{:x} bitdepth={} xcs={} ycs={} stride={:?}",
            img.w, img.h, img.d_w, img.d_h, img.fmt, img.bit_depth, img.x_chroma_shift, img.y_chroma_shift, img.stride);
        if let Some(a) = &f0.alpha {
            let mut adec = vpx::Decoder::new(vpx::VPX_THREADS).unwrap();
            let aimg = adec.decode(a).unwrap();
            println!("alpha image: w={} h={} d_w={} d_h={} fmt=0x{:x} stride={:?}",
                aimg.w, aimg.h, aimg.d_w, aimg.d_h, aimg.fmt, aimg.stride);
        }
        // 回归断言：同帧用两个独立 1 线程解码器各解一次，亮度平面输出必须逐字节一致；
        // 若有人把线程数调回多线程（row-mt 数据竞争）会在此处失败。
        let luma_bytes = || -> Vec<u8> {
            let mut dd = vpx::Decoder::new(vpx::VPX_THREADS).unwrap();
            let img = dd.decode(&f0.video).unwrap();
            let mut v = Vec::new();
            let p = img.planes[vpx::VPX_PLANE_Y];
            for y in 0..img.d_h as usize {
                let row = unsafe { std::slice::from_raw_parts(p.add(y * img.stride[0] as usize), img.d_w as usize) };
                v.extend_from_slice(row);
            }
            v
        };
        let b1 = luma_bytes();
        let b2 = luma_bytes();
        if b1 == b2 {
            println!("解码确定性: 同帧两次解码逐字节一致 ({} B)", b1.len());
        } else {
            panic!("解码不确定性: 两帧输出不一致（发现多线程竞争？）");
        }
    }

    let t0 = std::time::Instant::now();
    let frame = dec.next_frame().expect("解码第一帧失败");
    let dec_ms = t0.elapsed().as_millis();
    println!("第一帧解码耗时: {}ms, 缓冲 {}B", dec_ms, frame.len());

    let mut a0 = 0u32;
    let mut a_lt128 = 0u32;
    let mut a255 = 0u32;
    let mut total = 0u32;
    for i in (3..frame.len()).step_by(4) {
        let a = frame[i];
        total += 1;
        if a == 0 { a0 += 1; }
        if a < 128 { a_lt128 += 1; }
        if a >= 250 { a255 += 1; }
    }
    println!("alpha 分布: total={} a=0:{:.1}% a<128:{:.1}% a>=250:{:.1}%",
        total,
        a0 as f64 * 100.0 / total as f64,
        a_lt128 as f64 * 100.0 / total as f64,
        a255 as f64 * 100.0 / total as f64,
    );

    // 非全透明（有实际画面）
    assert!(a255 > 0, "没有完全不透明像素，解码可能失败");
    assert!(a0 < total, "全透明帧");

    // 3. 连续解码 10 帧测性能
    let t0 = std::time::Instant::now();
    let mut n = 0;
    while let Some(_) = dec.next_frame() {
        n += 1;
        if n >= 20 {
            break;
        }
    }
    let dur = t0.elapsed().as_millis();
    println!("连续解码 {} 帧耗时 {}ms, 平均 {:.2}ms/帧", n, dur, dur as f64 / n as f64);
    assert!((dur as f64 / n as f64) < 40.0, "解码性能不足，达不到 24fps");
    println!("\n=== 解码管线验证通过 ===");
}
