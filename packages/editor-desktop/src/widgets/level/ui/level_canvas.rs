use std::sync::Arc;

use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;

use crate::entities::level::{Area, Level, OpponentTrigger};
use crate::entities::preference::use_preferences;
use crate::entities::project::{ProjectRepository, use_projects_refresh};
use crate::shared::{
    UseHistory, ViewControlBindings, use_image_cache_buster, versioned_asset_url,
    workspace_asset_url,
};

/// Level Canvas 専用の zoom 段階。Sprite Canvas は subpixel 揃え制約で整数 + 0.5 単位だが、
/// Level の base 画像は pixel art とは限らない (1920x1080 等の大きい写実的画像が多い) ので、
/// 縮小側と 1.0 周辺をもう少し細かくする。
const LEVEL_ZOOM_LEVELS: &[f64] = &[
    0.1, 0.125, 0.25, 0.333, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0,
];

fn nearest_level_zoom_index(current: f64) -> usize {
    if !current.is_finite() {
        return LEVEL_ZOOM_LEVELS
            .iter()
            .position(|&v| (v - 1.0).abs() < f64::EPSILON)
            .unwrap_or(0);
    }
    let mut best_idx = 0;
    let mut best_dist = f64::INFINITY;
    for (i, &v) in LEVEL_ZOOM_LEVELS.iter().enumerate() {
        let d = (v - current).abs();
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }
    best_idx
}

/// ホイール 1 ノッチで次の zoom 値を返す。`ViewControlBindings::next_wheel_zoom` と同じ
/// ロジックだが `LEVEL_ZOOM_LEVELS` 上を階段する点だけ違う。invert は bindings から受ける。
fn next_level_wheel_zoom(current: f64, delta_y: f64, invert: bool) -> Option<f64> {
    if delta_y == 0.0 {
        return None;
    }
    let zoom_in = if invert { delta_y > 0.0 } else { delta_y < 0.0 };
    let idx = nearest_level_zoom_index(current);
    let next_idx = if zoom_in {
        idx.saturating_add(1).min(LEVEL_ZOOM_LEVELS.len() - 1)
    } else {
        idx.saturating_sub(1)
    };
    let next = LEVEL_ZOOM_LEVELS[next_idx];
    if (next - current).abs() < f64::EPSILON {
        None
    } else {
        Some(next)
    }
}

/// Area の 4 頂点ハンドル種別。near 系は上下辺 (Z=`near_z`) を共有し、far 系は同様に
/// `far_z` を共有することで、台形の上下 2 辺が常にスクリーン水平に保たれる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AreaHandle {
    NearLeft,
    NearRight,
    FarLeft,
    FarRight,
}

#[derive(Debug, Clone, Copy)]
enum DragKind {
    /// Area の頂点ハンドル。
    AreaHandle {
        handle: AreaHandle,
        start_area: Area,
    },
    /// Canvas 全体の pan (視点移動)。`start_pan` は mousedown 時点で確定し、
    /// mousemove で canvas-pixel delta をそのまま (zoom 補正なしで) 加算する。
    PanCanvas { start_pan: [f64; 2] },
    /// Camera 開始位置 (world X, Y) のドラッグ。
    CameraStartHandle { start_x: i32, start_y: i32 },
    /// Player Spawn (world X, Z) のドラッグ。Y は常に 0。
    PlayerSpawnHandle { start_x: i32, start_z: i32 },
    /// Opponent trigger の発火閾値 (Player X) のドラッグ。
    TriggerLine { index: usize, start_x: i32 },
    /// Opponent trigger の spawn 位置 (X, Z) のドラッグ。Y は数値入力のみ。
    TriggerSpawnPoint {
        index: usize,
        start_x: i32,
        start_z: i32,
    },
}

#[derive(Debug, Clone)]
struct DragState {
    kind: DragKind,
    start_mouse: [i32; 2],
}

/// ドラッグ開始時の Area スナップショットに world delta を適用して新しい Area を返す。
/// 累積誤差を避けるため、mousemove では常に start_area から再計算する。
#[allow(clippy::similar_names)]
fn apply_area_drag(start: Area, handle: AreaHandle, dx_world: i32, dz_world: i32) -> Area {
    let mut a = start;
    match handle {
        AreaHandle::NearLeft => {
            a.near_min_x = start.near_min_x + dx_world;
            a.near_z = start.near_z + dz_world;
        }
        AreaHandle::NearRight => {
            a.near_max_x = start.near_max_x + dx_world;
            a.near_z = start.near_z + dz_world;
        }
        AreaHandle::FarLeft => {
            a.far_min_x = start.far_min_x + dx_world;
            a.far_z = start.far_z + dz_world;
        }
        AreaHandle::FarRight => {
            a.far_max_x = start.far_max_x + dx_world;
            a.far_z = start.far_z + dz_world;
        }
    }
    a
}

/// `base_dimensions` がある場合、area の 4 頂点を画像内に clamp する。
/// X は [0, width]、Z (= 画像ピクセル Y) は [0, height] に収める。
#[allow(clippy::cast_possible_wrap)]
fn clamp_area_to_image(mut a: Area, dim: [u32; 2]) -> Area {
    let w = dim[0] as i32;
    let h = dim[1] as i32;
    a.near_min_x = a.near_min_x.clamp(0, w);
    a.near_max_x = a.near_max_x.clamp(0, w);
    a.far_min_x = a.far_min_x.clamp(0, w);
    a.far_max_x = a.far_max_x.clamp(0, w);
    a.near_z = a.near_z.clamp(0, h);
    a.far_z = a.far_z.clamp(0, h);
    a
}

#[allow(clippy::cast_possible_wrap)]
fn clamp_image_x(x: i32, dim: Option<[u32; 2]>) -> i32 {
    match dim {
        Some(d) => x.clamp(0, d[0] as i32),
        None => x,
    }
}

/// 画像ピクセル Y (= world Z や camera 視界上端) を [0, image_height] に clamp する。
#[allow(clippy::cast_possible_wrap)]
fn clamp_image_y(y: i32, dim: Option<[u32; 2]>) -> i32 {
    match dim {
        Some(d) => y.clamp(0, d[1] as i32),
        None => y,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn mouse_xy(evt: &MouseEvent) -> [i32; 2] {
    let c = evt.client_coordinates();
    [c.x.round() as i32, c.y.round() as i32]
}

fn is_primary(evt: &MouseEvent) -> bool {
    evt.trigger_button()
        .is_none_or(|b| b == MouseButton::Primary)
}

/// Level の base 画像を背景に、`area` (台形) と各種ハンドルを
/// SpriteCanvas と同じ zoom (ホイール) + pan (middle button) 機構で視覚編集する。
///
/// Pattern D: 親 (LevelEditor) から `draft` Signal と `history` を受け取り、
/// drag end / Area 追加削除 / trigger ハンドルの操作はすべて draft に書き込む。
/// disk への保存は親の Save ボタンが担う。
///
/// 座標系 (ADR-0026):
/// - world (X, Z) = base 画像ピクセル (x, y) そのもの: `image_x = world_x`、`image_y = world_z`
/// - world Y は高さ (上方向正)。画像座標は下方向正なので `screen_y = world_z - world_y`
/// - image-pixel → CSS-pixel: anchor の `transform: scale({zoom})` で変換 (子要素は image-pixel ベース)
/// - mouse delta (CSS-pixel) → image-pixel: `delta_css / zoom`
///
/// 台形制約: near 系ハンドルは `near_z` を共有して動くため、上下 2 辺は常にスクリーン水平。
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::too_many_lines
)]
#[component]
pub fn LevelCanvas(mut draft: Signal<Level>, mut history: UseHistory<Level>) -> Element {
    let preferences = use_preferences();
    let bindings: ViewControlBindings = preferences.read().view_controls;

    let mut dragging: Signal<Option<DragState>> = use_signal(|| None);
    // drag 中の「途中値」を保持する一時 Signal。mouseup で確定して draft に書き込む。
    let mut transient_area: Signal<Option<Area>> = use_signal(|| None);
    let mut transient_camera: Signal<Option<[i32; 2]>> = use_signal(|| None);
    let mut transient_player_spawn: Signal<Option<[i32; 2]>> = use_signal(|| None);
    let mut transient_trigger: Signal<Option<(usize, OpponentTrigger)>> = use_signal(|| None);

    let mut zoom = use_signal(|| 1.0_f64);
    let mut pan = use_signal(|| [0.0_f64, 0.0]);
    // プレビュー視界サイズの源。この Level を参照する Project とその解像度を集め、ドロップダウンで
    // 選んだ Project の解像度で camera 矩形 / trigger 縦線を描く (Level には保存しない)。
    let project_repo = use_context::<Arc<dyn ProjectRepository>>();
    let projects_refresh = use_projects_refresh();
    let mut referencing_projects: Signal<Vec<(String, u32, u32)>> = use_signal(Vec::new);
    let mut selected_project_idx = use_signal(|| 0_usize);
    // 編集中の Area の index。areas.len() の範囲外なら 0 として扱う。
    let mut selected_area_idx: Signal<usize> = use_signal(|| 0_usize);

    let cache_buster = use_image_cache_buster();
    let version = cache_buster.map_or(0, |s| s.read().0);

    // ---- draft から派生する表示用の値を集める ----
    // 必ず read() を最小限のスコープに留めて、後続の onmousedown 等のクロージャで再 borrow が
    // 起きないようにする。
    let snapshot = draft.read().clone();
    let level_name = snapshot.name.clone();
    let level_base = snapshot.base.clone();
    let base_dim = snapshot.base_dimensions;
    let areas_count = snapshot.areas.len();
    let triggers_count = snapshot.opponent_triggers.len();
    let selected_idx = (*selected_area_idx.read()).min(areas_count.saturating_sub(1));

    // この Level を参照する Project 一覧 (name, width, height) を集める。
    {
        let project_repo = project_repo.clone();
        let lvl_name = level_name.clone();
        use_effect(move || {
            let _ = projects_refresh.subscribe();
            let mut v = Vec::new();
            if let Ok(names) = project_repo.list() {
                for n in names {
                    if let Ok(p) = project_repo.get(&n)
                        && p.levels.iter().any(|l| l == &lvl_name)
                    {
                        v.push((p.name, p.resolution.width, p.resolution.height));
                    }
                }
            }
            referencing_projects.set(v);
        });
    }

    // 選択中 Project の解像度 (参照 Project が無ければ 640x360 デフォルト)。
    let projects = referencing_projects();
    let sel_proj_idx = selected_project_idx().min(projects.len().saturating_sub(1));
    let view_res = projects
        .get(sel_proj_idx)
        .map_or([640_u32, 360], |&(_, w, h)| [w, h]);
    let view_w = view_res[0] as f32;
    let view_h = view_res[1] as f32;

    let img_url = versioned_asset_url(
        workspace_asset_url(&format!("data/levels/{level_name}/{level_base}")),
        version,
    );

    let drag_kind = dragging().map(|d| d.kind);

    let display_camera = match drag_kind {
        Some(DragKind::CameraStartHandle { .. }) => {
            transient_camera().unwrap_or([snapshot.camera_start_x, snapshot.camera_start_y])
        }
        _ => [snapshot.camera_start_x, snapshot.camera_start_y],
    };
    let display_player_spawn = match drag_kind {
        Some(DragKind::PlayerSpawnHandle { .. }) => {
            transient_player_spawn().unwrap_or([snapshot.player_spawn_x, snapshot.player_spawn_z])
        }
        _ => [snapshot.player_spawn_x, snapshot.player_spawn_z],
    };

    // 表示中の Area を解決する関数。選択中で drag 中なら transient、それ以外は snapshot をそのまま。
    let resolve_area = |i: usize| -> Option<Area> {
        if i == selected_idx && matches!(drag_kind, Some(DragKind::AreaHandle { .. })) {
            transient_area()
        } else {
            snapshot.areas.get(i).copied()
        }
    };

    // 表示中の trigger を解決。drag 中の対象 index なら transient、それ以外は snapshot から。
    let resolve_trigger = |i: usize| -> Option<OpponentTrigger> {
        let is_drag_target = matches!(
            drag_kind,
            Some(DragKind::TriggerLine { index, .. } | DragKind::TriggerSpawnPoint { index, .. })
                if index == i
        );
        if is_drag_target {
            transient_trigger
                .read()
                .as_ref()
                .map(|(_, t)| t.clone())
                .or_else(|| snapshot.opponent_triggers.get(i).cloned())
        } else {
            snapshot.opponent_triggers.get(i).cloned()
        }
    };

    let polygon_points_for = |a: Area| -> String {
        format!(
            "{},{} {},{} {},{} {},{}",
            a.near_min_x,
            a.near_z,
            a.near_max_x,
            a.near_z,
            a.far_max_x,
            a.far_z,
            a.far_min_x,
            a.far_z,
        )
    };

    // 選択中の Area とその頂点 (ハンドル位置)
    let selected_area = resolve_area(selected_idx).unwrap_or_default();
    let sel_near_y = selected_area.near_z;
    let sel_far_y = selected_area.far_z;
    let handles = [
        (AreaHandle::NearLeft, (selected_area.near_min_x, sel_near_y)),
        (
            AreaHandle::NearRight,
            (selected_area.near_max_x, sel_near_y),
        ),
        (AreaHandle::FarLeft, (selected_area.far_min_x, sel_far_y)),
        (AreaHandle::FarRight, (selected_area.far_max_x, sel_far_y)),
    ];

    let zoom_value = zoom();
    let inv_zoom = 1.0_f64 / zoom_value;
    let inv_zoom_f32 = inv_zoom as f32;
    let pan_value = pan();
    let pan_x = pan_value[0];
    let pan_y = pan_value[1];

    let on_canvas_mousedown = move |evt: MouseEvent| {
        let Some(button) = evt.trigger_button() else {
            return;
        };
        if bindings.is_pan_button(button) {
            evt.stop_propagation();
            dragging.set(Some(DragState {
                kind: DragKind::PanCanvas {
                    start_pan: *pan.peek(),
                },
                start_mouse: mouse_xy(&evt),
            }));
        }
    };

    let on_mousemove = move |evt: MouseEvent| {
        let Some(drag) = dragging.peek().clone() else {
            return;
        };
        let mouse = mouse_xy(&evt);
        let dx_screen = f64::from(mouse[0] - drag.start_mouse[0]);
        let dy_screen = f64::from(mouse[1] - drag.start_mouse[1]);
        match drag.kind {
            DragKind::PanCanvas { start_pan } => {
                pan.set([start_pan[0] + dx_screen, start_pan[1] + dy_screen]);
            }
            DragKind::AreaHandle { handle, start_area } => {
                let z = zoom_value;
                let dx_world = (dx_screen / z).round() as i32;
                let dz_world = (dy_screen / z).round() as i32;
                let mut new_area = apply_area_drag(start_area, handle, dx_world, dz_world);
                if let Some(dim) = base_dim {
                    new_area = clamp_area_to_image(new_area, dim);
                }
                if *transient_area.peek() != Some(new_area) {
                    transient_area.set(Some(new_area));
                }
            }
            DragKind::CameraStartHandle { start_x, start_y } => {
                let z = zoom_value;
                let new_x = clamp_image_x(start_x + (dx_screen / z).round() as i32, base_dim);
                // camera_start_y は視界上端の画像 Y。下にドラッグで増える (画像下方向)。
                let new_y = clamp_image_y(start_y + (dy_screen / z).round() as i32, base_dim);
                let next = [new_x, new_y];
                if *transient_camera.peek() != Some(next) {
                    transient_camera.set(Some(next));
                }
            }
            DragKind::PlayerSpawnHandle { start_x, start_z } => {
                let z = zoom_value;
                let new_x = clamp_image_x(start_x + (dx_screen / z).round() as i32, base_dim);
                let new_z = clamp_image_y(start_z + (dy_screen / z).round() as i32, base_dim);
                let next = [new_x, new_z];
                if *transient_player_spawn.peek() != Some(next) {
                    transient_player_spawn.set(Some(next));
                }
            }
            DragKind::TriggerLine { index, start_x } => {
                let z = zoom_value;
                let new_x = clamp_image_x(start_x + (dx_screen / z).round() as i32, base_dim);
                let base_trigger = draft
                    .peek()
                    .opponent_triggers
                    .get(index)
                    .cloned()
                    .unwrap_or_default();
                let new_trigger = OpponentTrigger {
                    trigger_x: new_x,
                    ..base_trigger
                };
                let next = Some((index, new_trigger));
                if *transient_trigger.peek() != next {
                    transient_trigger.set(next);
                }
            }
            DragKind::TriggerSpawnPoint {
                index,
                start_x,
                start_z,
            } => {
                let z = zoom_value;
                let new_x = clamp_image_x(start_x + (dx_screen / z).round() as i32, base_dim);
                // marker は高さ (Y) を無視し image_y = spawn_z で表示するので、縦移動はそのまま spawn_z。
                let new_z = clamp_image_y(start_z + (dy_screen / z).round() as i32, base_dim);
                let base_trigger_unwrapped = draft
                    .peek()
                    .opponent_triggers
                    .get(index)
                    .cloned()
                    .unwrap_or_default();
                let new_trigger = OpponentTrigger {
                    spawn_x: new_x,
                    spawn_z: new_z,
                    ..base_trigger_unwrapped
                };
                let next = Some((index, new_trigger));
                if *transient_trigger.peek() != next {
                    transient_trigger.set(next);
                }
            }
        }
    };

    let on_mouseup = move |_evt: MouseEvent| {
        let Some(drag) = dragging.peek().clone() else {
            return;
        };
        match drag.kind {
            DragKind::AreaHandle { .. } => {
                if let Some(new_area) = transient_area.peek().as_ref().copied() {
                    let cur = draft.peek().clone();
                    let mut new_areas = cur.areas.clone();
                    if let Some(slot) = new_areas.get_mut(selected_idx)
                        && *slot != new_area
                    {
                        *slot = new_area;
                        history.record();
                        draft.set(Level {
                            areas: new_areas,
                            ..cur
                        });
                    }
                }
                transient_area.set(None);
            }
            DragKind::CameraStartHandle { .. } => {
                if let Some([cx, cy]) = transient_camera.peek().as_ref().copied() {
                    let cur = draft.peek().clone();
                    if cur.camera_start_x != cx || cur.camera_start_y != cy {
                        history.record();
                        draft.set(Level {
                            camera_start_x: cx,
                            camera_start_y: cy,
                            ..cur
                        });
                    }
                }
                transient_camera.set(None);
            }
            DragKind::PlayerSpawnHandle { .. } => {
                if let Some([px, pz]) = transient_player_spawn.peek().as_ref().copied() {
                    let cur = draft.peek().clone();
                    if cur.player_spawn_x != px || cur.player_spawn_z != pz {
                        history.record();
                        draft.set(Level {
                            player_spawn_x: px,
                            player_spawn_z: pz,
                            ..cur
                        });
                    }
                }
                transient_player_spawn.set(None);
            }
            DragKind::TriggerLine { .. } | DragKind::TriggerSpawnPoint { .. } => {
                if let Some((idx, t)) = transient_trigger.peek().clone() {
                    let cur = draft.peek().clone();
                    let mut new_triggers = cur.opponent_triggers.clone();
                    if let Some(slot) = new_triggers.get_mut(idx)
                        && *slot != t
                    {
                        *slot = t;
                        history.record();
                        draft.set(Level {
                            opponent_triggers: new_triggers,
                            ..cur
                        });
                    }
                }
                transient_trigger.set(None);
            }
            DragKind::PanCanvas { .. } => {
                // pan は永続化しない (UI 状態のみ)
            }
        }
        dragging.set(None);
    };

    let invert_wheel = bindings.invert_wheel_zoom;
    let on_wheel = move |evt: WheelEvent| {
        let current = *zoom.peek();
        if let Some(next) =
            next_level_wheel_zoom(current, evt.delta().strip_units().y, invert_wheel)
        {
            // canvas 中央を固定してズーム: 画面中心にある画像点が動かないよう pan を比率で調整する。
            // anchor は left:50%/top:50% 起点なので pan' = pan * (next / current) で中心が保たれる。
            let ratio = next / current;
            let cur_pan = *pan.peek();
            pan.set([cur_pan[0] * ratio, cur_pan[1] * ratio]);
            zoom.set(next);
        }
    };

    let original_selected_area = selected_area;
    let original_camera = [snapshot.camera_start_x, snapshot.camera_start_y];
    let original_player_spawn = [snapshot.player_spawn_x, snapshot.player_spawn_z];

    let on_add_area = move |evt: MouseEvent| {
        evt.stop_propagation();
        let cur = draft.peek().clone();
        let mut new_areas = cur.areas.clone();
        new_areas.push(Area::default());
        let new_idx = new_areas.len() - 1;
        history.record();
        draft.set(Level {
            areas: new_areas,
            ..cur
        });
        selected_area_idx.set(new_idx);
    };

    let on_delete_area = move |evt: MouseEvent| {
        evt.stop_propagation();
        let cur = draft.peek().clone();
        if cur.areas.len() <= 1 {
            return;
        }
        let idx = selected_idx;
        if idx >= cur.areas.len() {
            return;
        }
        let mut new_areas = cur.areas.clone();
        new_areas.remove(idx);
        history.record();
        draft.set(Level {
            areas: new_areas,
            ..cur
        });
        selected_area_idx.set(0);
    };

    let can_delete = areas_count > 1;

    // Camera / Player Spawn の screen 位置 (= 画像ピクセル位置そのもの)
    // SVG マーカー描画用に image-pixel (i32) を f32 にする。inv_zoom 等との演算を簡潔にするため。
    let camera_screen_x = display_camera[0] as f32;
    let camera_screen_y = display_camera[1] as f32;
    let player_screen_x = display_player_spawn[0] as f32;
    let player_screen_y = display_player_spawn[1] as f32;

    // base 画像は natural size で表示する。これを怠ると Tailwind preflight の `max-width: 100%`
    // が img をキャンバス幅に縮小し、image-pixel 座標で描く SVG マーカー (camera 矩形等) と
    // スケールがずれて矩形が画像からはみ出す (SpriteCanvas / sprite_reference と同じ対処)。
    // base_dimensions があれば width/height も明示してレイアウトシフトを防ぐ。
    let img_size_style = base_dim.map_or_else(
        || "max-width: none;".to_string(),
        |[w, h]| format!("max-width: none; width: {w}px; height: {h}px;"),
    );

    // trigger 用 snapshot を closure に渡せるよう clone
    let triggers_snapshot = snapshot.opponent_triggers.clone();

    rsx! {
        div {
            class: "relative w-full h-full overflow-hidden select-none bg-base-200 border border-base-300",
            onmousedown: on_canvas_mousedown,
            onmousemove: on_mousemove,
            onmouseup: on_mouseup,
            onmouseleave: on_mouseup,
            onwheel: on_wheel,

            div {
                class: "absolute",
                style: "left: round(50%, 1px); top: round(50%, 1px); transform: translate({pan_x}px, {pan_y}px) scale({zoom_value}); transform-origin: top left; will-change: transform;",

                div { class: "relative inline-block",
                    img {
                        class: "block",
                        src: "{img_url}",
                        alt: "Level base layer",
                        draggable: false,
                        style: "{img_size_style}",
                    }

                    svg {
                        class: "absolute inset-0 pointer-events-none",
                        style: "width: 100%; height: 100%; overflow: visible;",

                        // Area 台形 (全部描画。非選択は薄く、選択中は濃く)。中央に Area 番号。
                        for i in 0..areas_count {
                            if let Some(a) = resolve_area(i) {
                                {
                                    let pts = polygon_points_for(a);
                                    let is_selected = i == selected_idx;
                                    let cls = if is_selected {
                                        "fill-accent/30 stroke-accent"
                                    } else {
                                        "fill-accent/10 stroke-accent/40"
                                    };
                                    let sw = if is_selected { 2.0 } else { 1.0 } * inv_zoom;
                                    let cx = (a.near_min_x + a.near_max_x + a.far_min_x
                                        + a.far_max_x) as f32
                                        / 4.0;
                                    let cy = f32::midpoint(a.near_z as f32, a.far_z as f32);
                                    let fs = 18.0 * inv_zoom_f32;
                                    rsx! {
                                        polygon {
                                            key: "area-{i}",
                                            points: "{pts}",
                                            class: "{cls}",
                                            style: "stroke-width: {sw};",
                                        }
                                        text {
                                            x: "{cx}",
                                            y: "{cy}",
                                            class: "fill-accent font-bold pointer-events-none",
                                            style: "font-size: {fs}px; text-anchor: middle; dominant-baseline: central;",
                                            "{i + 1}"
                                        }
                                    }
                                }
                            }
                        }

                        // Opponent trigger の発火閾値 (画像高さ全体の破線縦線)。上部に番号。
                        for i in 0..triggers_count {
                            if let Some(t) = resolve_trigger(i) {
                                {
                                    let sw = 1.5 * inv_zoom;
                                    let dash = 6.0 * inv_zoom;
                                    let fs = 18.0 * inv_zoom_f32;
                                    // 縦線は camera 視界の縦範囲 (camera_start_y から解像度の高さ分) に揃え、
                                    // 番号はその中央に置く。
                                    let top_y = camera_screen_y;
                                    let bottom_y = camera_screen_y + view_h;
                                    let label_y = camera_screen_y + view_h / 2.0;
                                    rsx! {
                                        line {
                                            key: "trigger-line-{i}",
                                            x1: "{t.trigger_x}",
                                            y1: "{top_y}",
                                            x2: "{t.trigger_x}",
                                            y2: "{bottom_y}",
                                            class: "stroke-warning",
                                            style: "stroke-width: {sw}; stroke-dasharray: {dash};",
                                        }
                                        text {
                                            x: "{t.trigger_x}",
                                            y: "{label_y}",
                                            class: "fill-warning font-bold pointer-events-none",
                                            style: "font-size: {fs}px; text-anchor: middle; dominant-baseline: central;",
                                            "{i + 1}"
                                        }
                                    }
                                }
                            }
                        }

                        // カメラ視界: camera_start を左上とした解像度サイズの矩形 (画面に映る範囲)
                        {
                            let sw = 1.5 * inv_zoom;
                            let dash = 4.0 * inv_zoom;
                            rsx! {
                                rect {
                                    x: "{camera_screen_x}",
                                    y: "{camera_screen_y}",
                                    width: "{view_w}",
                                    height: "{view_h}",
                                    fill: "none",
                                    class: "stroke-info/70",
                                    style: "stroke-width: {sw}; stroke-dasharray: {dash};",
                                }
                            }
                        }

                        // Player Spawn 十字マーカー
                        {
                            let s = 10.0 * inv_zoom_f32;
                            let sw = 2.0 * inv_zoom;
                            rsx! {
                                line {
                                    x1: "{player_screen_x - s}",
                                    y1: "{player_screen_y}",
                                    x2: "{player_screen_x + s}",
                                    y2: "{player_screen_y}",
                                    class: "stroke-success",
                                    style: "stroke-width: {sw};",
                                }
                                line {
                                    x1: "{player_screen_x}",
                                    y1: "{player_screen_y - s}",
                                    x2: "{player_screen_x}",
                                    y2: "{player_screen_y + s}",
                                    class: "stroke-success",
                                    style: "stroke-width: {sw};",
                                }
                            }
                        }
                    }

                    // Opponent trigger の縦線ドラッグ帯
                    for i in 0..triggers_count {
                        if let Some(t) = resolve_trigger(i) {
                            {
                                let t_for_md = triggers_snapshot.get(i).cloned();
                                rsx! {
                                    div {
                                        key: "trigger-line-hit-{i}",
                                        class: "absolute cursor-ew-resize",
                                        style: "left: {(t.trigger_x as f32) - 5.0 * inv_zoom_f32}px; top: 0; width: {10.0 * inv_zoom_f32}px; bottom: 0; z-index: 6;",
                                        title: "Trigger #{i + 1} 発火 X",
                                        onmousedown: move |evt: MouseEvent| {
                                            if !is_primary(&evt) {
                                                return;
                                            }
                                            evt.stop_propagation();
                                            let Some(t0) = t_for_md.clone() else {
                                                return;
                                            };
                                            let start_x = t0.trigger_x;
                                            dragging
                                                .set(
                                                    Some(DragState {
                                                        kind: DragKind::TriggerLine {
                                                            index: i,
                                                            start_x,
                                                        },
                                                        start_mouse: mouse_xy(&evt),
                                                    }),
                                                );
                                            transient_trigger.set(Some((i, t0)));
                                        },
                                    }
                                }
                            }
                        }
                    }

                    // Opponent trigger spawn marker
                    for i in 0..triggers_count {
                        if let Some(t) = resolve_trigger(i) {
                            {
                                // marker は高さ (Y) を無視し X (spawn_x) と Z (spawn_z) だけで表示する。
                                let spawn_screen_y = t.spawn_z;
                                let t_for_md = triggers_snapshot.get(i).cloned();
                                rsx! {
                                    div {
                                        key: "trigger-spawn-hit-{i}",
                                        class: "absolute w-6 h-6 bg-warning border-2 border-warning-content cursor-move shadow flex items-center justify-center text-xs font-bold text-warning-content",
                                        style: "left: {t.spawn_x}px; top: {spawn_screen_y}px; transform: translate(-50%, -50%) scale({inv_zoom}); transform-origin: center; z-index: 8;",
                                        title: "Trigger #{i + 1} spawn (X+Z)",
                                        onmousedown: move |evt: MouseEvent| {
                                            if !is_primary(&evt) {
                                                return;
                                            }
                                            evt.stop_propagation();
                                            let Some(t0) = t_for_md.clone() else {
                                                return;
                                            };
                                            let start_x = t0.spawn_x;
                                            let start_z = t0.spawn_z;
                                            dragging
                                                .set(
                                                    Some(DragState {
                                                        kind: DragKind::TriggerSpawnPoint {
                                                            index: i,
                                                            start_x,
                                                            start_z,
                                                        },
                                                        start_mouse: mouse_xy(&evt),
                                                    }),
                                                );
                                            transient_trigger.set(Some((i, t0)));
                                        },
                                        "{i + 1}"
                                    }
                                }
                            }
                        }
                    }

                    // Camera 開始位置ハンドル
                    div {
                        class: "absolute w-6 h-6 rounded-full bg-info border-2 border-base-100 cursor-move shadow",
                        style: "left: {camera_screen_x}px; top: {camera_screen_y}px; transform: translate(-50%, -50%) scale({inv_zoom}); transform-origin: center; z-index: 9;",
                        title: "カメラ視界の左上 (X+Y)",
                        onmousedown: move |evt: MouseEvent| {
                            if !is_primary(&evt) {
                                return;
                            }
                            evt.stop_propagation();
                            dragging
                                .set(
                                    Some(DragState {
                                        kind: DragKind::CameraStartHandle {
                                            start_x: original_camera[0],
                                            start_y: original_camera[1],
                                        },
                                        start_mouse: mouse_xy(&evt),
                                    }),
                                );
                            transient_camera.set(Some(original_camera));
                        },
                    }

                    // Player Spawn ハンドル
                    div {
                        class: "absolute w-6 h-6 rounded-full bg-success border-2 border-base-100 cursor-move shadow",
                        style: "left: {player_screen_x}px; top: {player_screen_y}px; transform: translate(-50%, -50%) scale({inv_zoom}); transform-origin: center; z-index: 9;",
                        title: "Player Spawn (X+Z)",
                        onmousedown: move |evt: MouseEvent| {
                            if !is_primary(&evt) {
                                return;
                            }
                            evt.stop_propagation();
                            dragging
                                .set(
                                    Some(DragState {
                                        kind: DragKind::PlayerSpawnHandle {
                                            start_x: original_player_spawn[0],
                                            start_z: original_player_spawn[1],
                                        },
                                        start_mouse: mouse_xy(&evt),
                                    }),
                                );
                            transient_player_spawn.set(Some(original_player_spawn));
                        },
                    }

                    // Area 4 頂点ハンドル
                    for (handle, pos) in handles {
                        div {
                            key: "{handle:?}",
                            class: "absolute w-4 h-4 rounded-full bg-accent border-2 border-base-100 cursor-move shadow",
                            style: "left: {pos.0}px; top: {pos.1}px; transform: translate(-50%, -50%) scale({inv_zoom}); transform-origin: center; z-index: 10;",
                            onmousedown: move |evt: MouseEvent| {
                                if !is_primary(&evt) {
                                    return;
                                }
                                evt.stop_propagation();
                                dragging
                                    .set(
                                        Some(DragState {
                                            kind: DragKind::AreaHandle {
                                                handle,
                                                start_area: original_selected_area,
                                            },
                                            start_mouse: mouse_xy(&evt),
                                        }),
                                    );
                                transient_area.set(Some(original_selected_area));
                            },
                        }
                    }
                }
            }

            // Area タブ
            div {
                class: "absolute top-2 left-2 flex gap-1 items-center z-20",
                onmousedown: move |evt: MouseEvent| evt.stop_propagation(),
                for i in 0..areas_count {
                    {
                        let is_sel = i == selected_idx;
                        let cls = if is_sel {
                            "btn btn-xs btn-primary"
                        } else {
                            "btn btn-xs btn-outline"
                        };
                        rsx! {
                            button {
                                key: "tab-{i}",
                                r#type: "button",
                                class: "{cls}",
                                onclick: move |_| selected_area_idx.set(i),
                                "Area {i + 1}"
                            }
                        }
                    }
                }
                button {
                    r#type: "button",
                    class: "btn btn-xs btn-outline",
                    title: "Area を追加",
                    onclick: on_add_area,
                    "+ 追加"
                }
                if can_delete {
                    button {
                        r#type: "button",
                        class: "btn btn-xs btn-ghost text-error",
                        title: "選択中の Area を削除",
                        onclick: on_delete_area,
                        "✕ 削除"
                    }
                }
            }

            // プレビュー視界 (Project 解像度) の選択。camera 矩形 / trigger 縦線のサイズを決める。
            div {
                class: "absolute top-2 right-2 flex items-center gap-1 z-20 bg-base-100/80 rounded px-2 py-1 shadow",
                onmousedown: move |evt: MouseEvent| evt.stop_propagation(),
                span { class: "text-xs text-base-content/70", "視界" }
                if projects.is_empty() {
                    span { class: "text-xs text-base-content/50", "参照 Project なし (640×360)" }
                } else {
                    select {
                        class: "select select-bordered select-xs",
                        onchange: move |e| {
                            if let Ok(i) = e.value().parse::<usize>() {
                                selected_project_idx.set(i);
                            }
                        },
                        for (i, (name, w, h)) in projects.iter().enumerate() {
                            option {
                                key: "proj-{i}",
                                value: "{i}",
                                selected: i == sel_proj_idx,
                                "{name} ({w}×{h})"
                            }
                        }
                    }
                }
            }

            // 状態表示
            div { class: "absolute right-2 bottom-2 px-2 py-1 rounded bg-base-300/80 text-xs font-mono text-base-content/80 pointer-events-none space-y-0.5",
                div { "zoom × {zoom_value}" }
                div { "camera = ({display_camera[0]}, {display_camera[1]})" }
                div { "spawn = ({display_player_spawn[0]}, 0, {display_player_spawn[1]})" }
                div { "triggers = {triggers_count}" }
            }
        }
    }
}
