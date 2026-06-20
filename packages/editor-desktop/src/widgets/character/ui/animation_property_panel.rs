//! AnimationEditor 右ペインの Property Panel。Frame 選択依存の各セクションをまとめる。
//!
//! ファイル分割:
//! - `loop_section`: Animation 全体の Loop / Loop Start (Frame 選択に依存しない)
//! - `frame_properties`: Frame の Duration
//! - `layers`: Layer 一覧と選択中 Layer の Editor
//! - `overrides`: Frame レベルで Sprite を上書きする 3 系統 (Flip / Pivot Offset / Body / Attack Box)
//!
//! `parse_flip` / `flip_to_value` は layers と overrides の両方で使われるので、
//! 親モジュールに置いて `super::` 参照させる。

use dioxus::prelude::*;

use super::sprite_reference::{ReferenceSection, SpriteReference};
use crate::entities::character::{Animation, BoxKind, Character, SelectedBox, use_playback};
use crate::shared::{FlipMode, UseHistory};

mod frame_properties;
mod frame_sound;
mod layers;
mod loop_section;
mod overrides;
mod role_section;

use frame_properties::FramePropertiesSection;
use frame_sound::FrameSoundSection;
use layers::{LayerListSection, SelectedLayerEditor};
use loop_section::AnimationLoopSection;
use overrides::FrameOverridesSection;
use role_section::AnimationRoleSection;

#[component]
pub fn AnimationPropertyPanel(
    character: Character,
    draft: Signal<Animation>,
    history: UseHistory<Animation>,
    selected_frame_index: ReadSignal<usize>,
    selected_layer_index: Signal<Option<usize>>,
    selected_box: Signal<Option<SelectedBox>>,
    references: Signal<Vec<SpriteReference>>,
) -> Element {
    let frame_index = selected_frame_index();
    let frame = {
        let read = draft.read();
        read.frames.get(frame_index).cloned()
    };
    // 再生中は編集 UI を一括 disable。fieldset[disabled] で form 要素を一括停止する。
    let locked = use_playback()().locks_editing();

    rsx! {
        fieldset {
            class: "h-full overflow-y-auto p-3 space-y-4 bg-base-200 rounded-box border-0 m-0 min-w-0",
            disabled: locked,
            // Role / Variant は engine への semantic な紐付けを決める設定 (Frame 非依存)。
            // Loop 設定の上に置いて最初に視界に入るようにする。
            // Phase 6: 終了条件ステータス行が Physics の lie_down_duration_ms / rise_duration_ms
            // を参照するので、physics 値を渡す。Character.physics は ReadSignal にはできない
            // (props は Character clone) ので、固定値として渡す (Character 更新時に親が
            // AnimationPropertyPanel ごと再評価する想定)。
            AnimationRoleSection { draft, history, physics: character.physics }

            // Animation 全体の設定 (Loop)。Frame 選択に依存しないので上段に常時表示する。
            div { class: "border-t border-base-300 pt-3",
                AnimationLoopSection { draft, history }
            }

            if let Some(frame) = frame {
                div { class: "border-t border-base-300 pt-3",
                    FramePropertiesSection {
                        draft,
                        history,
                        frame_index,
                        duration: frame.duration,
                    }
                }

                div { class: "border-t border-base-300 pt-3",
                    LayerListSection {
                        character: character.clone(),
                        draft,
                        history,
                        frame_index,
                        layers: frame.layers.clone(),
                        selected_layer_index,
                    }
                }

                if let Some(li) = selected_layer_index() {
                    if let Some(layer) = frame.layers.get(li).cloned() {
                        div { class: "border-t border-base-300 pt-3",
                            SelectedLayerEditor {
                                character: character.clone(),
                                draft,
                                history,
                                frame_index,
                                layer_index: li,
                                layer,
                                selected_layer_index,
                            }
                        }
                    }
                }

                div { class: "border-t border-base-300 pt-3",
                    FrameOverridesSection {
                        draft,
                        history,
                        frame_index,
                        character_depth: character.depth,
                        flip: frame.flip,
                        pivot_offset: frame.pivot_point_offset,
                        // 3 状態判定用の「box 個数」だけ渡す (None=Inherit / Some(0)=Disable / Some(n>0)=Override)。
                        // Body / Attack で型が違う (HitBox / AttackBox) ので、実データは子側で取り直す。
                        body_state: BoxKind::Body.frame_override_state(&frame),
                        attack_state: BoxKind::Attack.frame_override_state(&frame),
                        selected_box,
                    }
                }

                div { class: "border-t border-base-300 pt-3",
                    FrameSoundSection {
                        character: character.clone(),
                        draft,
                        history,
                        frame_index,
                        selected: frame.sound.clone(),
                    }
                }

                div { class: "border-t border-base-300 pt-3",
                    ReferenceSection { character: character.clone(), references }
                }
            } else {
                p { class: "text-sm text-base-content/60 italic border-t border-base-300 pt-3",
                    "Frame が選択されていません。"
                }
            }
        }
    }
}

pub(super) fn parse_flip(s: &str) -> Option<FlipMode> {
    match s {
        "horizontal" => Some(FlipMode::Horizontal),
        "vertical" => Some(FlipMode::Vertical),
        "both" => Some(FlipMode::Both),
        _ => None,
    }
}

pub(super) fn flip_to_value(f: Option<FlipMode>) -> &'static str {
    match f {
        Some(FlipMode::Horizontal) => "horizontal",
        Some(FlipMode::Vertical) => "vertical",
        Some(FlipMode::Both) => "both",
        None => "none",
    }
}
