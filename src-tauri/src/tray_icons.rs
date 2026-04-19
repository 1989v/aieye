//! 트레이 아이콘: 파일 기반 로딩. `src-tauri/icons/tray/` 에 있는 PNG 를
//! 런타임에 읽어들임. 파일 없으면 programmatic fallback.
//!
//! 기대 파일 (없으면 fallback):
//!   idle.png / idle@2x.png
//!   finished.png / finished@2x.png
//!   gen-0.png ~ gen-5.png (+ @2x)
//!
//! @2x 가 있으면 우선, 없으면 @1x 사용. 둘 다 없으면 programmatic fallback.

use image::{ImageBuffer, Rgba, RgbaImage};

const SIZE: u32 = 44;
/// 트레이 아이콘 기본 색상 (macOS system blue 톤).
/// alpha 는 원본 유지 (AA 엣지 보존).
const TINT_R: u8 = 77;
const TINT_G: u8 = 163;
const TINT_B: u8 = 255;
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
    // 파일 탐색 기준: CARGO_MANIFEST_DIR/icons/tray/
    // 번들에서 실행될 땐 bundle 내부 resource 참조는 불편하므로 런타임 개발 편의상
    // 현재 실행 경로 기반 몇 군데 순회.
    // exe 위치 기준 여러 후보 (tauri bundle 구조 변동 대비)
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    let candidate_dirs = [
        // dev: cargo run / debug build
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("icons/tray"),
        // 표준 macOS app: .app/Contents/Resources/icons/tray
        exe_dir.join("../Resources/icons/tray"),
        // tauri resources 경로 변형
        exe_dir.join("../Resources/_up_/icons/tray"),
        // fallback: exe 바로 옆
        exe_dir.join("icons/tray"),
    ];

    let loader = |name: &str| {
        for base in &candidate_dirs {
            if let Some(bytes) = try_load(base, name) {
                tracing::info!("tray icon loaded from disk: {name}");
                // 원본이 검정 곡선만 있는 closed-eye 프레임만 파란색 리컬러.
                // 나머지는 사용자가 만든 파란 디자인 그대로 사용 (디테일 보존).
                let needs_recolor = matches!(name, "idle" | "gen-3");
                return Some(if needs_recolor {
                    recolor_blue(&bytes).unwrap_or(bytes)
                } else {
                    bytes
                });
            }
        }
        None
    };

    let frames_names = ["gen-0", "gen-1", "gen-2", "gen-3", "gen-4", "gen-5"];
    let frames_loaded: Vec<Vec<u8>> = frames_names
        .iter()
        .map(|n| loader(n))
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect();

    let icons = TrayIcons {
        idle: loader("idle").unwrap_or_else(|| render_png(draw_eye(0.0, false))),
        finished: loader("finished").unwrap_or_else(|| render_png(draw_eye(1.0, true))),
        // 6개가 전부 있어야 애니메이션이 매끄러움 — 하나라도 누락되면 전부 fallback
        generating: if frames_loaded.len() == frames_names.len() {
            frames_loaded
        } else {
            let defaults = [1.0_f32, 0.7, 0.3, 0.0, 0.3, 0.7];
            defaults
                .iter()
                .map(|o| render_png(draw_eye(*o, true)))
                .collect()
        },
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

/// PNG 바이트 입력 → 불투명 픽셀 전부를 TINT_* 블루로 치환, alpha 는 보존.
/// AA 엣지는 원본 alpha 에 의해 자연스럽게 유지됨. 실패 시 원본 유지.
fn recolor_blue(png: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(png).ok()?;
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        if p.0[3] > 0 {
            p.0[0] = TINT_R;
            p.0[1] = TINT_G;
            p.0[2] = TINT_B;
        }
    }
    let mut out = Vec::new();
    rgba
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// @2x 우선, 없으면 @1x. 둘 다 없으면 None.
fn try_load(base: &std::path::Path, stem: &str) -> Option<Vec<u8>> {
    let hi = base.join(format!("{stem}@2x.png"));
    if hi.exists() {
        if let Ok(bytes) = std::fs::read(&hi) {
            return Some(bytes);
        }
    }
    let lo = base.join(format!("{stem}.png"));
    if lo.exists() {
        if let Ok(bytes) = std::fs::read(&lo) {
            return Some(bytes);
        }
    }
    None
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
