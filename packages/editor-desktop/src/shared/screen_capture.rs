//! virtual screen 全域のスクリーンショットを取り、overlay 用に合成する (Plan: atomic-questing-wozniak)。
//!
//! `xcap` で各 monitor の `RgbaImage` を取得し、virtual screen 座標系で 1 枚に合成する。
//! 合成結果は overlay window の背景画像 (data: URL) とピクセル取得用 buffer の両方に使う。
use std::fmt;
use std::io::Cursor;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::{ImageBuffer, ImageFormat, Rgba, RgbaImage};
use xcap::Monitor;

use crate::entities::project::HexColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorPlacement {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorLayout {
    /// virtual screen 左上 (physical px, 負値あり)。
    pub origin: (i32, i32),
    /// virtual screen 全体のサイズ (physical px)。
    pub size: (u32, u32),
    pub monitors: Vec<MonitorPlacement>,
}

/// virtual screen 全域の合成スクリーンショット + メタ情報。
/// `composite` と `png_data_url` は Arc で共有し、コンポーネント間の clone を軽量に保つ。
#[derive(Clone)]
pub struct CaptureResult {
    pub layout: MonitorLayout,
    pub composite: Arc<RgbaImage>,
    pub png_data_url: Arc<str>,
}

impl PartialEq for CaptureResult {
    fn eq(&self, other: &Self) -> bool {
        // 同じキャプチャ result を共有しているかは Arc identity で十分 (内容比較は重い)。
        Arc::ptr_eq(&self.composite, &other.composite) && self.layout == other.layout
    }
}

#[derive(Debug, Clone)]
pub enum CaptureError {
    NoMonitor,
    Xcap(String),
    Encode(String),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMonitor => write!(f, "モニタが見つかりません"),
            Self::Xcap(m) => write!(f, "画面取得に失敗: {m}"),
            Self::Encode(m) => write!(f, "画像エンコードに失敗: {m}"),
        }
    }
}

impl std::error::Error for CaptureError {}

#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
pub fn capture_virtual_screen() -> Result<CaptureResult, CaptureError> {
    let monitors = Monitor::all().map_err(|e| CaptureError::Xcap(e.to_string()))?;
    if monitors.is_empty() {
        return Err(CaptureError::NoMonitor);
    }

    let mut placements = Vec::with_capacity(monitors.len());
    let mut images = Vec::with_capacity(monitors.len());
    for mon in &monitors {
        let x = mon.x().map_err(|e| CaptureError::Xcap(e.to_string()))?;
        let y = mon.y().map_err(|e| CaptureError::Xcap(e.to_string()))?;
        let w = mon.width().map_err(|e| CaptureError::Xcap(e.to_string()))?;
        let h = mon
            .height()
            .map_err(|e| CaptureError::Xcap(e.to_string()))?;
        let img = mon
            .capture_image()
            .map_err(|e| CaptureError::Xcap(e.to_string()))?;
        placements.push(MonitorPlacement { x, y, w, h });
        images.push(img);
    }

    let min_x = placements.iter().map(|p| p.x).min().unwrap_or(0);
    let min_y = placements.iter().map(|p| p.y).min().unwrap_or(0);
    let max_x = placements
        .iter()
        .map(|p| p.x + p.w as i32)
        .max()
        .unwrap_or(0);
    let max_y = placements
        .iter()
        .map(|p| p.y + p.h as i32)
        .max()
        .unwrap_or(0);
    let total_w = (max_x - min_x).max(1) as u32;
    let total_h = (max_y - min_y).max(1) as u32;

    let mut composite: RgbaImage = ImageBuffer::from_pixel(total_w, total_h, Rgba([0, 0, 0, 255]));
    for (p, img) in placements.iter().zip(images.iter()) {
        let ox = (p.x - min_x) as u32;
        let oy = (p.y - min_y) as u32;
        for sy in 0..img.height().min(p.h) {
            for sx in 0..img.width().min(p.w) {
                let pixel = img.get_pixel(sx, sy);
                composite.put_pixel(ox + sx, oy + sy, *pixel);
            }
        }
    }

    let layout = MonitorLayout {
        origin: (min_x, min_y),
        size: (total_w, total_h),
        monitors: placements,
    };
    let png_data_url = encode_png_data_url(&composite)?;
    Ok(CaptureResult {
        layout,
        composite: Arc::new(composite),
        png_data_url,
    })
}

fn encode_png_data_url(img: &RgbaImage) -> Result<Arc<str>, CaptureError> {
    let mut buf = Vec::with_capacity(img.width() as usize * img.height() as usize / 4);
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| CaptureError::Encode(e.to_string()))?;
    let b64 = STANDARD.encode(&buf);
    Ok(Arc::from(format!("data:image/png;base64,{b64}")))
}

/// virtual screen 内の physical pixel 座標から色を取り出す。
/// 範囲外は `None`。
#[must_use]
#[allow(clippy::cast_sign_loss)]
pub fn pixel_at(c: &CaptureResult, screen_x: i32, screen_y: i32) -> Option<HexColor> {
    let lx = screen_x - c.layout.origin.0;
    let ly = screen_y - c.layout.origin.1;
    if lx < 0 || ly < 0 {
        return None;
    }
    let lx = lx as u32;
    let ly = ly as u32;
    if lx >= c.layout.size.0 || ly >= c.layout.size.1 {
        return None;
    }
    let p = c.composite.get_pixel(lx, ly);
    Some(HexColor {
        r: p[0],
        g: p[1],
        b: p[2],
        a: 255,
    })
}
