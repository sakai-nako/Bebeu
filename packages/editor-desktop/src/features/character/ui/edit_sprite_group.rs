use std::sync::Arc;

use dioxus::prelude::*;

use super::{ApplyFirstSpriteButton, ApplyPreviousSpriteButton};
use crate::entities::character::{
    Character, CharacterRepository, SelectedBox, SpriteGroup, use_characters_refresh,
};
use crate::entities::keybinding::Action;
use crate::entities::navigation_guard::use_navigation_guard;
use crate::features::keybinding::use_keyboard_action;
use crate::shared::{HitBox, SpriteDiskOps, UseHistory};

const DEFAULT_BOX_SIZE: i32 = 32;

#[component]
pub fn SpriteGroupEditorActions(
    character: Character,
    original_group: SpriteGroup,
    mut draft: Signal<SpriteGroup>,
    mut history: UseHistory<SpriteGroup>,
    mut selected_sprite_index: Signal<usize>,
    mut selected_box: Signal<Option<SelectedBox>>,
    mut disk_ops: Signal<SpriteDiskOps>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut refresh = use_characters_refresh();
    let nav = use_navigator();
    let mut guard = use_navigation_guard();
    let mut error = use_signal(|| None::<String>);
    // 「最後に保存した状態」を保持。is_dirty 判定と Cancel 確認の両方で参照する。
    // 親側 (SpriteGroupEditorPage) の refresh 再フェッチで `original_group` prop が
    // 更新されるまでに 1 フレームのラグがあるため、自前 Signal でブレを防ぐ。
    let mut baseline = use_signal(|| original_group.clone());

    // is_dirty を NavigationGuard に同期し、breadcrumb / 左 rail / Cancel 等あらゆるナビ起点で
    // 確認ダイアログが出るようにする。値が変わったときだけ書き込む (set 内でガード)。
    use_effect(move || {
        let dirty = draft() != *baseline.read();
        guard.set_blocked(dirty);
    });

    // 編集画面が unmount されるときに必ず blocked を解除する (破棄経由で抜けたあとや
    // 例外的な遷移時に「離れた後もダイアログが出続ける」ことを防ぐ)
    use_drop(move || {
        guard.set_blocked(false);
    });

    // Cancel ボタンの遷移先。Detail を廃止したので、Cancel は Character ページへ戻す。
    let cancel_url = format!("/characters/{}", character.name);

    let mut on_add_box = move |kind_for_new: fn(usize) -> SelectedBox| {
        let sprite_index = selected_sprite_index();
        let mut updated = draft();
        let new_box = HitBox::new(0, 0, DEFAULT_BOX_SIZE, DEFAULT_BOX_SIZE);
        if let Some(s) = updated.sprites.get_mut(sprite_index) {
            // push_box は kind を保持して使うため、index 部はダミー（push 後に返る new_index で正しい SelectedBox を作る）
            let target_for_kind = kind_for_new(0);
            let new_index = s.push_box(target_for_kind, new_box);
            // 変更が確定した時点で履歴に積む (peek().clone() なので draft.set より前に呼ぶ)
            history.record();
            draft.set(updated);
            selected_box.set(Some(kind_for_new(new_index)));
        }
    };

    // ボタン onclick とキーボードショートカットの両方から呼べるように `use_callback` 化する。
    // `Callback` は Copy なので、複数のクロージャから自由に発火できる。
    let on_save = use_callback({
        let character = character.clone();
        let original_group = original_group.clone();
        let repo = repo.clone();
        move |()| {
            let mut current_draft = draft();
            // 保存時、空の Vec は None に変換（既存 yml 規約と整合）
            for s in &mut current_draft.sprites {
                if s.body_boxes.as_ref().is_some_and(Vec::is_empty) {
                    s.body_boxes = None;
                }
                if s.attack_boxes.as_ref().is_some_and(Vec::is_empty) {
                    s.attack_boxes = None;
                }
            }
            let mut updated = character.clone();
            if let Some(slot) = updated
                .sprite_groups
                .iter_mut()
                .find(|g| g.name == original_group.name)
            {
                *slot = current_draft.clone();
            } else {
                error.set(Some(format!(
                    "SpriteGroup '{}' が Character に見つかりません",
                    original_group.name
                )));
                return;
            }
            match repo.update(&updated) {
                Ok(()) => {
                    // 保存後は編集画面に留まる。draft / baseline 両方を正規化済みの値に揃え、
                    // 「未保存」判定が空 Vec ↔ None でブレないようにする。
                    draft.set(current_draft.clone());
                    baseline.set(current_draft);
                    // disk_ops を commit:
                    //   - pending_deletions の旧画像を実際に disk から削除
                    //   - pending_overwrites の `.bak` を削除して上書きを確定
                    //   - pending_imports は yml に登録されたので保留扱い解除（クリア）
                    //   こうすることで Editor unmount 時の rollback 対象から外れる
                    let ops = disk_ops.peek().clone();
                    for basename in &ops.pending_deletions {
                        let _ = repo.delete_sprite_image(
                            &character.name,
                            &original_group.name,
                            basename,
                        );
                    }
                    for basename in &ops.pending_overwrites {
                        let _ = repo.discard_sprite_image_backup(
                            &character.name,
                            &original_group.name,
                            basename,
                        );
                    }
                    if !ops.is_empty() {
                        disk_ops.set(SpriteDiskOps::default());
                    }
                    refresh.bump();
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    });

    // Ctrl+S (または preferences.yml で設定されたキー) で同じ保存処理を発火する
    use_keyboard_action(Action::Save, move || on_save.call(()));

    // Sprite 切り替え: 現在の index を 1 つ進める / 戻す / 端へジャンプする。
    // sprites 数 0 の場合は何もしない。Box 選択は対象 Sprite が変わるためクリアする。
    let select_sprite = use_callback(move |new_index: usize| {
        let len = draft.peek().sprites.len();
        if len == 0 {
            return;
        }
        let clamped = new_index.min(len - 1);
        if *selected_sprite_index.peek() != clamped {
            selected_sprite_index.set(clamped);
            selected_box.set(None);
        }
    });
    use_keyboard_action(Action::SelectPrevSprite, move || {
        let current = *selected_sprite_index.peek();
        if current > 0 {
            select_sprite.call(current - 1);
        }
    });
    use_keyboard_action(Action::SelectNextSprite, move || {
        let current = *selected_sprite_index.peek();
        select_sprite.call(current + 1);
    });
    use_keyboard_action(Action::SelectFirstSprite, move || {
        select_sprite.call(0);
    });
    use_keyboard_action(Action::SelectLastSprite, move || {
        let len = draft.peek().sprites.len();
        if len > 0 {
            select_sprite.call(len - 1);
        }
    });

    // 選択中 Sprite の並び替え: 隣の Sprite と swap し、選択も追従させる。
    // 端 (= 動かす方向に余地が無い) では no-op。`forward = true` で 1 つ後ろへ、
    // false で 1 つ前へ。
    let move_selected = use_callback(move |forward: bool| {
        let current = *selected_sprite_index.peek();
        let len = draft.peek().sprites.len();
        let target = if forward {
            if current + 1 >= len {
                return;
            }
            current + 1
        } else {
            if current == 0 {
                return;
            }
            current - 1
        };
        history.record();
        let mut updated = draft.peek().clone();
        updated.sprites.swap(current, target);
        draft.set(updated);
        selected_sprite_index.set(target);
    });
    use_keyboard_action(Action::MoveSpritePrev, move || move_selected.call(false));
    use_keyboard_action(Action::MoveSpriteNext, move || move_selected.call(true));

    // Pivot 移動: 選択中 Sprite の pivot_point を 1 px 単位で動かす。
    let move_pivot = use_callback(move |(dx, dy): (i32, i32)| {
        let sprite_index = *selected_sprite_index.peek();
        let mut updated = draft.peek().clone();
        let Some(s) = updated.sprites.get_mut(sprite_index) else {
            return;
        };
        s.pivot_point[0] += dx;
        s.pivot_point[1] += dy;
        history.record();
        draft.set(updated);
    });
    use_keyboard_action(Action::MovePivotUp, move || move_pivot.call((0, -1)));
    use_keyboard_action(Action::MovePivotDown, move || move_pivot.call((0, 1)));
    use_keyboard_action(Action::MovePivotLeft, move || move_pivot.call((-1, 0)));
    use_keyboard_action(Action::MovePivotRight, move || move_pivot.call((1, 0)));

    // Undo / Redo: UseHistory は Copy なので各クロージャに別コピーを渡す。
    // クロージャ内の binding が `mut` でないと `&mut self` メソッドを呼べないため
    // `let mut h = ...` でローカルに mutable 化する。
    use_keyboard_action(Action::Undo, move || {
        let mut h = history;
        h.undo();
    });
    use_keyboard_action(Action::Redo, move || {
        let mut h = history;
        h.redo();
    });

    // Cancel ボタン: NavigationGuard に詳細ページ URL を要求する。dirty なら confirm が出るし、
    // dirty でなければ即座に遷移する (breadcrumb / 左 rail と同じ経路に統一)。
    let on_cancel_clicked = {
        let cancel_url = cancel_url.clone();
        move |_| guard.try_navigate(&nav, cancel_url.clone())
    };

    let is_dirty = draft() != *baseline.read();
    let can_undo = history.can_undo();
    let can_redo = history.can_redo();

    rsx! {
        div { class: "flex items-center gap-2",
            button {
                class: "btn btn-ghost btn-sm",
                disabled: !can_undo,
                title: "元に戻す",
                onclick: move |_| {
                    let mut h = history;
                    h.undo();
                },
                "↶ Undo"
            }
            button {
                class: "btn btn-ghost btn-sm",
                disabled: !can_redo,
                title: "やり直す",
                onclick: move |_| {
                    let mut h = history;
                    h.redo();
                },
                "↷ Redo"
            }
            div { class: "divider divider-horizontal mx-0" }
            button {
                class: "btn btn-info btn-sm",
                onclick: move |_| on_add_box(SelectedBox::Body),
                "+ Body Box"
            }
            button {
                class: "btn btn-error btn-sm",
                onclick: move |_| on_add_box(SelectedBox::Attack),
                "+ Attack Box"
            }
            div { class: "divider divider-horizontal mx-0" }
            ApplyFirstSpriteButton { draft, history }
            ApplyPreviousSpriteButton { draft, history, selected_sprite_index }
            div { class: "divider divider-horizontal mx-0" }
            button { class: "btn btn-ghost btn-sm", onclick: on_cancel_clicked, "Cancel" }
            if is_dirty {
                span {
                    class: "badge badge-warning badge-sm",
                    title: "未保存の変更があります",
                    "● 未保存"
                }
            }
            button {
                class: "btn btn-primary btn-sm",
                onclick: move |_| on_save.call(()),
                "Save"
            }
        }

        if let Some(message) = error() {
            div { role: "alert", class: "alert alert-error mt-2",
                span { "{message}" }
            }
        }
    }
}
