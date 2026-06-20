//! Sprite Canvas / Animation Canvas に重ねて表示する各種マーカー
//! (Pivot / Frame Pivot / Layer Pivot / Body Box / Attack Box / Reference / Origin /
//! Image Frame) の表示・非表示を粒度ありで切り替える。
//!
//! 両 canvas でマーカー構成が違う (Sprite には単一 Pivot のみ・Origin 無し、Animation には
//! Frame/Layer Pivot と Origin がある) ため、`CanvasVisibility` は両者の **superset** を
//! 1 構造体で持つ。各 canvas は `CanvasVisibilityBar` に自分が使う `Field` の配列を渡し、
//! 該当するトグルだけを表示する。参照しないフィールドは無視されるだけ (害は無い)。
//!
//! セッション内 Signal で持つだけで disk には書かない (`SpriteReference` と同じ扱い)。
//! 後続で再生機能が入った際は、再生開始時に `editing_only_off()` を当てて停止時に
//! 元の状態へ戻す、という運用を想定している。

use dioxus::prelude::*;

/// Canvas マーカー類の表示フラグ。`Default::default()` は全て `true` (現状維持)。
/// 意味的に独立した「マーカー種別ごとの on/off」の集まりなので、
/// clippy::struct_excessive_bools のリファクタ提案 (state machine 化) は当てはまらない。
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasVisibility {
    /// Sprite Canvas の単一 Pivot marker。Animation Canvas では未使用 (frame_pivot / layer_pivot を使う)。
    pub pivot: bool,
    pub frame_pivot: bool,
    pub layer_pivot: bool,
    pub body_boxes: bool,
    pub attack_boxes: bool,
    pub references: bool,
    pub origin: bool,
    /// 元画像の外枠 (画像 dimensions の矩形)。Sprite / Animation 両 canvas 共通。
    pub image_frame: bool,
}

impl Default for CanvasVisibility {
    fn default() -> Self {
        Self {
            pivot: true,
            frame_pivot: true,
            layer_pivot: true,
            body_boxes: true,
            attack_boxes: true,
            references: true,
            origin: true,
            image_frame: true,
        }
    }
}

impl CanvasVisibility {
    /// 全マーカー非表示。
    #[must_use]
    pub fn all_off() -> Self {
        Self {
            pivot: false,
            frame_pivot: false,
            layer_pivot: false,
            body_boxes: false,
            attack_boxes: false,
            references: false,
            origin: false,
            image_frame: false,
        }
    }

    /// 編集系マーカー (pivot / boxes / origin / image_frame) を off、references は維持。
    /// 再生プリセット用。
    #[must_use]
    pub fn editing_only_off() -> Self {
        Self {
            references: true,
            ..Self::all_off()
        }
    }
}

/// 各フィールドへ抜き差しするためのキー。`CanvasVisibilityBar` の table-driven 描画用。
/// 各 canvas は自分が表示するトグルだけを `Vec<Field>` で Bar に渡す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Pivot,
    FramePivot,
    LayerPivot,
    BodyBoxes,
    AttackBoxes,
    References,
    Origin,
    ImageFrame,
}

impl Field {
    fn get(self, v: CanvasVisibility) -> bool {
        match self {
            Self::Pivot => v.pivot,
            Self::FramePivot => v.frame_pivot,
            Self::LayerPivot => v.layer_pivot,
            Self::BodyBoxes => v.body_boxes,
            Self::AttackBoxes => v.attack_boxes,
            Self::References => v.references,
            Self::Origin => v.origin,
            Self::ImageFrame => v.image_frame,
        }
    }

    /// 指定値を書き込む。一括トグル (All ボタン) からも個別トグルからも使う。
    fn set(self, v: &mut CanvasVisibility, on: bool) {
        match self {
            Self::Pivot => v.pivot = on,
            Self::FramePivot => v.frame_pivot = on,
            Self::LayerPivot => v.layer_pivot = on,
            Self::BodyBoxes => v.body_boxes = on,
            Self::AttackBoxes => v.attack_boxes = on,
            Self::References => v.references = on,
            Self::Origin => v.origin = on,
            Self::ImageFrame => v.image_frame = on,
        }
    }

    fn toggle(self, v: &mut CanvasVisibility) {
        self.set(v, !self.get(*v));
    }

    /// ボタンに出す 1 文字ラベル。
    fn label(self) -> &'static str {
        match self {
            Self::Pivot | Self::FramePivot => "P",
            Self::LayerPivot => "L",
            Self::BodyBoxes => "B",
            Self::AttackBoxes => "A",
            Self::References => "R",
            Self::Origin => "O",
            Self::ImageFrame => "F",
        }
    }

    /// ホバー時のフルタイトル。
    fn title(self) -> &'static str {
        match self {
            Self::Pivot => "Pivot",
            Self::FramePivot => "Frame Pivot",
            Self::LayerPivot => "Layer Pivot",
            Self::BodyBoxes => "Body Boxes",
            Self::AttackBoxes => "Attack Boxes",
            Self::References => "References",
            Self::Origin => "Origin Marker",
            Self::ImageFrame => "Image Frame (元画像の外枠)",
        }
    }
}

/// Canvas 左上に floating 表示するトグルバー。`fields` で表示するトグル項目とその順序を指定する。
#[component]
pub fn CanvasVisibilityBar(
    mut visibility: Signal<CanvasVisibility>,
    fields: Vec<Field>,
) -> Element {
    let snap = visibility();
    // 「すべて on」判定は渡された fields 限定 (canvas が参照しないフィールドは無視する)。
    let all_on = fields.iter().all(|f| f.get(snap));

    // 「すべて on」のときは全 off、そうでなければ全 on にする一括トグル。
    let fields_for_all = fields.clone();
    let on_all_click = move |_evt: MouseEvent| {
        let cur = visibility();
        let target = !fields_for_all.iter().all(|f| f.get(cur));
        let mut next = cur;
        for f in &fields_for_all {
            f.set(&mut next, target);
        }
        visibility.set(next);
    };
    let all_class = if all_on {
        "btn btn-xs btn-primary font-mono px-2"
    } else {
        "btn btn-xs btn-ghost btn-outline font-mono px-2 opacity-60"
    };
    let all_title = if all_on {
        "全マーカーを非表示にする"
    } else {
        "全マーカーを表示する"
    };

    rsx! {
        div { class: "flex items-center gap-1",
            button {
                r#type: "button",
                class: "{all_class}",
                title: "{all_title}",
                onclick: on_all_click,
                "All"
            }
            // 個別トグルとの視覚的区切り
            div { class: "w-px h-4 bg-base-content/30 mx-0.5" }
            for field in fields.iter().copied() {
                ToggleButton {
                    key: "{field:?}",
                    is_on: field.get(snap),
                    field,
                    visibility,
                }
            }
        }
    }
}

#[component]
fn ToggleButton(is_on: bool, field: Field, mut visibility: Signal<CanvasVisibility>) -> Element {
    let class = if is_on {
        "btn btn-xs btn-primary font-mono w-7 px-0"
    } else {
        "btn btn-xs btn-ghost btn-outline font-mono w-7 px-0 opacity-60"
    };

    let on_click = move |_evt: MouseEvent| {
        let mut next = visibility();
        field.toggle(&mut next);
        visibility.set(next);
    };

    rsx! {
        button {
            r#type: "button",
            class: "{class}",
            title: "{field.title()}",
            onclick: on_click,
            "{field.label()}"
        }
    }
}
