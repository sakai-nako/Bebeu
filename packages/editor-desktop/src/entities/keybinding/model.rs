use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::shared::{KeyBinding, KeyModifiers};

// 修飾キーの const テーブル。`define_actions!` のデフォルト指定で使う。
//
// struct update syntax (`..NONE`) は stable const でも使えるが、可読性のため全フィールド
// 明示で書き下す。新しい修飾キー組合せが必要になったらここに 1 行追加する。
const MOD_NONE: KeyModifiers = KeyModifiers {
    ctrl: false,
    shift: false,
    alt: false,
    meta: false,
};
const MOD_CTRL: KeyModifiers = KeyModifiers {
    ctrl: true,
    shift: false,
    alt: false,
    meta: false,
};
const MOD_SHIFT: KeyModifiers = KeyModifiers {
    ctrl: false,
    shift: true,
    alt: false,
    meta: false,
};
const MOD_CTRL_SHIFT: KeyModifiers = KeyModifiers {
    ctrl: true,
    shift: true,
    alt: false,
    meta: false,
};

/// `Action` の variant 定義 + `label` / `id` / `ALL` / `default_binding` を 1 つのテーブルから
/// 生成する。1 行に variant / 日本語ラベル / snake_case 識別子 / デフォルト修飾キー / デフォルト
/// メインキーをまとめる。
///
/// 新しいショートカット対応アクションを追加するときは:
/// 1. このマクロ呼び出しに 1 行追加
/// 2. 該当画面で `use_keyboard_action(Action::NewOne, callback)` を 1 行追加
macro_rules! define_actions {
    ( $(
        $(#[doc = $doc:literal])*
        $variant:ident,
            $label:literal,
            $id:literal,
            $mods:expr,
            $key:literal
    );* $(;)? ) => {
        /// エディタが認識する「ユーザー設定可能なコマンド」の集合。
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum Action {
            $(
                $(#[doc = $doc])*
                $variant,
            )*
        }

        impl Action {
            /// UI 表示用の日本語ラベル。
            #[must_use]
            pub fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)*
                }
            }

            /// `key` 属性等に使う識別子 (snake_case)。
            #[must_use]
            pub fn id(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)*
                }
            }

            /// 全 Action を列挙する。Preferences の編集 UI でループに使う。
            pub const ALL: &'static [Action] = &[
                $(Self::$variant,)*
            ];

            /// この Action のデフォルトキーバインド。`KeyBindings::default()` も
            /// 「個別アクションをデフォルトに戻す」もここを参照する。
            #[must_use]
            pub fn default_binding(self) -> KeyBinding {
                match self {
                    $(
                        Self::$variant => KeyBinding {
                            modifiers: $mods,
                            key: $key.to_string(),
                        },
                    )*
                }
            }
        }
    };
}

// Save / Undo / Redo は SpriteGroup / Animation / SoundGroup の 3 エディタ共通のコマンドとして
// 1 つの Action にまとめる。各エディタの Actions コンポーネントが mount された時にだけ
// `use_keyboard_action(Action::Save, ...)` を register するので、active な editor 1 つだけが
// 処理する設計（unmount で hook ごと消える）。設定 UI からも 1 行だけで管理できる。
//
// MovePivot* は「上に動かす (= pivot_point[1] が減る)」で画像が画面上で下に動く意味論なので、
// デフォルトキーは Action 名と逆方向の矢印キー (Up→ArrowDown 等) を割り当てている。
define_actions! {
    /// 編集内容を保存する（active な Editor が処理する）
    Save, "保存", "save", MOD_CTRL, "s";
    /// 直前の編集を取り消す（active な Editor が処理する）
    Undo, "元に戻す", "undo", MOD_CTRL, "z";
    /// 取り消した編集を再適用する（active な Editor が処理する）
    Redo, "やり直す", "redo", MOD_CTRL_SHIFT, "z";
    /// SpriteGroup Editor で前の Sprite を選択する
    SelectPrevSprite, "前の Sprite を選択", "select_prev_sprite", MOD_CTRL, "ArrowLeft";
    /// SpriteGroup Editor で次の Sprite を選択する
    SelectNextSprite, "次の Sprite を選択", "select_next_sprite", MOD_CTRL, "ArrowRight";
    /// SpriteGroup Editor で最初の Sprite を選択する
    SelectFirstSprite, "最初の Sprite を選択", "select_first_sprite", MOD_CTRL, "Home";
    /// SpriteGroup Editor で最後の Sprite を選択する
    SelectLastSprite, "最後の Sprite を選択", "select_last_sprite", MOD_CTRL, "End";
    /// SpriteGroup Editor で選択中 Sprite を 1 つ前へ移動する
    MoveSpritePrev, "選択中 Sprite を前へ移動", "move_sprite_prev", MOD_CTRL_SHIFT, "ArrowLeft";
    /// SpriteGroup Editor で選択中 Sprite を 1 つ後ろへ移動する
    MoveSpriteNext, "選択中 Sprite を後ろへ移動", "move_sprite_next", MOD_CTRL_SHIFT, "ArrowRight";
    /// SpriteGroup Editor で選択中 Sprite の Pivot を上へ移動する
    MovePivotUp, "Pivot を上へ移動", "move_pivot_up", MOD_SHIFT, "ArrowDown";
    /// SpriteGroup Editor で選択中 Sprite の Pivot を下へ移動する
    MovePivotDown, "Pivot を下へ移動", "move_pivot_down", MOD_SHIFT, "ArrowUp";
    /// SpriteGroup Editor で選択中 Sprite の Pivot を左へ移動する
    MovePivotLeft, "Pivot を左へ移動", "move_pivot_left", MOD_SHIFT, "ArrowRight";
    /// SpriteGroup Editor で選択中 Sprite の Pivot を右へ移動する
    MovePivotRight, "Pivot を右へ移動", "move_pivot_right", MOD_SHIFT, "ArrowLeft";
    /// Animation Editor で前の Frame を選択する
    SelectPrevFrame, "前の Frame を選択", "select_prev_frame", MOD_CTRL, "ArrowLeft";
    /// Animation Editor で次の Frame を選択する
    SelectNextFrame, "次の Frame を選択", "select_next_frame", MOD_CTRL, "ArrowRight";
    /// Animation Editor で最初の Frame を選択する
    SelectFirstFrame, "最初の Frame を選択", "select_first_frame", MOD_CTRL, "Home";
    /// Animation Editor で最後の Frame を選択する
    SelectLastFrame, "最後の Frame を選択", "select_last_frame", MOD_CTRL, "End";
    /// Animation Editor の再生 / 一時停止トグル
    PlayPauseAnimation, "Animation を再生 / 一時停止", "play_pause_animation", MOD_NONE, "Space";
    /// Animation Editor の再生停止 (先頭フレームに戻る)
    StopAnimation, "Animation の再生を停止 (先頭へ)", "stop_animation", MOD_SHIFT, "Space";
}

/// Action → KeyBinding のマッピング。
///
/// `#[serde(transparent)]` で `HashMap` を直接 YAML に出力する。Action の serde 表現は
/// snake_case 文字列なので YAML のマップキーとして自然に扱える。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyBindings(HashMap<Action, KeyBinding>);

impl Default for KeyBindings {
    fn default() -> Self {
        let mut map = HashMap::new();
        for action in Action::ALL.iter().copied() {
            map.insert(action, action.default_binding());
        }
        Self(map)
    }
}

impl KeyBindings {
    /// 押下されたキー組合せが割り当てられたアクションをすべて返す。
    /// 通常は Action と KeyBinding は 1:1（例: Ctrl+S → Save）だが、ユーザーが意図的に
    /// 複数 Action を同じキーに割り当てた場合に備えて Vec を返す。設定 UI 側はこの結果を
    /// 競合警告に使う。
    #[must_use]
    pub fn resolve(&self, kb: &KeyBinding) -> Vec<Action> {
        self.0
            .iter()
            .filter(|(_, bound)| *bound == kb)
            .map(|(a, _)| *a)
            .collect()
    }

    /// 与えられた `kb` がすでに別の Action に紐付いている場合、その Action を全て返す。
    /// `except` 自身は除外する (同じ Action に再設定するときは競合扱いしない)。
    #[must_use]
    pub fn conflicts(&self, kb: &KeyBinding, except: Action) -> Vec<Action> {
        self.0
            .iter()
            .filter(|(a, bound)| **a != except && *bound == kb)
            .map(|(a, _)| *a)
            .collect()
    }

    /// アクションのキーバインドを設定する。同 Action の既存値は上書き。
    pub fn set(&mut self, action: Action, kb: KeyBinding) {
        self.0.insert(action, kb);
    }

    /// アクションのキーバインドを未設定にする。元から未設定だった場合は何もしない。
    pub fn remove(&mut self, action: Action) {
        self.0.remove(&action);
    }

    /// すべてのバインドを未設定にする。`Default::default()` (= 既定キー入り) ではなく、空のマップ。
    pub fn clear_all(&mut self) {
        self.0.clear();
    }

    #[must_use]
    pub fn get(&self, action: Action) -> Option<&KeyBinding> {
        self.0.get(&action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl_s() -> KeyBinding {
        KeyBinding {
            modifiers: KeyModifiers {
                ctrl: true,
                ..Default::default()
            },
            key: "s".into(),
        }
    }

    fn ctrl_shift_s() -> KeyBinding {
        KeyBinding {
            modifiers: KeyModifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            key: "s".into(),
        }
    }

    #[test]
    fn default_bindings_contain_save_ctrl_s() {
        let bindings = KeyBindings::default();
        assert_eq!(bindings.get(Action::Save), Some(&ctrl_s()));
    }

    #[test]
    fn key_bindings_resolve_returns_action() {
        let bindings = KeyBindings::default();
        // Save / Undo / Redo は 3 エディタ共通の単一 Action に統合済み
        let resolved = bindings.resolve(&ctrl_s());
        assert_eq!(resolved, vec![Action::Save]);
    }

    #[test]
    fn key_bindings_resolve_returns_empty_for_unbound() {
        // ctrl+shift+s はデフォルトで未割り当て（Redo は z+ctrl+shift）
        let bindings = KeyBindings::default();
        assert!(bindings.resolve(&ctrl_shift_s()).is_empty());
    }

    #[test]
    fn key_bindings_conflicts_excludes_self() {
        let bindings = KeyBindings::default();
        // 同 Action に同じ binding を上書きするのは競合ではない（自分自身は除外される）。
        // Save が単一 Action になったので、Ctrl+S の conflict は無くなる。
        let conflicts = bindings.conflicts(&ctrl_s(), Action::Save);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn key_bindings_conflicts_detects_cross_action_collision() {
        let mut bindings = KeyBindings::default();
        // SelectPrevSprite を Ctrl+S にぶつけると、Save から見て conflict として返る
        bindings.set(Action::SelectPrevSprite, ctrl_s());
        let conflicts = bindings.conflicts(&ctrl_s(), Action::Save);
        assert_eq!(conflicts, vec![Action::SelectPrevSprite]);
    }

    #[test]
    fn key_bindings_set_overwrites() {
        let mut bindings = KeyBindings::default();
        bindings.set(Action::Save, ctrl_shift_s());
        assert_eq!(bindings.get(Action::Save), Some(&ctrl_shift_s()));
    }

    #[test]
    fn key_bindings_remove_unsets_action() {
        let mut bindings = KeyBindings::default();
        assert!(bindings.get(Action::Save).is_some());
        bindings.remove(Action::Save);
        assert!(bindings.get(Action::Save).is_none());
    }

    #[test]
    fn key_bindings_clear_all_empties_map() {
        let mut bindings = KeyBindings::default();
        bindings.clear_all();
        for action in Action::ALL.iter().copied() {
            assert!(
                bindings.get(action).is_none(),
                "after clear_all, {action:?} should be unset",
            );
        }
        // 空でも resolve / conflicts は安全に動作する
        assert!(bindings.resolve(&ctrl_s()).is_empty());
        assert!(bindings.conflicts(&ctrl_s(), Action::Save).is_empty());
    }

    #[test]
    fn default_binding_matches_default_keybindings_map() {
        let bindings = KeyBindings::default();
        for action in Action::ALL.iter().copied() {
            assert_eq!(
                bindings.get(action),
                Some(&action.default_binding()),
                "{action:?} の default_binding と KeyBindings::default() がずれている",
            );
        }
    }

    #[test]
    fn key_bindings_yaml_round_trips() -> anyhow::Result<()> {
        let bindings = KeyBindings::default();
        let yaml = serde_saphyr::to_string(&bindings)?;
        // YAML には "save: ctrl+s" 相当が含まれるはず
        assert!(
            yaml.contains("save"),
            "expected snake_case key in yaml, got: {yaml}"
        );
        assert!(
            yaml.contains("ctrl+s"),
            "expected ctrl+s string form in yaml, got: {yaml}"
        );
        let parsed: KeyBindings = serde_saphyr::from_str(&yaml)?;
        assert_eq!(parsed, bindings);
        Ok(())
    }
}
