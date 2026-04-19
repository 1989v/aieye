//! 트레이 아이콘: 눈(aieye) 테마.
//! - idle: 감은 눈 (곡선)
//! - generating: 깜빡임 6프레임
//! - finished: 활짝 뜬 눈 (pupil + 하이라이트)
//!
//! 44x44 @2x template icon (검정 ink + alpha).

use image::{ImageBuffer, Rgba, RgbaImage};

const SIZE: u32 = 44;
const CX: f32 = 22.0;
const CY: f32 = 22.0;
const EYE_HALF_W: f32 = 16.0;
const EYE_MAX_HALF_H: f32 = 9.0;

pub struct TrayIcons {
    pub idle: Vec<u8>,
    pub finished: Vec<u8>,
    /// 6프레임 깜빡임: [open, half-closing, near-closed, closed, near-closed, half-closing]
    pub generating: Vec<Vec<u8>>,
}

pub fn generate_all() -> TrayIcons {
    // 깜빡임 패턴: 한 싸이클당 open→close→open
    let frames = [1.0_f32, 0.7, 0.3, 0.0, 0.3, 0.7];
    let icons = TrayIcons {
        idle: render_png(draw_eye(0.0, false)),
        finished: render_png(draw_eye(1.0, true)),
        generating: frames
            .iter()
            .map(|o| render_png(draw_eye(*o, true)))
            .collect(),
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

/// 눈 그리기.
/// openness: 0.0(감음) ~ 1.0(활짝 뜸)
/// with_pupil: openness 가 충분히 크면 pupil + 하이라이트 그림
fn draw_eye(openness: f32, with_pupil: bool) -> RgbaImage {
    let mut c = new_canvas();
    let lid_half_h = EYE_MAX_HALF_H * openness;

    // 상/하 eyelid 를 parabolic curve 로: y_top = cy - lid_half_h * (1 - (x/w)^2)
    // 두껍게 2px
    let steps = 120;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = -EYE_HALF_W + 2.0 * EYE_HALF_W * t; // -w..=w
        let ratio = 1.0 - (x / EYE_HALF_W).powi(2);
        if ratio < 0.0 {
            continue;
        }
        let dy = lid_half_h * ratio;
        let px = CX + x;
        // 상단 eyelid
        stamp_circle(&mut c, px, CY - dy, 1.3, 255);
        // 하단 eyelid
        if openness > 0.02 {
            stamp_circle(&mut c, px, CY + dy, 1.3, 255);
        }
    }

    if with_pupil && openness > 0.35 {
        // iris (테두리 링) + pupil (검은 점)
        let iris_r = 4.2;
        let pupil_r = 2.6;
        // 눈동자는 openness 가 작을 때 눈꺼풀에 가려지게 (마스크처럼)
        let visible_h = (EYE_MAX_HALF_H * openness - 1.5).max(0.0);
        draw_clipped_circle(&mut c, CX, CY, iris_r, visible_h, 170);
        draw_clipped_circle(&mut c, CX, CY, pupil_r, visible_h, 255);
        // 하이라이트: 상단 우측 작은 반점 (alpha 0 구멍)
        if openness > 0.7 {
            punch_circle(&mut c, CX + 1.1, CY - 1.2, 0.7);
        }
    }
    c
}

/// circle 을 찍되 상/하 eyelid 영역을 벗어난 부분은 자름.
/// visible_h: 중심에서 위/아래로 이 높이 이내만 그림.
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

/// 지정 반경 내 픽셀의 alpha 를 0 으로 (하이라이트용 펀치).
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

/// 위치 (fx, fy) 에 반경 r 의 작은 원을 anti-aliased 느낌으로 스탬프.
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
                // linear falloff in the outer ring
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
