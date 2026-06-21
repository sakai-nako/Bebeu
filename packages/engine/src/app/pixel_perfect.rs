//! Pixel-perfect 拡大の中間 render texture pipeline (ADR-0026)。
//!
//! 各 scene の Camera は **viewport を整数倍した中間 texture** に描画し、Final Camera が
//! その texture を window サイズへ拡大して映す (linear filter)。
//! → 内部 (sprite テクスチャ → 中間 texture) は完全 nearest pixel-perfect、最終 window
//!    出力だけ滑らかに任意倍率を許容する形。
//!
//! 仕組み:
//! - [`PixelPerfectConfig`] (viewport / 中間 / window のピクセル size) を `entrypoint` が
//!   insert する
//! - [`PixelPerfectRenderPlugin`] が `Startup` で中間 [`Image`] を作り、Final Camera と
//!   Final Sprite を spawn する。中間 Image の handle は [`PixelPerfectTarget`] resource
//!   で持ち回る
//! - 各 scene Camera は `Camera.target = RenderTarget::Image(handle)` で中間 texture へ
//!   描画先を切り替える (`battle.rs` 参照)
//! - Final Camera / Sprite は [`FINAL_PASS_LAYER`] に置き、scene 側 (RenderLayers default
//!   = layer 0) と隔離する
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// 起動時 entrypoint が決定した「viewport / 中間 / window」の 3 段サイズ。
#[derive(Resource, Debug, Clone, Copy)]
pub struct PixelPerfectConfig {
    /// Project の論理解像度 (= scene camera が映す world 領域の px)。
    pub viewport: (u32, u32),
    /// 中間 render texture のサイズ (= `viewport × N` where N は整数)。
    pub intermediate: (u32, u32),
    /// Window 物理 pixel サイズ (= `bebeu-engine.yml` の `window` か fallback)。
    pub window: (u32, u32),
}

impl PixelPerfectConfig {
    /// Window と viewport から `N = floor(min(win_w/vp_w, win_h/vp_h))`(最低 1) を計算し、
    /// 中間 size を `viewport × N` で確定する。
    pub fn from_viewport_and_window(viewport: (u32, u32), window: (u32, u32)) -> Self {
        let (vw, vh) = viewport;
        let (ww, wh) = window;
        let scale = (ww / vw).min(wh / vh).max(1);
        Self {
            viewport,
            intermediate: (vw * scale, vh * scale),
            window,
        }
    }
}

/// 中間 render texture の handle。各 scene Camera が `target` に使う。
#[derive(Resource, Debug, Clone)]
pub struct PixelPerfectTarget {
    pub image: Handle<Image>,
}

/// Final pass (中間 texture を window に出す Camera + Sprite) が住む render layer。
/// scene 側 sprite (default = layer 0) と隔離するため固定値 1 を割り当てる。
pub const FINAL_PASS_LAYER: usize = 1;

#[derive(Component)]
pub struct FinalPassCamera;

#[derive(Component)]
pub struct FinalPassSprite;

pub struct PixelPerfectRenderPlugin;

impl Plugin for PixelPerfectRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_pixel_perfect_render);
    }
}

fn setup_pixel_perfect_render(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    config: Option<Res<PixelPerfectConfig>>,
) {
    let Some(config) = config else {
        // smoke test 等 PixelPerfectConfig を注入しないパスでは何もしない。
        tracing::info!("pixel_perfect: skipping setup (no PixelPerfectConfig)");
        return;
    };

    let (iw, ih) = config.intermediate;
    let mut image = Image::new_target_texture(iw, ih, TextureFormat::Bgra8UnormSrgb, None);
    // window 上では非整数倍で拡大されるため linear で補間して rippling を避ける。
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    let handle = images.add(image);

    let (ww, wh) = config.window;
    let scale = (ww as f32 / iw as f32).min(wh as f32 / ih as f32);

    tracing::info!(
        viewport = ?config.viewport,
        intermediate = ?config.intermediate,
        window = ?config.window,
        scale = scale,
        "pixel_perfect: setup",
    );

    commands.insert_resource(PixelPerfectTarget {
        image: handle.clone(),
    });

    // Final Camera: default RenderTarget (= primary window)、order > scene camera (=0)、
    // layer 1 のみ映す。Camera2d の require が RenderTarget を default で挿入してくれる。
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
        RenderLayers::layer(FINAL_PASS_LAYER),
        FinalPassCamera,
    ));

    // Final Sprite: 中間 texture を中心配置で scale 倍に拡大。
    // aspect が viewport と window で違うと min-scale により上下 / 左右に黒帯が出る。
    commands.spawn((
        Sprite::from_image(handle),
        Transform::from_scale(Vec3::splat(scale)),
        RenderLayers::layer(FINAL_PASS_LAYER),
        FinalPassSprite,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_scale_picks_largest_n_that_fits_in_window() {
        let cfg = PixelPerfectConfig::from_viewport_and_window((384, 216), (1280, 720));
        assert_eq!(cfg.intermediate, (1152, 648)); // N=3
    }

    #[test]
    fn integer_scale_is_at_least_1_even_for_tiny_window() {
        let cfg = PixelPerfectConfig::from_viewport_and_window((384, 216), (100, 100));
        assert_eq!(cfg.intermediate, (384, 216)); // N=1 floor
    }

    #[test]
    fn integer_scale_uses_minimum_of_axes() {
        // 横は ×4 まで入るが縦は ×2 までしか入らないケース
        let cfg = PixelPerfectConfig::from_viewport_and_window((100, 100), (500, 250));
        assert_eq!(cfg.intermediate, (200, 200));
    }
}
