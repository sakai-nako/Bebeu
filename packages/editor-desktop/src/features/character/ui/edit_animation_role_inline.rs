use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{Animation, CharacterRepository, Role, use_characters_refresh};

/// AnimationRow 上で Animation の Role + Variant を inline 編集するフォーム。
///
/// 表示モード (色付き badge + ✎ トリガー) は呼び出し側の widget (AnimationRow) が持ち、
/// このコンポーネントは編集フォームと永続化だけを担う。`editing` Signal は親と共有し、
/// Save / Cancel で親側に閉じる合図を返す (編集フォームは editing 中だけ mount される)。
///
/// **レイアウト**: ラッパ要素を持たず select / variant / Save / Cancel を直接返す (fragment)。
/// これにより AnimationRow の li flex に直接並び、非編集時の badge/✎/Rename/Delete と同位置に
/// 収まって編集切り替え時のレイアウト差を最小化する。select は固定幅、variant 入力は
/// multi-cardinality role のときだけ出す (single では不要なので省いてさらにコンパクトに)。
///
/// 正規化規約は AnimationRoleSection (property panel) と揃える:
/// - single-cardinality role に切り替えたら variant=0
/// - Custom 以外に切り替えたら export_number=None
///
/// 単一 Animation YAML だけを書き直す `update_animation` を使う。frames / boxes はそのまま温存される。
#[component]
pub fn EditAnimationRoleInline(
    character_name: String,
    animation: Animation,
    mut editing: Signal<bool>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();

    // editing 中だけ mount されるので、初期値は現在の Animation で seed すれば常に最新。
    let mut draft_role = use_signal(|| animation.role);
    let mut draft_variant = use_signal(|| animation.variant);
    let mut error = use_signal(|| None::<String>);

    let on_role = move |evt: Event<FormData>| {
        let Some(new_role) = Role::from_yaml_value(&evt.value()) else {
            return;
        };
        draft_role.set(new_role);
        // single-cardinality に切り替えたら variant を 0 に正規化する。
        if new_role.is_single_cardinality() {
            draft_variant.set(0);
        }
    };

    let on_variant = move |evt: Event<FormData>| {
        if let Ok(v) = evt.value().trim().parse::<u32>() {
            draft_variant.set(v);
        }
    };

    let on_save = {
        let animation = animation.clone();
        let character_name = character_name.clone();
        move |_| {
            let new_role = draft_role();
            let new_variant = if new_role.is_single_cardinality() {
                0
            } else {
                draft_variant()
            };
            // 変更なしならそのまま閉じる。
            if new_role == animation.role && new_variant == animation.variant {
                editing.set(false);
                error.set(None);
                return;
            }
            let mut updated = animation.clone();
            updated.role = new_role;
            updated.variant = new_variant;
            // Custom 以外には export_number を持たせない (property panel と同じ正規化)。
            if new_role != Role::Custom {
                updated.export_number = None;
            }
            match repo.update_animation(&character_name, &updated) {
                Ok(()) => {
                    refresh.bump();
                    editing.set(false);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };

    let on_cancel = move |_| {
        editing.set(false);
        error.set(None);
    };

    let single = draft_role().is_single_cardinality();

    rsx! {
        select {
            class: "select select-bordered select-xs w-36",
            value: "{draft_role().yaml_value()}",
            onchange: on_role,
            for r in Role::all().iter().copied() {
                option { value: r.yaml_value(), selected: r == draft_role(), "{r.display_label()}" }
            }
        }
        // variant は multi-cardinality role でのみ意味を持つので、その時だけ出す。
        if !single {
            input {
                r#type: "number",
                class: "input input-bordered input-xs w-14",
                min: "0",
                value: "{draft_variant()}",
                title: "multi-cardinality role の slot 番号",
                onchange: on_variant,
            }
        }
        button {
            r#type: "button",
            class: "btn btn-primary btn-xs",
            onclick: on_save,
            "Save"
        }
        button {
            r#type: "button",
            class: "btn btn-ghost btn-xs",
            onclick: on_cancel,
            "Cancel"
        }
        if let Some(message) = error() {
            p { class: "text-error text-xs w-full", "{message}" }
        }
    }
}
