//! 트레이 아이콘 프레임을 런타임에 프로그래밍으로 생성.
//! 22x22 @1x 기준. macOS template icon (검은 ink + alpha) 으로 사용하므로
//! dark/light 모드에서 자동으로 tint 됨.

use image::{ImageBuffer, Rgba, RgbaImage};

const SIZE: u32 = 44; // @2x 기준 더 선명

/// 생성 가능한 아이콘 종류.
pub struct TrayIcons {
    pub idle: Vec<u8>,
    pub finished: Vec<u8>,
    /// 6 프레임의 펄싱 도트 애니메이션.
    pub generating: Vec<Vec<u8>>,
}

pub fn generate_all() -> TrayIcons {
    TrayIcons {
        idle: render_png(draw_idle()),
        finished: render_png(draw_finished()),
        generating: (0..6).map(|i| render_png(draw_generating(i))).collect(),
    }
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

fn ink() -> Rgba<u8> {
    Rgba([0, 0, 0, 255])
}

/// idle: 눈 모양 (outer ring + pupil)
fn draw_idle() -> RgbaImage {
    let mut c = new_canvas();
    let cx = (SIZE / 2) as i32;
    let cy = (SIZE / 2) as i32;
    // outer circle outline
    draw_circle_outline(&mut c, cx, cy, 16, 3);
    // pupil
    draw_filled_circle(&mut c, cx, cy, 6);
    c
}

/// finished: 체크마크
fn draw_finished() -> RgbaImage {
    let mut c = new_canvas();
    // 체크마크 두 선분 (좌측 아래로 내려갔다 우측 위로 올라감)
    draw_thick_line(&mut c, 10, 22, 18, 30, 4);
    draw_thick_line(&mut c, 18, 30, 34, 14, 4);
    c
}

/// generating: 6프레임 — 중앙 도트 + 주위 6개 포지션 중 한 곳만 강조
fn draw_generating(frame: u32) -> RgbaImage {
    let mut c = new_canvas();
    let cx = (SIZE / 2) as f32;
    let cy = (SIZE / 2) as f32;
    draw_filled_circle(&mut c, cx as i32, cy as i32, 4);
    // 6개 도트를 원주 상에 배치, frame 번째만 진하게, 나머지는 옅게
    for i in 0..6 {
        let angle = (i as f32) * std::f32::consts::TAU / 6.0 - std::f32::consts::FRAC_PI_2;
        let x = (cx + angle.cos() * 14.0) as i32;
        let y = (cy + angle.sin() * 14.0) as i32;
        let alpha = if i == frame { 255u8 } else { 90 };
        draw_filled_circle_alpha(&mut c, x, y, 3, alpha);
    }
    c
}

fn draw_filled_circle(img: &mut RgbaImage, cx: i32, cy: i32, r: i32) {
    draw_filled_circle_alpha(img, cx, cy, r, 255);
}

fn draw_filled_circle_alpha(img: &mut RgbaImage, cx: i32, cy: i32, r: i32, alpha: u8) {
    let r2 = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r2 {
                put_pixel(img, cx + dx, cy + dy, Rgba([0, 0, 0, alpha]));
            }
        }
    }
}

fn draw_circle_outline(img: &mut RgbaImage, cx: i32, cy: i32, r: i32, thickness: i32) {
    let outer = r * r;
    let inner = (r - thickness) * (r - thickness);
    for dy in -r..=r {
        for dx in -r..=r {
            let d = dx * dx + dy * dy;
            if d <= outer && d >= inner {
                put_pixel(img, cx + dx, cy + dy, ink());
            }
        }
    }
}

fn draw_thick_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, thickness: i32) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let steps = dx.max(dy).max(1);
    let half = thickness / 2;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (x0 as f32 + (x1 - x0) as f32 * t) as i32;
        let y = (y0 as f32 + (y1 - y0) as f32 * t) as i32;
        for oy in -half..=half {
            for ox in -half..=half {
                if ox * ox + oy * oy <= half * half {
                    put_pixel(img, x + ox, y + oy, ink());
                }
            }
        }
    }
}

fn put_pixel(img: &mut RgbaImage, x: i32, y: i32, px: Rgba<u8>) {
    if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 {
        return;
    }
    img.put_pixel(x as u32, y as u32, px);
}
