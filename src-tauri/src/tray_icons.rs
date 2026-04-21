//! 트레이 아이콘: 파일 기반 로딩. `src-tauri/icons/tray/` 에 있는 PNG 를
//! 런타임에 읽어들임. 파일 없으면 programmatic fallback.
//!
//! 기대 파일 (Figma blink 세트, 없으면 fallback):
//!   blink_{22,44,88}_f{0..6}.png
//!     f0 = fully open (idle), f6 = fully closed
//!   런타임은 88px 을 메인으로 사용 (22/44 는 번들 동봉 — 향후 확장/툴 용).
//!
//! 애니메이션은 ping-pong (f0→f6→f1) 12 프레임 순환 = 자연스러운 눈 깜빡임.

use image::{ImageBuffer, Rgba, RgbaImage};

const SIZE: u32 = 44;
const CX: f32 = 22.0;
const CY: f32 = 22.0;
const EYE_HALF_W: f32 = 16.0;
const EYE_MAX_HALF_H: f32 = 9.0;

pub struct TrayIcons {
    pub idle: Vec<u8>,
    pub finished: Vec<u8>,
    pub generating: Vec<Vec<u8>>,
}

pub fn generate_all() -> TrayIcons {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    let candidate_dirs = [
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons/tray"),
        exe_dir.join("../Resources/icons/tray"),
        exe_dir.join("../Resources/_up_/icons/tray"),
        exe_dir.join("icons/tray"),
    ];

    let load = |name: &str| -> Option<Vec<u8>> {
        for base in &candidate_dirs {
            let path = base.join(format!("{name}.png"));
            if path.exists() {
                if let Ok(bytes) = std::fs::read(&path) {
                    tracing::info!("tray icon loaded from disk: {name}");
                    return Some(bytes);
                }
            }
        }
        None
    };

    // 88px 프레임을 메인으로 로드 (f0 = open, f6 = closed).
    let frames: Vec<Option<Vec<u8>>> =
        (0..=6).map(|i| load(&format!("blink_88_f{i}"))).collect();
    let all_ok = frames.iter().all(|f| f.is_some());

    let (idle_bytes, generating_seq) = if all_ok {
        let f: Vec<Vec<u8>> = frames.into_iter().flatten().collect();
        // ping-pong: f0,f1,f2,f3,f4,f5,f6,f5,f4,f3,f2,f1 (12 프레임)
        let seq: Vec<Vec<u8>> = [0, 1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1]
            .iter()
            .map(|&i| f[i].clone())
            .collect();
        (f[0].clone(), seq)
    } else {
        // programmatic fallback — 모든 프레임이 있어야 ping-pong 이 매끄러움.
        let defaults = [1.0_f32, 0.83, 0.66, 0.5, 0.33, 0.16, 0.0];
        let rendered: Vec<Vec<u8>> = defaults
            .iter()
            .map(|o| render_png(draw_eye(*o, true)))
            .collect();
        let seq: Vec<Vec<u8>> = [0, 1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1]
            .iter()
            .map(|&i| rendered[i].clone())
            .collect();
        (rendered[0].clone(), seq)
    };

    let icons = TrayIcons {
        // finished 는 별도 디자인이 없으므로 f0 (open) 공유 + 숫자 배지로 구분.
        idle: idle_bytes.clone(),
        finished: idle_bytes,
        generating: generating_seq,
    };

    if let Some(dir) = std::env::var_os("AIEYE_DUMP_ICONS") {
        let d = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("idle.png"), &icons.idle);
        let _ = std::fs::write(d.join("finished.png"), &icons.finished);
        for (i, png) in icons.generating.iter().enumerate() {
            let _ = std::fs::write(d.join(format!("gen-{i}.png")), png);
        }
    }
    icons
}

fn render_png(img: RgbaImage) -> Vec<u8> {
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .expect("png encode");
    out
}

fn new_canvas() -> RgbaImage {
    ImageBuffer::from_pixel(SIZE, SIZE, Rgba([0, 0, 0, 0]))
}

fn ink(alpha: u8) -> Rgba<u8> {
    Rgba([0, 0, 0, alpha])
}

fn draw_eye(openness: f32, with_pupil: bool) -> RgbaImage {
    let mut c = new_canvas();
    let lid_half_h = EYE_MAX_HALF_H * openness;
    let steps = 120;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = -EYE_HALF_W + 2.0 * EYE_HALF_W * t;
        let ratio = 1.0 - (x / EYE_HALF_W).powi(2);
        if ratio < 0.0 {
            continue;
        }
        let dy = lid_half_h * ratio;
        let px = CX + x;
        stamp_circle(&mut c, px, CY - dy, 1.3, 255);
        if openness > 0.02 {
            stamp_circle(&mut c, px, CY + dy, 1.3, 255);
        }
    }
    if with_pupil && openness > 0.35 {
        let iris_r = 4.2;
        let pupil_r = 2.6;
        let visible_h = (EYE_MAX_HALF_H * openness - 1.5).max(0.0);
        draw_clipped_circle(&mut c, CX, CY, iris_r, visible_h, 170);
        draw_clipped_circle(&mut c, CX, CY, pupil_r, visible_h, 255);
        if openness > 0.7 {
            punch_circle(&mut c, CX + 1.1, CY - 1.2, 0.7);
        }
    }
    c
}

fn draw_clipped_circle(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, visible_h: f32, alpha: u8) {
    let r_i = r.ceil() as i32;
    let r2 = r * r;
    for dy in -r_i..=r_i {
        for dx in -r_i..=r_i {
            let fx = dx as f32;
            let fy = dy as f32;
            if fx * fx + fy * fy > r2 {
                continue;
            }
            if fy.abs() > visible_h {
                continue;
            }
            put_pixel(img, (cx + fx) as i32, (cy + fy) as i32, ink(alpha));
        }
    }
}

fn punch_circle(img: &mut RgbaImage, cx: f32, cy: f32, r: f32) {
    let r_i = r.ceil() as i32;
    let r2 = r * r;
    for dy in -r_i..=r_i {
        for dx in -r_i..=r_i {
            let fx = dx as f32;
            let fy = dy as f32;
            if fx * fx + fy * fy > r2 {
                continue;
            }
            let x = (cx + fx) as i32;
            let y = (cy + fy) as i32;
            if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 {
                continue;
            }
            img.put_pixel(x as u32, y as u32, Rgba([0, 0, 0, 0]));
        }
    }
}

fn stamp_circle(img: &mut RgbaImage, fx: f32, fy: f32, r: f32, alpha: u8) {
    let r_i = r.ceil() as i32;
    let r_outer = r + 0.5;
    let r_inner = (r - 0.5).max(0.0);
    for dy in -r_i..=r_i {
        for dx in -r_i..=r_i {
            let px = fx + dx as f32;
            let py = fy + dy as f32;
            let d = ((px - fx).powi(2) + (py - fy).powi(2)).sqrt();
            if d > r_outer {
                continue;
            }
            let edge_alpha = if d < r_inner {
                alpha
            } else {
                let t = ((r_outer - d) / (r_outer - r_inner)).clamp(0.0, 1.0);
                (alpha as f32 * t) as u8
            };
            let x = px.round() as i32;
            let y = py.round() as i32;
            blend_pixel(img, x, y, edge_alpha);
        }
    }
}

fn blend_pixel(img: &mut RgbaImage, x: i32, y: i32, alpha: u8) {
    if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 {
        return;
    }
    let existing = img.get_pixel(x as u32, y as u32).0[3];
    if alpha > existing {
        img.put_pixel(x as u32, y as u32, ink(alpha));
    }
}

fn put_pixel(img: &mut RgbaImage, x: i32, y: i32, px: Rgba<u8>) {
    if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 {
        return;
    }
    img.put_pixel(x as u32, y as u32, px);
}
