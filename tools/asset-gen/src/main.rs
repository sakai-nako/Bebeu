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
    // 吹っ飛び flow 用 placeholder (ADR-0024/0025)。1 sprite per group の最小構成。
    // knockback_up/down と bounce_up/down は同じ airborne sprite を animation YAML 経由で
    // 共有する (= sprite-group は 6 個、animation は 8 個)。
    write_sprite(&base, "airborne_up", 1, 0, palette)?;
    write_sprite(&base, "airborne_down", 1, 0, palette)?;
    write_sprite(&base, "slide", 1, 0, palette)?;
    write_sprite(&base, "lie_down", 1, 0, palette)?;
    write_sprite(&base, "rise", 1, 0, palette)?;
    write_sprite(&base, "dead_lie_down", 1, 0, palette)?;
    // DownHit (Phase D2): 地上 hit pose。
    write_sprite(&base, "down_hit", 1, 0, palette)?;
    // DownAttack (Phase E2): 足元の AttackBox を持つ下段攻撃 pose。
    write_sprite(&base, "down_attack", 1, 0, palette)?;
    write_sprite(&base, "down_attack", 2, 1, palette)?;
    // Jump / JumpAttack (ADR-0027)。jump は 1 frame ループ (重力で自然落下中)、
    // jump_attack は 2 frame (構え / 突き) の Attack 系。
    write_sprite(&base, "jump", 1, 0, palette)?;
    write_sprite(&base, "jump_attack", 1, 0, palette)?;
    write_sprite(&base, "jump_attack", 2, 1, palette)?;
    // Guard / GuardBreak (ADR-0028)。guard は 1 frame ループ、guard_break は 1 frame の
    // 中継 (次フレームで KnockbackUp に切り替わる)。
    write_sprite(&base, "guard", 1, 0, palette)?;
    write_sprite(&base, "guard_break", 1, 0, palette)?;
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
        "airborne_up" => render_airborne_sprite(palette, false),
        "airborne_down" => render_airborne_sprite(palette, true),
        "slide" => render_slide_sprite(palette),
        "lie_down" => render_lie_down_sprite(palette, false),
        "rise" => render_rise_sprite(palette),
        "dead_lie_down" => render_lie_down_sprite(palette, true),
        "down_hit" => render_down_hit_sprite(palette),
        "down_attack" => render_down_attack_sprite(palette, phase),
        "jump" => render_jump_sprite(palette),
        "jump_attack" => render_jump_attack_sprite(palette, phase),
        "guard" => render_guard_sprite(palette),
        "guard_break" => render_guard_break_sprite(palette),
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

/// 吹っ飛び中 (空中) の sprite。`falling=false` = 上昇 (`KnockbackUp` / `BounceUp` 用)、
/// `falling=true` = 下降 (`KnockbackDown` / `BounceDown` 用)。
/// 上昇は腕を上に、下降は腕を下に流して空中感を出す。
fn render_airborne_sprite(palette: &CharacterPalette, falling: bool) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    // 体は通常より少し縮める (浮いている感)
    let body_y = 22;
    let head_y = if falling { 18 } else { 10 };
    fill_rect(&mut buf, 16, head_y, 16, 14, palette.head);
    fill_rect(&mut buf, 14, body_y, 20, 28, palette.body);
    fill_rect(&mut buf, 14, body_y + 22, 20, 4, palette.accent);
    // 脚は両側に広がる (空中フェーズ)
    fill_rect(&mut buf, 12, body_y + 28, 6, 18, palette.body);
    fill_rect(&mut buf, 30, body_y + 28, 6, 18, palette.body);
    // 腕の角度を上昇 / 下降で変える
    if falling {
        // 下降: 腕を背後・下方向に流す
        fill_rect(&mut buf, 4, body_y + 14, 10, 4, palette.body);
        fill_rect(&mut buf, 34, body_y + 14, 10, 4, palette.body);
    } else {
        // 上昇: 腕を上方向に上げる
        fill_rect(&mut buf, 8, body_y - 8, 6, 12, palette.body);
        fill_rect(&mut buf, 34, body_y - 8, 6, 12, palette.body);
    }
    buf
}

/// `Slide` 用 sprite (地面に体を横たえて滑っている)。pivot は `(24, 90)` 前提。
/// 体を完全に横向きにする (頭が画面右、足が左)。
fn render_slide_sprite(palette: &CharacterPalette) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    let y_top = 76; // 地面 (pivot=90) のすぐ上
    // 体: 横長
    fill_rect(&mut buf, 4, y_top + 2, 32, 10, palette.body);
    fill_rect(&mut buf, 4, y_top, 32, 2, palette.accent);
    // 頭: 右端
    fill_rect(&mut buf, 34, y_top, 12, 12, palette.head);
    // 腕 (流れて後方): 左側
    fill_rect(&mut buf, 0, y_top + 4, 6, 4, palette.body);
    buf
}

/// `LieDown` / `DeadLieDown` 用 sprite。`dead=false` で通常の倒れポーズ、
/// `dead=true` でアクセント色の X 印を頭部に乗せる (KO 演出)。
fn render_lie_down_sprite(palette: &CharacterPalette, dead: bool) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    let y_top = 78;
    // 体: 完全に横たわる
    fill_rect(&mut buf, 6, y_top + 2, 30, 8, palette.body);
    fill_rect(&mut buf, 6, y_top, 30, 2, palette.accent);
    // 頭: 右端
    fill_rect(&mut buf, 34, y_top, 12, 12, palette.head);
    if dead {
        // X 目印 (KO): アクセント色で 2 本のラインを頭の中央に交差させる
        fill_rect(&mut buf, 36, y_top + 3, 8, 2, palette.accent);
        fill_rect(&mut buf, 36, y_top + 7, 8, 2, palette.accent);
    }
    buf
}

/// 下段攻撃 sprite。phase 0 = しゃがみ構え、phase 1 = 前方下方に拳を突き出す。
/// `AttackBox` は YAML 側で `top_left:[32, 78], bottom_right:[48, 90]` の足元位置にセット
/// (= 倒れた敵の `lie_down` body box (world Y 0-14) と Y 範囲が overlap、立ち body box には
/// 当たらない設計)。
fn render_down_attack_sprite(palette: &CharacterPalette, phase: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    let extend: i32 = if phase == 0 { 0 } else { 10 };
    // しゃがみ姿勢: 通常よりちょっと低い
    fill_rect(&mut buf, 16, 22, 16, 14, palette.head);
    fill_rect(&mut buf, 14, 36, 20, 24, palette.body);
    fill_rect(&mut buf, 14, 54, 20, 4, palette.accent);
    // 屈んだ脚 (両方膝が前)
    fill_rect(&mut buf, 14, 60, 6, 28, palette.body);
    fill_rect(&mut buf, 28, 60, 6, 28, palette.body);
    // 引き手 (左腕)
    fill_rect(&mut buf, 10, 40, 4, 16, palette.body);
    // 突き手 (右腕): phase で前方下に伸びる
    fill_rect(&mut buf, 34, 72, 4 + extend, 6, palette.body);
    fill_rect(&mut buf, 38 + extend, 76, 4, 10, palette.accent);
    buf
}

/// `DownHit` 用 sprite。地面に伏せたまま hit を受けた瞬間のポーズ。`lie_down` ベースに
/// 頭の jerk と腕の broadening で「衝撃が来た感」を出す。
fn render_down_hit_sprite(palette: &CharacterPalette) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    let y_top = 78;
    // 体: 横たわる (lie_down と同じ)
    fill_rect(&mut buf, 6, y_top + 2, 30, 8, palette.body);
    fill_rect(&mut buf, 6, y_top, 30, 2, palette.accent);
    // 頭: 右端、ちょっと右に jerk (= 衝撃を受けた感)
    fill_rect(&mut buf, 37, y_top - 1, 11, 12, palette.head);
    // 腕: 衝撃で broaden (両側に少し広がる)
    fill_rect(&mut buf, 0, y_top + 4, 8, 4, palette.body);
    fill_rect(&mut buf, 4, y_top + 8, 4, 4, palette.body);
    buf
}

/// `Rise` 用 sprite。半身を起こした kneeling ポーズ。idle と `lie_down` の中間。
fn render_rise_sprite(palette: &CharacterPalette) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    // 頭: 半分立ち上がった位置 (idle より下、lie_down より上)
    fill_rect(&mut buf, 16, 42, 16, 14, palette.head);
    // 体: 縦寄りだが屈んでる
    fill_rect(&mut buf, 14, 56, 20, 24, palette.body);
    fill_rect(&mut buf, 14, 76, 20, 4, palette.accent);
    // 片膝立ち: 左脚は地面、右脚は曲げる
    fill_rect(&mut buf, 14, 80, 6, 8, palette.body); // 左脚 (地面に近い)
    fill_rect(&mut buf, 24, 78, 12, 6, palette.body); // 右脚 (横に出る)
    // 腕: 支え (左) と上向き (右、起き上がる勢い)
    fill_rect(&mut buf, 8, 60, 6, 16, palette.body);
    fill_rect(&mut buf, 32, 48, 6, 14, palette.body);
    buf
}

/// `Jump` 用 sprite (ADR-0027)。両膝を曲げて両腕を上に伸ばす空中姿勢。
/// pivot は idle と同じ (24, 90) 前提。Y 軸は重力で動くので 1 frame ループ。
fn render_jump_sprite(palette: &CharacterPalette) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    // 頭は通常位置よりやや上
    fill_rect(&mut buf, 16, 10, 16, 16, palette.head);
    // 胴体: 通常より少し縮める (脚を引き寄せた感)
    fill_rect(&mut buf, 14, 26, 20, 28, palette.body);
    fill_rect(&mut buf, 14, 50, 20, 4, palette.accent);
    // 膝を曲げた脚 (画面下に伸ばさず、胴体の真下に折り畳む)
    fill_rect(&mut buf, 14, 56, 8, 18, palette.body);
    fill_rect(&mut buf, 26, 56, 8, 18, palette.body);
    // 両腕を上方向に上げる (= 飛び上がりの勢い)
    fill_rect(&mut buf, 10, 4, 4, 22, palette.body);
    fill_rect(&mut buf, 34, 4, 4, 22, palette.body);
    buf
}

/// `JumpAttack` 用 sprite (ADR-0027)。phase 0 = 構え (空中で膝抱え)、
/// phase 1 = 前方に蹴りを出す。AttackBox は YAML 側で前方やや低位置に置く想定。
fn render_jump_attack_sprite(palette: &CharacterPalette, phase: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    let kick: i32 = if phase == 0 { 0 } else { 12 };
    // 頭
    fill_rect(&mut buf, 16, 12, 16, 14, palette.head);
    // 胴体 (少し前傾)
    fill_rect(&mut buf, 14, 26, 20, 26, palette.body);
    fill_rect(&mut buf, 14, 48, 20, 4, palette.accent);
    // 後ろ脚 (引き脚)
    fill_rect(&mut buf, 14, 52, 6, 22, palette.body);
    // 蹴り脚 (phase で前方に伸びる)
    fill_rect(&mut buf, 28, 50, 4 + kick, 6, palette.body);
    fill_rect(&mut buf, 32 + kick, 48, 4, 10, palette.accent);
    // 両腕でバランス (両側に広げる)
    fill_rect(&mut buf, 4, 30, 10, 4, palette.body);
    fill_rect(&mut buf, 34, 30, 10, 4, palette.body);
    buf
}

/// `Guard` 用 sprite (ADR-0028)。両腕を顔の前で交差させて防御するポーズ。
/// 立ち姿勢ベース、idle と同じ pivot。
fn render_guard_sprite(palette: &CharacterPalette) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    // 頭 (やや前傾でガードに合わせる)
    fill_rect(&mut buf, 16, 14, 16, 14, palette.head);
    // 胴体
    fill_rect(&mut buf, 14, 28, 20, 32, palette.body);
    fill_rect(&mut buf, 14, 54, 20, 4, palette.accent);
    // 脚 (両足きちんと地面)
    fill_rect(&mut buf, 16, 60, 6, 28, palette.body);
    fill_rect(&mut buf, 26, 60, 6, 28, palette.body);
    // 防御する両腕: 顔の前に水平に並べる (accent 色で「シールド」感)
    fill_rect(&mut buf, 8, 20, 14, 4, palette.body);
    fill_rect(&mut buf, 26, 20, 14, 4, palette.body);
    fill_rect(&mut buf, 10, 24, 28, 4, palette.accent);
    buf
}

/// `GuardBreak` 用 sprite (ADR-0028)。ガードが弾かれた瞬間: 腕が左右に弾けて
/// 上体が後ろにのけぞる。1 frame しか出ない (次 frame で `KnockbackUp` に切り替わる)。
fn render_guard_break_sprite(palette: &CharacterPalette) -> Vec<u8> {
    let mut buf = vec![0u8; (SPRITE_W * SPRITE_H * 4) as usize];
    // 頭 (のけぞって後方 = 画面左に流れる)
    fill_rect(&mut buf, 12, 12, 16, 16, palette.head);
    // 胴体 (やや後ろに傾く)
    fill_rect(&mut buf, 12, 28, 20, 30, palette.body);
    fill_rect(&mut buf, 12, 54, 20, 4, palette.accent);
    // 脚 (踏みとどまる)
    fill_rect(&mut buf, 14, 60, 6, 28, palette.body);
    fill_rect(&mut buf, 26, 60, 6, 28, palette.body);
    // 腕が弾け飛んだ感: 左腕は後方、右腕は前方に大きく振り回される
    fill_rect(&mut buf, 0, 28, 12, 4, palette.body);
    fill_rect(&mut buf, 32, 28, 16, 4, palette.body);
    fill_rect(&mut buf, 44, 32, 4, 6, palette.accent);
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
