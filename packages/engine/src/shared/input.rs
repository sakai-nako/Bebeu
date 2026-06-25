//! Gameplay 入力の Action 抽象 (移動 / 攻撃 / Jump / Guard 等)。
//!
//! 各 gameplay system は `Res<ButtonInput<KeyCode>>` を直接見るのではなく、
//! `Res<ActionMap>` 経由で `action_map.pressed(&keys, Action::Jump)` のように引く。
//! バインドは [`ActionMap::load`] で `packages/engine/config/input.yml` (ADR-0016)
//! から fail-soft に読み込み、未指定の action は内部の default が残る。
//!
//! ## スコープ
//!
//! - gameplay 入力 (movement / attack / jump / guard) のみ
//! - debug キー (F1-F4) と scene transition (Title の Enter/Space) は KeyCode 直書きのまま
//!   - debug は keybinding 変更ニーズが薄い (開発専用)
//!   - menu 系入力 (Confirm/Cancel/Up/Down) は title 画面実装フェーズで別 enum を導入する
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::Deserialize;

/// gameplay 用 input action。1 action に複数の KeyCode を bind できる
/// ([`ActionMap`])。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Attack,
    DownAttack,
    Jump,
    Guard,
}

impl Action {
    /// 全 variant の列挙。デフォルト bind 構築・テスト用。
    /// 順序は `enum` の宣言順に揃える。
    pub const ALL: [Action; 8] = [
        Action::MoveLeft,
        Action::MoveRight,
        Action::MoveUp,
        Action::MoveDown,
        Action::Attack,
        Action::DownAttack,
        Action::Jump,
        Action::Guard,
    ];

    /// `input.yml` の bindings キー (snake_case) として使う安定文字列。
    #[must_use]
    pub fn as_snake_case(self) -> &'static str {
        match self {
            Action::MoveLeft => "move_left",
            Action::MoveRight => "move_right",
            Action::MoveUp => "move_up",
            Action::MoveDown => "move_down",
            Action::Attack => "attack",
            Action::DownAttack => "down_attack",
            Action::Jump => "jump",
            Action::Guard => "guard",
        }
    }

    /// snake_case 文字列から逆引き。未知文字列は `None`。
    #[must_use]
    pub fn from_snake_case(s: &str) -> Option<Action> {
        // ALL を素直に走査 (Action は 8 variants で逐次比較で十分速い)。
        Self::ALL.into_iter().find(|a| a.as_snake_case() == s)
    }
}

/// メニュー操作の入力 action (Title / Options scene 用)。
///
/// gameplay 用 [`Action`] と分けてあるのは、キーコンフィグの対象範囲を分離するため
/// (gameplay は yml で差し替え可、menu はキーコンフィグ画面の対象外)。
/// ただし helper 関数 ([`menu_action_pressed`] / [`menu_action_just_pressed`]) は
/// menu の固定キー (Enter/Space/Esc + 矢印/WASD) に加えて gameplay [`Action`] の
/// バインドも OR で参照することで、Attack キーを「決定」/ Jump キーを「キャンセル」/
/// MoveUp/MoveDown キーを「上下」として menu でも使えるようにしてある。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuAction {
    Up,
    Down,
    Confirm,
    Cancel,
}

/// menu action にバインドされた固定 KeyCode 一覧 (gameplay 連動分は除く)。
const fn menu_action_keys(action: MenuAction) -> &'static [KeyCode] {
    match action {
        MenuAction::Up => &[KeyCode::ArrowUp, KeyCode::KeyW],
        MenuAction::Down => &[KeyCode::ArrowDown, KeyCode::KeyS],
        MenuAction::Confirm => &[KeyCode::Enter, KeyCode::Space],
        MenuAction::Cancel => &[KeyCode::Escape],
    }
}

/// menu action に対応する gameplay [`Action`] を返す (連動マッピング)。
/// gameplay 側 [`Action`] のバインドがそのまま menu 入力としても効くようにする。
const fn menu_action_gameplay(action: MenuAction) -> Action {
    match action {
        MenuAction::Up => Action::MoveUp,
        MenuAction::Down => Action::MoveDown,
        MenuAction::Confirm => Action::Attack,
        MenuAction::Cancel => Action::Jump,
    }
}

/// menu action のいずれかの KeyCode が押下中なら true。
/// 固定キー OR 連動 gameplay action のバインドの両方を見る。
pub fn menu_action_pressed(
    keys: &ButtonInput<KeyCode>,
    action_map: &ActionMap,
    action: MenuAction,
) -> bool {
    menu_action_keys(action).iter().any(|&k| keys.pressed(k))
        || action_map.pressed(keys, menu_action_gameplay(action))
}

/// menu action のいずれかの KeyCode がこの frame で just_pressed なら true。
pub fn menu_action_just_pressed(
    keys: &ButtonInput<KeyCode>,
    action_map: &ActionMap,
    action: MenuAction,
) -> bool {
    menu_action_keys(action)
        .iter()
        .any(|&k| keys.just_pressed(k))
        || action_map.just_pressed(keys, menu_action_gameplay(action))
}

/// `input.yml` でサポートする KeyCode の文字列名 → `KeyCode` 逆引き。
///
/// 名前は Bevy `KeyCode` の variant 名 (`Debug` で出る文字列) に揃える。サポート範囲:
/// - 英字: `KeyA` - `KeyZ`
/// - 数字: `Digit0` - `Digit9`
/// - 矢印: `ArrowLeft` / `ArrowRight` / `ArrowUp` / `ArrowDown`
/// - F1 - F12
/// - 修飾: `ShiftLeft` / `ShiftRight` / `ControlLeft` / `ControlRight` / `AltLeft` / `AltRight`
/// - 特殊: `Space` / `Enter` / `Escape` / `Tab` / `Backspace` / `Delete`
///
/// gameplay + menu 操作に必要な範囲だけサポート。yml に未知文字列が来たら呼び出し側で
/// warn して skip するためここでは `Option` 返し。
#[must_use]
pub fn key_code_from_str(s: &str) -> Option<KeyCode> {
    match s {
        "KeyA" => Some(KeyCode::KeyA),
        "KeyB" => Some(KeyCode::KeyB),
        "KeyC" => Some(KeyCode::KeyC),
        "KeyD" => Some(KeyCode::KeyD),
        "KeyE" => Some(KeyCode::KeyE),
        "KeyF" => Some(KeyCode::KeyF),
        "KeyG" => Some(KeyCode::KeyG),
        "KeyH" => Some(KeyCode::KeyH),
        "KeyI" => Some(KeyCode::KeyI),
        "KeyJ" => Some(KeyCode::KeyJ),
        "KeyK" => Some(KeyCode::KeyK),
        "KeyL" => Some(KeyCode::KeyL),
        "KeyM" => Some(KeyCode::KeyM),
        "KeyN" => Some(KeyCode::KeyN),
        "KeyO" => Some(KeyCode::KeyO),
        "KeyP" => Some(KeyCode::KeyP),
        "KeyQ" => Some(KeyCode::KeyQ),
        "KeyR" => Some(KeyCode::KeyR),
        "KeyS" => Some(KeyCode::KeyS),
        "KeyT" => Some(KeyCode::KeyT),
        "KeyU" => Some(KeyCode::KeyU),
        "KeyV" => Some(KeyCode::KeyV),
        "KeyW" => Some(KeyCode::KeyW),
        "KeyX" => Some(KeyCode::KeyX),
        "KeyY" => Some(KeyCode::KeyY),
        "KeyZ" => Some(KeyCode::KeyZ),
        "Digit0" => Some(KeyCode::Digit0),
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        "Digit4" => Some(KeyCode::Digit4),
        "Digit5" => Some(KeyCode::Digit5),
        "Digit6" => Some(KeyCode::Digit6),
        "Digit7" => Some(KeyCode::Digit7),
        "Digit8" => Some(KeyCode::Digit8),
        "Digit9" => Some(KeyCode::Digit9),
        "ArrowLeft" => Some(KeyCode::ArrowLeft),
        "ArrowRight" => Some(KeyCode::ArrowRight),
        "ArrowUp" => Some(KeyCode::ArrowUp),
        "ArrowDown" => Some(KeyCode::ArrowDown),
        "F1" => Some(KeyCode::F1),
        "F2" => Some(KeyCode::F2),
        "F3" => Some(KeyCode::F3),
        "F4" => Some(KeyCode::F4),
        "F5" => Some(KeyCode::F5),
        "F6" => Some(KeyCode::F6),
        "F7" => Some(KeyCode::F7),
        "F8" => Some(KeyCode::F8),
        "F9" => Some(KeyCode::F9),
        "F10" => Some(KeyCode::F10),
        "F11" => Some(KeyCode::F11),
        "F12" => Some(KeyCode::F12),
        "ShiftLeft" => Some(KeyCode::ShiftLeft),
        "ShiftRight" => Some(KeyCode::ShiftRight),
        "ControlLeft" => Some(KeyCode::ControlLeft),
        "ControlRight" => Some(KeyCode::ControlRight),
        "AltLeft" => Some(KeyCode::AltLeft),
        "AltRight" => Some(KeyCode::AltRight),
        "Space" => Some(KeyCode::Space),
        "Enter" => Some(KeyCode::Enter),
        "Escape" => Some(KeyCode::Escape),
        "Tab" => Some(KeyCode::Tab),
        "Backspace" => Some(KeyCode::Backspace),
        "Delete" => Some(KeyCode::Delete),
        _ => None,
    }
}

/// `KeyCode` → サポート文字列名の逆方向 (キーコンフィグ画面の表示 / save 用)。
/// [`key_code_from_str`] でサポートしていない variant は `None`。
#[must_use]
pub fn key_code_to_str(code: KeyCode) -> Option<&'static str> {
    match code {
        KeyCode::KeyA => Some("KeyA"),
        KeyCode::KeyB => Some("KeyB"),
        KeyCode::KeyC => Some("KeyC"),
        KeyCode::KeyD => Some("KeyD"),
        KeyCode::KeyE => Some("KeyE"),
        KeyCode::KeyF => Some("KeyF"),
        KeyCode::KeyG => Some("KeyG"),
        KeyCode::KeyH => Some("KeyH"),
        KeyCode::KeyI => Some("KeyI"),
        KeyCode::KeyJ => Some("KeyJ"),
        KeyCode::KeyK => Some("KeyK"),
        KeyCode::KeyL => Some("KeyL"),
        KeyCode::KeyM => Some("KeyM"),
        KeyCode::KeyN => Some("KeyN"),
        KeyCode::KeyO => Some("KeyO"),
        KeyCode::KeyP => Some("KeyP"),
        KeyCode::KeyQ => Some("KeyQ"),
        KeyCode::KeyR => Some("KeyR"),
        KeyCode::KeyS => Some("KeyS"),
        KeyCode::KeyT => Some("KeyT"),
        KeyCode::KeyU => Some("KeyU"),
        KeyCode::KeyV => Some("KeyV"),
        KeyCode::KeyW => Some("KeyW"),
        KeyCode::KeyX => Some("KeyX"),
        KeyCode::KeyY => Some("KeyY"),
        KeyCode::KeyZ => Some("KeyZ"),
        KeyCode::Digit0 => Some("Digit0"),
        KeyCode::Digit1 => Some("Digit1"),
        KeyCode::Digit2 => Some("Digit2"),
        KeyCode::Digit3 => Some("Digit3"),
        KeyCode::Digit4 => Some("Digit4"),
        KeyCode::Digit5 => Some("Digit5"),
        KeyCode::Digit6 => Some("Digit6"),
        KeyCode::Digit7 => Some("Digit7"),
        KeyCode::Digit8 => Some("Digit8"),
        KeyCode::Digit9 => Some("Digit9"),
        KeyCode::ArrowLeft => Some("ArrowLeft"),
        KeyCode::ArrowRight => Some("ArrowRight"),
        KeyCode::ArrowUp => Some("ArrowUp"),
        KeyCode::ArrowDown => Some("ArrowDown"),
        KeyCode::F1 => Some("F1"),
        KeyCode::F2 => Some("F2"),
        KeyCode::F3 => Some("F3"),
        KeyCode::F4 => Some("F4"),
        KeyCode::F5 => Some("F5"),
        KeyCode::F6 => Some("F6"),
        KeyCode::F7 => Some("F7"),
        KeyCode::F8 => Some("F8"),
        KeyCode::F9 => Some("F9"),
        KeyCode::F10 => Some("F10"),
        KeyCode::F11 => Some("F11"),
        KeyCode::F12 => Some("F12"),
        KeyCode::ShiftLeft => Some("ShiftLeft"),
        KeyCode::ShiftRight => Some("ShiftRight"),
        KeyCode::ControlLeft => Some("ControlLeft"),
        KeyCode::ControlRight => Some("ControlRight"),
        KeyCode::AltLeft => Some("AltLeft"),
        KeyCode::AltRight => Some("AltRight"),
        KeyCode::Space => Some("Space"),
        KeyCode::Enter => Some("Enter"),
        KeyCode::Escape => Some("Escape"),
        KeyCode::Tab => Some("Tab"),
        KeyCode::Backspace => Some("Backspace"),
        KeyCode::Delete => Some("Delete"),
        _ => None,
    }
}

/// Action → 物理 KeyCode 群のマッピング (Resource)。
///
/// 1 action に複数 KeyCode を割り当てられる (例: MoveLeft = ArrowLeft + KeyA)。
/// 起動時は [`ActionMap::default`] で組み立て、Phase 2 以降で yml 由来の差し替えに置き換える。
#[derive(Resource, Debug, Clone)]
pub struct ActionMap {
    bindings: HashMap<Action, Vec<KeyCode>>,
}

impl ActionMap {
    /// 任意のバインドから組み立てる。yml ロード経路から使う想定。
    #[must_use]
    pub fn from_bindings(bindings: HashMap<Action, Vec<KeyCode>>) -> Self {
        Self { bindings }
    }

    /// 指定 action のバインドを置き換える (キーコンフィグ画面の編集経路)。空 vec で unbind。
    pub fn set_binding(&mut self, action: Action, codes: Vec<KeyCode>) {
        self.bindings.insert(action, codes);
    }

    /// 規定の `input.yml` の path ([`Self::load`] と同じ解決順)。save 経路 (キーコンフィグ画面)
    /// から呼べるよう公開している。
    #[must_use]
    pub fn default_yml_path() -> PathBuf {
        Self::resolve_yaml_path()
    }

    /// 規定の path から `input.yml` を読み込んで [`ActionMap`] を返す。
    /// path 解決順は env `BEATEMUP_INPUT_CONFIG` > `CARGO_MANIFEST_DIR/config/input.yml`。
    /// ファイルが無い / 読み込みエラー時は default にフォールバック ([`Self::load_or_default`])。
    #[must_use]
    pub fn load() -> Self {
        let path = Self::resolve_yaml_path();
        Self::load_or_default(&path)
    }

    fn resolve_yaml_path() -> PathBuf {
        if let Some(env) = std::env::var_os("BEATEMUP_INPUT_CONFIG") {
            return PathBuf::from(env);
        }
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("input.yml")
    }

    /// `input.yml` を読んで [`ActionMap`] を組み立てる。fail-soft:
    /// - ファイルが無い → info ログを出して [`ActionMap::default`]
    /// - 読み込み / パース失敗 → warn を出して [`ActionMap::default`]
    /// - 未知の action / KeyCode 文字列 → warn を出して skip (他のエントリはそのまま)
    /// - yml に出てこない action は default のバインドが残る (部分上書き)
    pub fn load_or_default(path: &Path) -> Self {
        if !path.exists() {
            tracing::info!(
                path = %path.display(),
                "input_config: not found, using built-in default bindings",
            );
            return Self::default();
        }
        let text = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    path = %path.display(),
                    "input_config: read failed, using default bindings",
                );
                return Self::default();
            }
        };
        let raw: InputConfig = match serde_saphyr::from_str(&text) {
            Ok(c) => c,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    path = %path.display(),
                    "input_config: parse failed, using default bindings",
                );
                return Self::default();
            }
        };
        Self::from_raw_with_default_fallback(raw)
    }

    /// yml の生 [`InputConfig`] を default に被せる。default を起点にして
    /// yml で明示された action だけ override する。空 vec も尊重 (= 「unbind したい」を表現)。
    fn from_raw_with_default_fallback(raw: InputConfig) -> Self {
        let mut bindings = Self::default().bindings;
        for (action_str, key_strs) in raw.bindings {
            let Some(action) = Action::from_snake_case(&action_str) else {
                tracing::warn!(
                    action = %action_str,
                    "input_config: unknown action key, skipping",
                );
                continue;
            };
            let mut codes = Vec::with_capacity(key_strs.len());
            for key_str in &key_strs {
                if let Some(code) = key_code_from_str(key_str) {
                    codes.push(code);
                } else {
                    tracing::warn!(
                        key = %key_str,
                        action = %action_str,
                        "input_config: unknown key name, skipping",
                    );
                }
            }
            bindings.insert(action, codes);
        }
        Self { bindings }
    }

    /// 指定 action にバインドされた KeyCode のいずれかが押下中なら true。
    /// バインド未設定の action は常に false。
    pub fn pressed(&self, keys: &ButtonInput<KeyCode>, action: Action) -> bool {
        self.bindings
            .get(&action)
            .is_some_and(|codes| codes.iter().any(|&k| keys.pressed(k)))
    }

    /// 指定 action のいずれかの KeyCode がこの frame で just_pressed なら true。
    pub fn just_pressed(&self, keys: &ButtonInput<KeyCode>, action: Action) -> bool {
        self.bindings
            .get(&action)
            .is_some_and(|codes| codes.iter().any(|&k| keys.just_pressed(k)))
    }

    /// 指定 action に割り当てられた KeyCode 一覧を返す (キーコンフィグ UI 用)。
    /// バインド未設定なら空 slice。
    #[must_use]
    pub fn bindings_for(&self, action: Action) -> &[KeyCode] {
        self.bindings.get(&action).map_or(&[], Vec::as_slice)
    }
}

/// `input.yml` 出力時に毎回先頭に置くヘッダコメント。bundled default ファイルの
/// 解説 (使い方 + サポート KeyCode 一覧) を保持するため、save 経路でも常に同じ
/// header が出るようにする。
const INPUT_YML_HEADER: &str = "\
# Engine の gameplay 入力バインド (ADR-0016)。
#
# このファイルが置き場の規定 path:
#   env BEATEMUP_INPUT_CONFIG > packages/engine/config/input.yml
#
# fail-soft: ファイル不在やパース失敗時は内部のハードコード default に落ちる
# (`packages/engine/src/shared/input.rs` の `ActionMap::default`)。
#
# 未知の action 名や KeyCode 名は warn を出して skip される。yml に書かれなかった
# action は default のバインドがそのまま残る。空 vec ([]) は「この action を unbind
# したい」を表現する (= 入力を完全に無効化)。
#
# サポートする KeyCode 名 (Bevy `KeyCode` の variant 名にそのまま揃える):
#   - 英字: KeyA - KeyZ
#   - 数字: Digit0 - Digit9
#   - 矢印: ArrowLeft / ArrowRight / ArrowUp / ArrowDown
#   - F1 - F12
#   - 修飾: ShiftLeft / ShiftRight / ControlLeft / ControlRight / AltLeft / AltRight
#   - 特殊: Space / Enter / Escape / Tab / Backspace / Delete
#
# このファイルはキーコンフィグ画面 (Options scene) の Back 確定で自動上書きされる。
# 手で編集した場合はその内容を尊重するが、コメント以外の余計な書式はキー設定画面
# からの save 時に整列形式に揃え直される。
";

/// `bindings:` セクションでキー名の column を揃えるための padding 幅 (`down_attack:` = 12 chars)。
const KEY_COL_WIDTH: usize = 12;

/// [`ActionMap`] を `input.yml` に書き出す (手書き writer)。
///
/// 出力形式は bundled default の `packages/engine/config/input.yml` と一致するよう調整:
/// - 先頭に [`INPUT_YML_HEADER`] (使い方解説 + サポート KeyCode 一覧)
/// - `bindings:` 配下を [`Action::ALL`] 順で出力 (= HashMap の不定順を避ける)
/// - 値は flow style (`[ArrowLeft, KeyA]`) でキー名 column を [`KEY_COL_WIDTH`] で揃える
/// - 未サポート KeyCode が混ざっていた場合は出力からスキップ (warn なし; UI 経由では起こらない)
pub fn write_input_yml(path: &Path, map: &ActionMap) -> io::Result<()> {
    let mut text = String::from(INPUT_YML_HEADER);
    text.push_str("bindings:\n");
    for action in Action::ALL {
        let key_str = action.as_snake_case();
        let names: Vec<&'static str> = map
            .bindings_for(action)
            .iter()
            .filter_map(|&c| key_code_to_str(c))
            .collect();
        let label = format!("{key_str}:");
        // String への write は infallible (fmt::Error は発生し得ない)。
        writeln!(text, "  {label:<KEY_COL_WIDTH$} [{}]", names.join(", "))
            .expect("write to String never fails");
    }
    std::fs::write(path, text)
}

/// `input.yml` の生 schema。snake_case の action 名 → KeyCode 文字列の配列。
///
/// 例:
/// ```yaml
/// bindings:
///   move_left: [ArrowLeft, KeyA]
///   jump: [KeyI]
/// ```
///
/// yml に書かれなかった action は `ActionMap::default` のバインドがそのまま残る
/// (`ActionMap::from_raw_with_default_fallback` 参照)。
#[derive(Debug, Default, Deserialize)]
struct InputConfig {
    #[serde(default)]
    bindings: HashMap<String, Vec<String>>,
}

impl Default for ActionMap {
    /// 現状のハードコードと互換のデフォルトバインド。
    ///
    /// - 移動: 矢印キー + WASD
    /// - 攻撃: Space + J
    /// - 下段攻撃: K
    /// - ジャンプ: I (地上のみ受付は呼び出し側で判定)
    /// - ガード: L (押下中維持)
    fn default() -> Self {
        let bindings = HashMap::from([
            (Action::MoveLeft, vec![KeyCode::ArrowLeft, KeyCode::KeyA]),
            (Action::MoveRight, vec![KeyCode::ArrowRight, KeyCode::KeyD]),
            (Action::MoveUp, vec![KeyCode::ArrowUp, KeyCode::KeyW]),
            (Action::MoveDown, vec![KeyCode::ArrowDown, KeyCode::KeyS]),
            (Action::Attack, vec![KeyCode::Space, KeyCode::KeyJ]),
            (Action::DownAttack, vec![KeyCode::KeyK]),
            (Action::Jump, vec![KeyCode::KeyI]),
            (Action::Guard, vec![KeyCode::KeyL]),
        ]);
        Self { bindings }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_cover_all_actions() {
        let map = ActionMap::default();
        for action in Action::ALL {
            assert!(
                !map.bindings_for(action).is_empty(),
                "default should bind {action:?}"
            );
        }
    }

    #[test]
    fn pressed_returns_true_when_any_bound_key_is_held() {
        let map = ActionMap::default();
        let mut keys = ButtonInput::<KeyCode>::default();
        // MoveLeft は ArrowLeft / KeyA の両方が bind 済み。KeyA だけ押す。
        keys.press(KeyCode::KeyA);
        assert!(map.pressed(&keys, Action::MoveLeft));
        // 他 action は反応しない (sanity)。
        assert!(!map.pressed(&keys, Action::MoveRight));
    }

    #[test]
    fn pressed_returns_false_when_no_bound_key_is_held() {
        let map = ActionMap::default();
        let keys = ButtonInput::<KeyCode>::default();
        for action in Action::ALL {
            assert!(
                !map.pressed(&keys, action),
                "no key held → {action:?}=false"
            );
        }
    }

    #[test]
    fn just_pressed_fires_once_per_press_for_any_bound_key() {
        let map = ActionMap::default();
        let mut keys = ButtonInput::<KeyCode>::default();
        // Attack = Space + KeyJ。Space を just_press。
        keys.press(KeyCode::Space);
        assert!(map.just_pressed(&keys, Action::Attack));
        // 1 frame 経過相当: just_pressed 状態を消す (pressed は維持)。
        keys.clear();
        assert!(!map.just_pressed(&keys, Action::Attack));
        assert!(map.pressed(&keys, Action::Attack));
    }

    #[test]
    fn from_bindings_overrides_default_bindings() {
        // ジャンプを Z キーに変更する keyconfig 経路を想定。
        let mut bindings = HashMap::new();
        bindings.insert(Action::Jump, vec![KeyCode::KeyZ]);
        let map = ActionMap::from_bindings(bindings);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyZ);
        assert!(map.just_pressed(&keys, Action::Jump));
        // 未定義 action は常に false (バインド無し)。
        assert!(!map.pressed(&keys, Action::Attack));
    }

    #[test]
    fn action_snake_case_round_trips_for_all_variants() {
        for action in Action::ALL {
            let s = action.as_snake_case();
            assert_eq!(
                Action::from_snake_case(s),
                Some(action),
                "round-trip {action:?} via {s:?}",
            );
        }
    }

    #[test]
    fn action_from_snake_case_returns_none_for_unknown() {
        assert_eq!(Action::from_snake_case(""), None);
        assert_eq!(Action::from_snake_case("MoveLeft"), None); // PascalCase は不可
        assert_eq!(Action::from_snake_case("fly"), None);
    }

    #[test]
    fn key_code_round_trips_for_each_supported_family() {
        // 各 family から 1 つずつ pick して往復確認 (全 64 個書くと冗長なので族代表)。
        let samples = [
            (KeyCode::KeyA, "KeyA"),
            (KeyCode::KeyZ, "KeyZ"),
            (KeyCode::Digit0, "Digit0"),
            (KeyCode::Digit9, "Digit9"),
            (KeyCode::ArrowLeft, "ArrowLeft"),
            (KeyCode::ArrowDown, "ArrowDown"),
            (KeyCode::F1, "F1"),
            (KeyCode::F12, "F12"),
            (KeyCode::ShiftLeft, "ShiftLeft"),
            (KeyCode::ControlRight, "ControlRight"),
            (KeyCode::AltLeft, "AltLeft"),
            (KeyCode::Space, "Space"),
            (KeyCode::Enter, "Enter"),
            (KeyCode::Escape, "Escape"),
            (KeyCode::Tab, "Tab"),
            (KeyCode::Backspace, "Backspace"),
            (KeyCode::Delete, "Delete"),
        ];
        for (code, name) in samples {
            assert_eq!(
                key_code_from_str(name),
                Some(code),
                "from_str({name:?}) == {code:?}"
            );
            assert_eq!(
                key_code_to_str(code),
                Some(name),
                "to_str({code:?}) == {name:?}"
            );
        }
    }

    #[test]
    fn key_code_from_str_returns_none_for_unsupported() {
        assert_eq!(key_code_from_str(""), None);
        assert_eq!(key_code_from_str("Mouse0"), None);
        // サポート外の Bevy variant 名 (NumpadEnter は意図的に未サポート)。
        assert_eq!(key_code_from_str("NumpadEnter"), None);
    }

    #[test]
    fn key_code_to_str_returns_none_for_unsupported_variant() {
        // F13 は KeyCode に存在するがサポート外 (gameplay 用途で不要)。
        assert_eq!(key_code_to_str(KeyCode::F13), None);
    }

    /// yml で明示した action は override、書かれてない action は default のまま。
    #[test]
    fn from_raw_partial_override_keeps_default_for_unspecified_actions() {
        let raw = InputConfig {
            bindings: HashMap::from([("jump".to_string(), vec!["KeyZ".to_string()])]),
        };
        let map = ActionMap::from_raw_with_default_fallback(raw);
        assert_eq!(map.bindings_for(Action::Jump), &[KeyCode::KeyZ]);
        // 他は default のまま (例: Attack = Space + KeyJ)
        assert_eq!(
            map.bindings_for(Action::Attack),
            &[KeyCode::Space, KeyCode::KeyJ]
        );
    }

    /// 空 vec は「全 unbind」を意味する (default を残さない)。
    #[test]
    fn from_raw_empty_vec_unbinds_action() {
        let raw = InputConfig {
            bindings: HashMap::from([("jump".to_string(), vec![])]),
        };
        let map = ActionMap::from_raw_with_default_fallback(raw);
        assert!(map.bindings_for(Action::Jump).is_empty());
    }

    /// 未知 action 名は skip + default は維持。
    #[test]
    fn from_raw_unknown_action_is_skipped_and_defaults_preserved() {
        let raw = InputConfig {
            bindings: HashMap::from([("fly".to_string(), vec!["KeyV".to_string()])]),
        };
        let map = ActionMap::from_raw_with_default_fallback(raw);
        // Jump は default
        assert_eq!(map.bindings_for(Action::Jump), &[KeyCode::KeyI]);
    }

    /// 未知 KeyCode 文字列は skip、認識できたものだけ残す。
    #[test]
    fn from_raw_unknown_key_string_is_skipped_within_an_action() {
        let raw = InputConfig {
            bindings: HashMap::from([(
                "jump".to_string(),
                vec![
                    "KeyZ".to_string(),
                    "Mouse99".to_string(),
                    "KeyX".to_string(),
                ],
            )]),
        };
        let map = ActionMap::from_raw_with_default_fallback(raw);
        // 認識できた KeyZ と KeyX だけ残る。
        assert_eq!(
            map.bindings_for(Action::Jump),
            &[KeyCode::KeyZ, KeyCode::KeyX]
        );
    }

    #[test]
    fn load_or_default_returns_default_when_path_missing() {
        let path = std::path::PathBuf::from("definitely")
            .join("does")
            .join("not")
            .join("exist.yml");
        let map = ActionMap::load_or_default(&path);
        // default と同じ bind 数
        for action in Action::ALL {
            assert!(
                !map.bindings_for(action).is_empty(),
                "default should remain when file missing for {action:?}"
            );
        }
    }

    #[test]
    fn menu_action_pressed_recognizes_any_bound_key() {
        // Confirm = Enter + Space + Attack bind。Space 押下で反応。
        let map = ActionMap::default();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Space);
        assert!(menu_action_pressed(&keys, &map, MenuAction::Confirm));
        assert!(menu_action_just_pressed(&keys, &map, MenuAction::Confirm));
        // 他 menu action は反応しない (sanity)。Cancel / Up は Space に bind されてない。
        assert!(!menu_action_pressed(&keys, &map, MenuAction::Cancel));
        assert!(!menu_action_pressed(&keys, &map, MenuAction::Up));
    }

    #[test]
    fn menu_action_cancel_uses_escape() {
        let map = ActionMap::default();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        assert!(menu_action_just_pressed(&keys, &map, MenuAction::Cancel));
        assert!(!menu_action_pressed(&keys, &map, MenuAction::Confirm));
    }

    /// gameplay Action の bind が menu 側でも効くこと (Attack = Confirm, Jump = Cancel)。
    #[test]
    fn menu_action_falls_through_to_gameplay_bindings() {
        let map = ActionMap::default();
        // default で Attack = Space + KeyJ。KeyJ で Confirm 反応。
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyJ);
        assert!(menu_action_just_pressed(&keys, &map, MenuAction::Confirm));
        // default で Jump = KeyI。KeyI で Cancel 反応。
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyI);
        assert!(menu_action_just_pressed(&keys, &map, MenuAction::Cancel));
        // MoveUp = ArrowUp + KeyW。ArrowUp で Up 反応 (固定キー側でも gameplay 側でも当たる)。
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ArrowUp);
        assert!(menu_action_just_pressed(&keys, &map, MenuAction::Up));
    }

    /// キーコンフィグで Attack を Z に変えたら、Z で Confirm が反応するようになる。
    #[test]
    fn menu_action_confirm_follows_remapped_attack() {
        let mut bindings = ActionMap::default().bindings;
        bindings.insert(Action::Attack, vec![KeyCode::KeyZ]);
        let map = ActionMap::from_bindings(bindings);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyZ);
        assert!(menu_action_just_pressed(&keys, &map, MenuAction::Confirm));
        // 元の Space は固定キー側で反応する (固定 + gameplay の OR なので二重に効く)。
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Space);
        assert!(menu_action_just_pressed(&keys, &map, MenuAction::Confirm));
    }

    #[test]
    fn write_input_yml_round_trips_default_bindings() {
        // default → write → load で同じ binding が復元できることを確認。
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("input.yml");
        let original = ActionMap::default();
        write_input_yml(&path, &original).expect("write");
        let loaded = ActionMap::load_or_default(&path);
        for action in Action::ALL {
            assert_eq!(
                loaded.bindings_for(action),
                original.bindings_for(action),
                "round-trip mismatch for {action:?}",
            );
        }
    }

    #[test]
    fn write_input_yml_round_trips_custom_bindings() {
        // ジャンプを KeyZ に変更したカスタムバインドの round-trip。
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("input.yml");
        let mut custom_bindings = ActionMap::default().bindings;
        custom_bindings.insert(Action::Jump, vec![KeyCode::KeyZ]);
        custom_bindings.insert(Action::Guard, vec![]); // unbind 表現も round-trip すること
        let original = ActionMap::from_bindings(custom_bindings);
        write_input_yml(&path, &original).expect("write");
        let loaded = ActionMap::load_or_default(&path);
        assert_eq!(loaded.bindings_for(Action::Jump), &[KeyCode::KeyZ]);
        assert!(loaded.bindings_for(Action::Guard).is_empty());
        // 他は default 維持
        assert_eq!(
            loaded.bindings_for(Action::MoveLeft),
            &[KeyCode::ArrowLeft, KeyCode::KeyA]
        );
    }

    #[test]
    fn write_input_yml_starts_with_header_comment() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("input.yml");
        write_input_yml(&path, &ActionMap::default()).expect("write");
        let text = std::fs::read_to_string(&path).expect("read");
        // header の冒頭が含まれていれば、save 経由でもコメントが保たれている。
        assert!(
            text.starts_with("# Engine の gameplay 入力バインド"),
            "header missing, got:\n{text}",
        );
        // Action::ALL 順で出力されているか (move_left が move_right より先)
        let move_left_pos = text.find("move_left:").expect("move_left present");
        let move_right_pos = text.find("move_right:").expect("move_right present");
        assert!(
            move_left_pos < move_right_pos,
            "ALL order expected: move_left then move_right",
        );
    }

    #[test]
    fn load_or_default_parses_partial_yaml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("input.yml");
        std::fs::write(&path, "bindings:\n  jump: [KeyZ]\n  attack: [KeyJ, KeyX]\n")
            .expect("write yml");
        let map = ActionMap::load_or_default(&path);
        assert_eq!(map.bindings_for(Action::Jump), &[KeyCode::KeyZ]);
        assert_eq!(
            map.bindings_for(Action::Attack),
            &[KeyCode::KeyJ, KeyCode::KeyX]
        );
        // 他は default 維持
        assert_eq!(
            map.bindings_for(Action::MoveLeft),
            &[KeyCode::ArrowLeft, KeyCode::KeyA]
        );
    }
}
