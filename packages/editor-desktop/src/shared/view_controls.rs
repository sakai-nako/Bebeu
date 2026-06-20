use serde::{Deserialize, Serialize};

/// パン操作に割り当てるマウスボタン。
///
/// 後々の operation 設定 UI でユーザーが選べるよう enum で持つ。
/// マウスは Middle、トラックパッドは Left + 修飾キー、などのプリセットを将来的に
/// 切り替える前提。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanButton {
    Left,
    Middle,
    Right,
}

/// ホイール zoom が行き来する固定 zoom 倍率の昇順リスト。
///
/// 連続的な multiplicative zoom (例: ×1.1) だと sprite の rendered position が
/// device pixel に対して subpixel 位置に着地し、frame 切替や pivot 移動で「半 px ずれ」
/// として視認されるため、整数 + 0.5 倍率の固定リストに制限する。
///
/// 全レベルが整数または 0.5 単位なので、image-pixel × zoom の結果は常に整数 × 0.5。
/// pivot offset が integer image-pixel 単位である限り、rendered position は
/// device pixel 上できれいに揃う。
const ZOOM_LEVELS: &[f64] = &[
    0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0, 32.0,
];

/// Sprite 編集 Canvas など、2D ビューポートを持つ画面の入力割り当て。
///
/// マウス / トラックパッドの違いを吸収するための設定。現状は default 値のみ使用するが、
/// 将来的に preferences UI から差し替えられるよう Preferences の一フィールドとして保持する。
///
/// **注**: zoom 倍率自体はユーザー設定にせず固定リスト (`ZOOM_LEVELS`) で管理する。
/// 旧版の preferences.yml にあった `zoom_step` / `min_zoom` / `max_zoom` フィールドは
/// 廃止され、deserialize 時には serde の unknown-field 無視動作で透過的に捨てられる。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewControlBindings {
    /// パンドラッグに使うマウスボタン
    pub pan_button: PanButton,
    /// ホイールで拡大・縮小の方向を反転する（trackpad のナチュラルスクロール対応）。
    pub invert_wheel_zoom: bool,
}

impl Default for ViewControlBindings {
    fn default() -> Self {
        Self {
            pan_button: PanButton::Middle,
            invert_wheel_zoom: false,
        }
    }
}

impl ViewControlBindings {
    /// Dioxus の MouseButton を `pan_button` と比較する。
    #[must_use]
    pub fn is_pan_button(self, button: dioxus::html::input_data::MouseButton) -> bool {
        use dioxus::html::input_data::MouseButton;
        matches!(
            (self.pan_button, button),
            (PanButton::Left, MouseButton::Primary)
                | (PanButton::Middle, MouseButton::Auxiliary)
                | (PanButton::Right, MouseButton::Secondary)
        )
    }

    /// 与えられた wheel `delta_y` から、`current` の次に着地すべき zoom 値を返す。
    ///
    /// `ZOOM_LEVELS` 上を 1 つ上 / 下に移動した値を返し、既に端まで来ている場合や
    /// `delta_y == 0` の場合は `None`。`current` がリスト上の値からずれていても
    /// (旧 preferences.yml の連続値等) 最寄り index から階段する。
    #[must_use]
    pub fn next_wheel_zoom(self, current: f64, delta_y: f64) -> Option<f64> {
        if delta_y == 0.0 {
            return None;
        }
        // wheel up (delta_y < 0) で zoom in。invert_wheel_zoom が true なら反転。
        let zoom_in = if self.invert_wheel_zoom {
            delta_y > 0.0
        } else {
            delta_y < 0.0
        };
        let idx = nearest_zoom_index(current);
        let next_idx = if zoom_in {
            idx.saturating_add(1).min(ZOOM_LEVELS.len() - 1)
        } else {
            idx.saturating_sub(1)
        };
        let next = ZOOM_LEVELS[next_idx];
        if (next - current).abs() < f64::EPSILON {
            None
        } else {
            Some(next)
        }
    }

    /// 任意の zoom 値を `ZOOM_LEVELS` の min/max 範囲に clamp する。
    /// `ZOOM_LEVELS` は型に紐づく定数で `self` には依存しないが、
    /// 呼び出し側 (`bindings.clamp_zoom(...)`) の表記を維持するためメソッドにしてある。
    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn clamp_zoom(self, zoom: f64) -> f64 {
        zoom.clamp(ZOOM_LEVELS[0], ZOOM_LEVELS[ZOOM_LEVELS.len() - 1])
    }
}

/// `current` に最も近い `ZOOM_LEVELS` の index を返す。同距離タイなら下側を採用する。
/// `current` が NaN の場合は中央 (1.0 が入っている index) にフォールバック。
fn nearest_zoom_index(current: f64) -> usize {
    if !current.is_finite() {
        return ZOOM_LEVELS
            .iter()
            .position(|&v| (v - 1.0).abs() < f64::EPSILON)
            .unwrap_or(0);
    }
    let mut best_idx = 0;
    let mut best_dist = f64::INFINITY;
    for (i, &v) in ZOOM_LEVELS.iter().enumerate() {
        let d = (v - current).abs();
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }
    best_idx
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    // ZOOM_LEVELS の値はリテラル (0.25 / 0.5 / 整数) で f64 representation が正確、
    // clamp も入力そのまま返すケースをテストしているため、float_cmp は許容する。
    use super::*;

    fn bindings(invert: bool) -> ViewControlBindings {
        ViewControlBindings {
            pan_button: PanButton::Middle,
            invert_wheel_zoom: invert,
        }
    }

    #[test]
    fn next_wheel_zoom_steps_up_through_levels() {
        let b = bindings(false);
        assert_eq!(b.next_wheel_zoom(1.0, -1.0), Some(2.0));
        assert_eq!(b.next_wheel_zoom(2.0, -1.0), Some(3.0));
        assert_eq!(b.next_wheel_zoom(6.0, -1.0), Some(8.0));
    }

    #[test]
    fn next_wheel_zoom_steps_down_through_levels() {
        let b = bindings(false);
        assert_eq!(b.next_wheel_zoom(2.0, 1.0), Some(1.0));
        assert_eq!(b.next_wheel_zoom(1.0, 1.0), Some(0.5));
        assert_eq!(b.next_wheel_zoom(0.5, 1.0), Some(0.25));
    }

    #[test]
    fn next_wheel_zoom_returns_none_at_edges() {
        let b = bindings(false);
        // 上端で更に zoom in → None
        assert_eq!(b.next_wheel_zoom(32.0, -1.0), None);
        // 下端で更に zoom out → None
        assert_eq!(b.next_wheel_zoom(0.25, 1.0), None);
    }

    #[test]
    fn next_wheel_zoom_returns_none_for_zero_delta() {
        let b = bindings(false);
        assert_eq!(b.next_wheel_zoom(1.0, 0.0), None);
    }

    #[test]
    fn next_wheel_zoom_invert_swaps_direction() {
        let b = bindings(true);
        // invert=true なら delta_y > 0 で zoom in
        assert_eq!(b.next_wheel_zoom(1.0, 1.0), Some(2.0));
        assert_eq!(b.next_wheel_zoom(1.0, -1.0), Some(0.5));
    }

    #[test]
    fn next_wheel_zoom_snaps_legacy_continuous_value_to_nearest_level() {
        let b = bindings(false);
        // 旧版 preferences.yml で 1.21 (= 1.1^2) のような値が残っていても、
        // 最寄り (1.0) から 1 段上 (2.0) に進める
        assert_eq!(b.next_wheel_zoom(1.21, -1.0), Some(2.0));
        // 1.6 は 2.0 寄り → 1 段下は 1.0
        assert_eq!(b.next_wheel_zoom(1.6, 1.0), Some(1.0));
    }

    #[test]
    fn clamp_zoom_pins_to_min_and_max_levels() {
        let b = bindings(false);
        assert_eq!(b.clamp_zoom(0.01), ZOOM_LEVELS[0]);
        assert_eq!(b.clamp_zoom(1000.0), ZOOM_LEVELS[ZOOM_LEVELS.len() - 1]);
        assert_eq!(b.clamp_zoom(2.0), 2.0); // リスト内の値はそのまま
        // リストにない中間値は clamp 範囲内ならそのまま (round しない)
        assert_eq!(b.clamp_zoom(1.5), 1.5);
    }

    #[test]
    fn nearest_zoom_index_handles_exact_match() {
        assert_eq!(nearest_zoom_index(1.0), 2);
        assert_eq!(nearest_zoom_index(2.0), 3);
    }

    #[test]
    fn nearest_zoom_index_handles_between_levels() {
        // 1.4 → 1.0 (idx 2) のほうが 2.0 (idx 3) より近い
        assert_eq!(nearest_zoom_index(1.4), 2);
        // 1.6 → 2.0 (idx 3) のほうが 1.0 (idx 2) より近い
        assert_eq!(nearest_zoom_index(1.6), 3);
    }

    #[test]
    fn nearest_zoom_index_handles_nan() {
        let idx = nearest_zoom_index(f64::NAN);
        assert_eq!(ZOOM_LEVELS[idx], 1.0);
    }

    #[test]
    fn default_uses_middle_pan_button() {
        let b = ViewControlBindings::default();
        assert_eq!(b.pan_button, PanButton::Middle);
        assert!(!b.invert_wheel_zoom);
    }
}
