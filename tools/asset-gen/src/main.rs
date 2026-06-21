//! sample-projects 用プレースホルダー画像ジェネレータ。
//!
//! 単色矩形だけで構成した CC0 相当のスプライト/背景を、engine が期待する
//! `data/characters/<name>/sprite-groups/<group>/sprites/NNN.png` レイアウトに書き出す。
//! 第 1 引数で出力ルートを指定 (未指定なら `sample-projects/minimal`)。
//!
//! `cargo run -p asset-gen -- sample-projects/minimal`

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use png::{BitDepth, ColorType, Encoder};

const SPRITE_W: u32 = 48;
const SPRITE_H: u32 = 96;
const LEVEL_W: u32 = 640;
const LEVEL_H: u32 = 216;

type Color = [u8; 4];

fn main() -> Result<()> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("sample-projects/minimal"), PathBuf::from);
    println!("asset-gen: writing into {}", out.display());

    let hero = CharacterPalette {
        head: [0x46, 0x9B, 0xF5, 0xFF],
        body: [0x20, 0x57, 0xC5, 0xFF],
        accent: [0xFF, 0xCC, 0x55, 0xFF],
    };
    let enemy = CharacterPalette {
        head: [0xF5, 0x6B, 0x6B, 0xFF],
        body: [0xC5, 0x2D, 0x2D, 0xFF],
        accent: [0x33, 0x33, 0x33, 0xFF],
    };

    write_character(&out, "hero", &hero)?;
    write_character(&out, "enemy", &enemy)?;
    write_level(&out, "training")?;

    println!("asset-gen: done");
    Ok(())
}

struct CharacterPalette {
    head: Color,
    body: Color,
    accent: Color,
}

fn write_character(root: &Path, name: &str, palette: &CharacterPalette) -> Result<()> {
    let base = root.join("data").join("characters").join(name);
    write_sprite(&base, "idle", 1, 0, palette)?;
    write_sprite(&base, "walk", 1, 0, palette)?;
    write_sprite(&base, "walk", 2, 1, palette)?;
    write_sprite(&base, "walk", 3, 2, palette)?;
    write_sprite(&base, "attack", 1, 0, palette)?;
    write_sprite(&base, "attack", 2, 1, palette)?;
    write_sprite(&base, "hit", 1, 0, palette)?;
    write_sprite(&base, "thumbnail", 1, 0, palette)?;
    Ok(())
}

fn write_sprite(
    character_dir: &Path,
    group: &str,
    index: u32,
    phase: u32,
    palette: &CharacterPalette,
) -> Result<()> {
    let dir = character_dir
        .join("sprite-groups")
        .join(group)
        .join("sprites");
    let path = dir.join(format!("{index:03}.png"));
    let pixels = match group {
        "attack" => render_attack_sprite(palette, phase),
        "hit" => render_hit_sprite(palette),
        _ => render_character_sprite(palette, phase),
    };
    write_png(&path, SPRITE_W, SPRITE_H, &pixels)
        .with_context(|| format!("write sprite: {}", path.display()))
}

/// `SPRITE_W` x `SPRITE_H` のキャラスプライトを描く。
/// phase は walk アニメで胴体・腕・脚の位相 (0/1/2)。pivot は `(24, 90)` 前提。
fn render_character_sprite(palette: &CharacterPalette, phase: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    let bob: i32 = match phase % 3 {
        0 => 0,
        1 => -2,
        _ => -1,
    };
    let leg: i32 = match phase % 3 {
        1 => -2,
        2 => 2,
        _ => 0,
    };

    fill_rect(&mut buf, 16, 12 + bob, 16, 16, palette.head);
    fill_rect(&mut buf, 14, 28 + bob, 20, 32, palette.body);
    fill_rect(&mut buf, 14, 54 + bob, 20, 4, palette.accent);
    fill_rect(&mut buf, 16 - leg, 60, 6, 28, palette.body);
    fill_rect(&mut buf, 26 + leg, 60, 6, 28, palette.body);
    fill_rect(&mut buf, 10 - leg, 32 + bob, 4, 24, palette.body);
    fill_rect(&mut buf, 34 + leg, 32 + bob, 4, 24, palette.body);

    buf
}

/// 攻撃 sprite。phase 0 = 構え、phase 1 = 前方に拳を突き出す。
/// 右向き前提で描き、左向きは `Sprite.flip_x` で対応する。
fn render_attack_sprite(palette: &CharacterPalette, phase: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    let extend: i32 = if phase == 0 { 0 } else { 10 };

    fill_rect(&mut buf, 16, 12, 16, 16, palette.head);
    fill_rect(&mut buf, 14, 28, 20, 32, palette.body);
    fill_rect(&mut buf, 14, 54, 20, 4, palette.accent);
    fill_rect(&mut buf, 14, 60, 6, 28, palette.body);
    fill_rect(&mut buf, 28, 60, 6, 28, palette.body);
    // 引き手 (左腕)
    fill_rect(&mut buf, 10, 32, 4, 20, palette.body);
    // 突き手 (右腕): phase で前方に伸びる
    fill_rect(&mut buf, 34, 34, 4 + extend, 6, palette.body);
    fill_rect(&mut buf, 38 + extend, 32, 4, 10, palette.accent);

    buf
}

/// 被弾 sprite。のけぞって体が沈み、頭が後ろ (右向き前提なので画面左) へ流れる。
/// 左向きは `Sprite.flip_x` 経由でそのまま反転される。
fn render_hit_sprite(palette: &CharacterPalette) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    let body_dy: i32 = 3; // 衝撃で少し沈み込む
    let head_dx: i32 = -3; // 頭が後ろに振れる

    fill_rect(&mut buf, 16 + head_dx, 14, 16, 16, palette.head);
    fill_rect(&mut buf, 14, 28 + body_dy, 20, 30, palette.body);
    fill_rect(&mut buf, 14, 54 + body_dy, 20, 4, palette.accent);
    fill_rect(&mut buf, 14, 60, 6, 28, palette.body);
    fill_rect(&mut buf, 28, 60, 6, 28, palette.body);
    // 両腕、衝撃で広がる
    fill_rect(&mut buf, 4, 32 + body_dy, 6, 16, palette.body);
    fill_rect(&mut buf, 38, 32 + body_dy, 6, 16, palette.body);

    buf
}

fn write_level(root: &Path, name: &str) -> Result<()> {
    let dir = root.join("data").join("levels").join(name);
    let pixels = render_level_base();
    let path = dir.join("base.png");
    write_png(&path, LEVEL_W, LEVEL_H, &pixels)
        .with_context(|| format!("write level base: {}", path.display()))
}

/// 上 60% を空グラデーション、残りをチェック柄の地面。
fn render_level_base() -> Vec<u8> {
    let mut buf = vec![0u8; (LEVEL_W * LEVEL_H * 4) as usize];
    let horizon: u32 = LEVEL_H * 60 / 100;
    for y in 0..LEVEL_H {
        for x in 0..LEVEL_W {
            let color = if y < horizon {
                let t = y as f32 / horizon as f32;
                [
                    lerp(0x6A, 0xB7, t),
                    lerp(0xC0, 0xE0, t),
                    lerp(0xE0, 0xF5, t),
                    0xFF,
                ]
            } else if ((x / 16) + (y / 16)) % 2 == 0 {
                [0x4A, 0x6D, 0x3A, 0xFF]
            } else {
                [0x3E, 0x5C, 0x30, 0xFF]
            };
            put_pixel(&mut buf, LEVEL_W, x, y, color);
        }
    }
    buf
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let v = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
    v.clamp(0.0, 255.0) as u8
}

fn fill_rect(buf: &mut [u8], x: i32, y: i32, w: i32, h: i32, color: Color) {
    let sprite_w = SPRITE_W.cast_signed();
    let sprite_h = SPRITE_H.cast_signed();
    for dy in 0..h {
        let py = y + dy;
        if py < 0 || py >= sprite_h {
            continue;
        }
        for dx in 0..w {
            let px = x + dx;
            if px < 0 || px >= sprite_w {
                continue;
            }
            put_pixel(buf, SPRITE_W, px.cast_unsigned(), py.cast_unsigned(), color);
        }
    }
}

fn put_pixel(buf: &mut [u8], width: u32, x: u32, y: u32, color: Color) {
    let i = ((y * width + x) * 4) as usize;
    buf[i..i + 4].copy_from_slice(&color);
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create_dir_all: {}", parent.display()))?;
    }
    let file = fs::File::create(path)?;
    let w = std::io::BufWriter::new(file);
    let mut encoder = Encoder::new(w, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}
