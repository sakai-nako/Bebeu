use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;

use super::folder_image_picker::list_sorted_image_files;
use crate::entities::character::{CharacterRepository, ImportOutcome, Sprite, SpriteGroup};
use crate::shared::{AttackBox, HitBox, SpriteDiskOps, UseHistory, use_image_cache_buster};

/// SpriteGroupEditor で「同名 basename の画像群を倍率指定で一括差し替え」するボタン。
///
/// ## やること
///
/// - フォルダを選び、その直下の画像を basename ベースで既存 Sprite とマッチング
/// - マッチした各 Sprite に対し、画像ファイルを `import_sprite_image_with_backup` で差し替え
/// - 各 Sprite の `pivot_point` / `body_boxes` / `attack_boxes` を倍率でスケール変換
/// - 不一致画像は無視（マッチ件数だけ通知）
///
/// ## アトミック性
///
/// - 上書き時は `{basename}.bak` がバックアップされ、`disk_ops.pending_overwrites` に積まれる
/// - Save 時 → `discard_sprite_image_backup` で .bak を消して確定
/// - Cancel / unmount 時 → `restore_sprite_image_backup` で旧画像に戻る
///
/// ## 注意
///
/// - 倍率 1.0 でも実行可能（画像差し替えのみ、座標は変わらない）
/// - 倍率は f64、入力が無効 / 0 以下なら error を表示してフォーム送信を拒否
#[component]
pub fn ReimportSpritesScaledButton(
    character_name: String,
    sprite_group_name: String,
    draft: Signal<SpriteGroup>,
    history: UseHistory<SpriteGroup>,
    selected_sprite_index: Signal<usize>,
    disk_ops: Signal<SpriteDiskOps>,
) -> Element {
    let mut show_modal = use_signal(|| false);

    rsx! {
        button {
            class: "btn btn-secondary btn-sm",
            title: "倍率を指定して同名画像群で一括再インポート",
            onclick: move |_| show_modal.set(true),
            "Reimport ×s"
        }

        if show_modal() {
            ReimportScaledModal {
                character_name: character_name.clone(),
                sprite_group_name: sprite_group_name.clone(),
                draft,
                history,
                selected_sprite_index,
                disk_ops,
                onclose: move |()| show_modal.set(false),
            }
        }
    }
}

#[derive(Debug, Clone)]
struct PreviewResult {
    /// マッチした (sprite 配列内の position, source path)。
    matches: Vec<(usize, PathBuf)>,
    /// フォルダ内に見つけた画像ファイル総数。
    total_in_folder: usize,
    /// 既存 Sprite と basename 一致しなかった画像数。
    unmatched: usize,
    /// 選んだフォルダのパス（表示用）。
    folder: PathBuf,
}

#[component]
fn ReimportScaledModal(
    character_name: String,
    sprite_group_name: String,
    mut draft: Signal<SpriteGroup>,
    mut history: UseHistory<SpriteGroup>,
    mut selected_sprite_index: Signal<usize>,
    mut disk_ops: Signal<SpriteDiskOps>,
    onclose: EventHandler<()>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    // Editor 配下の画像 URL を再フェッチさせるためのカウンタ。
    // SpriteGroupEditor で provider 済みなので Some で返るはず。
    let cache_buster = use_image_cache_buster();

    let mut scale_input = use_signal(|| "1.0".to_string());
    let mut preview = use_signal(|| None::<PreviewResult>);
    let mut error = use_signal(|| None::<String>);

    let on_pick_folder = move |_| {
        let Some(folder) = rfd::FileDialog::new()
            .set_title("再インポート元フォルダを選択")
            .pick_folder()
        else {
            return;
        };

        let candidates = match list_sorted_image_files(&folder) {
            Ok(v) => v,
            Err(e) => {
                error.set(Some(e.to_string()));
                preview.set(None);
                return;
            }
        };
        let total_in_folder = candidates.len();

        // 既存 sprites と basename 一致するものだけ拾い、(position, source) で記録。
        let snapshot = draft.peek().clone();
        let mut matches: Vec<(usize, PathBuf)> = Vec::new();
        for path in &candidates {
            let Some(basename) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if let Some(pos) = snapshot.sprites.iter().position(|s| s.path == basename) {
                matches.push((pos, path.clone()));
            }
        }
        let unmatched = total_in_folder.saturating_sub(matches.len());

        preview.set(Some(PreviewResult {
            matches,
            total_in_folder,
            unmatched,
            folder,
        }));
        error.set(None);
    };

    let on_apply = {
        let character_name = character_name.clone();
        let sprite_group_name = sprite_group_name.clone();
        move |_| {
            let Some(snapshot_preview) = preview.peek().clone() else {
                error.set(Some("先にフォルダを選択してください".into()));
                return;
            };
            if snapshot_preview.matches.is_empty() {
                error.set(Some(
                    "既存 Sprite と一致する画像がフォルダに見つかりません".into(),
                ));
                return;
            }
            let scale = match scale_input.peek().trim().parse::<f64>() {
                Ok(v) if v > 0.0 && v.is_finite() => v,
                _ => {
                    error.set(Some("倍率は 0 より大きい数値で入力してください".into()));
                    return;
                }
            };

            // 履歴記録は disk 操作の前に。Undo 時は draft が再インポート前に戻る。
            history.record();

            // 上書き発生分を巻き戻すために覚えておく
            let mut overwrote: Vec<String> = Vec::new();
            // pending_imports に積む新規分（同名差し替えなら通常起きないが、symmetric に処理）
            let mut created: Vec<String> = Vec::new();

            let mut updated = draft.peek().clone();

            for (pos, source) in &snapshot_preview.matches {
                let outcome = match repo.import_sprite_image_with_backup(
                    &character_name,
                    &sprite_group_name,
                    source,
                ) {
                    Ok(o) => o,
                    Err(e) => {
                        // ロールバック: ここまでの上書き分を restore、新規分を delete
                        for basename in &overwrote {
                            let _ = repo.restore_sprite_image_backup(
                                &character_name,
                                &sprite_group_name,
                                basename,
                            );
                        }
                        for basename in &created {
                            let _ = repo.delete_sprite_image(
                                &character_name,
                                &sprite_group_name,
                                basename,
                            );
                        }
                        error.set(Some(format!("画像取り込みに失敗: {e}")));
                        return;
                    }
                };

                let basename = outcome.basename().to_string();
                match outcome {
                    ImportOutcome::Overwrote { .. } => overwrote.push(basename.clone()),
                    ImportOutcome::Created { .. } => created.push(basename.clone()),
                }

                // 対応する Sprite の path / 座標を倍率変換
                if let Some(sprite) = updated.sprites.get_mut(*pos) {
                    *sprite = scale_sprite(sprite, scale);
                    sprite.path = basename;
                }
            }

            // disk_ops を更新
            let mut ops = disk_ops.peek().clone();
            for basename in &overwrote {
                ops.add_pending_overwrite(basename.clone());
            }
            for basename in &created {
                ops.add_pending_import(basename.clone());
            }
            disk_ops.set(ops);

            // 最初にマッチした sprite を選択
            if let Some((first_pos, _)) = snapshot_preview.matches.first() {
                selected_sprite_index.set(*first_pos);
            }
            draft.set(updated);
            // 同 basename のまま画像が disk 上で書き換わったので、cache buster を bump して
            // webview の HTTP キャッシュをバイパスし、配下の画像を再フェッチさせる。
            if let Some(mut buster) = cache_buster {
                buster.write().bump();
            }
            error.set(None);
            onclose.call(());
        }
    };

    let preview_view = preview.read().clone();
    let matched_count = preview_view.as_ref().map_or(0, |p| p.matches.len());

    rsx! {
        dialog { class: "modal modal-open",
            div { class: "modal-box",
                h3 { class: "text-lg font-bold mb-4", "倍率指定で再インポート" }

                p { class: "text-xs text-base-content/70 mb-3",
                    "選んだフォルダの画像のうち、既存 Sprite とファイル名が一致するものを差し替え、Pivot / BodyBox / AttackBox を倍率に合わせてスケールします。"
                }

                if let Some(message) = error() {
                    div { role: "alert", class: "alert alert-error mb-3",
                        span { "{message}" }
                    }
                }

                fieldset { class: "fieldset",
                    legend { class: "fieldset-legend", "倍率" }
                    input {
                        r#type: "number",
                        class: "input input-bordered w-full",
                        value: "{scale_input}",
                        step: "0.1",
                        min: "0",
                        oninput: move |e| scale_input.set(e.value()),
                    }
                }

                div { class: "mt-3",
                    button {
                        r#type: "button",
                        class: "btn btn-outline btn-sm",
                        onclick: on_pick_folder,
                        "フォルダを選択..."
                    }
                }

                if let Some(p) = preview_view {
                    div { class: "mt-3 text-sm space-y-1",
                        p { class: "font-mono text-xs break-all", "{p.folder.display()}" }
                        p {
                            "マッチ: "
                            span { class: "font-bold", "{matched_count}" }
                            " / "
                            span { "{p.total_in_folder}" }
                            if p.unmatched > 0 {
                                span { class: "text-base-content/60",
                                    " （{p.unmatched} 枚は既存 Sprite と一致しないため無視）"
                                }
                            }
                        }
                    }
                }

                div { class: "modal-action",
                    button {
                        r#type: "button",
                        class: "btn btn-ghost",
                        onclick: move |_| onclose.call(()),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-primary",
                        disabled: matched_count == 0,
                        onclick: on_apply,
                        "適用"
                    }
                }
            }
            div { class: "modal-backdrop",
                button { onclick: move |_| onclose.call(()), "close" }
            }
        }
    }
}

/// Sprite の path 以外（pivot_point / body_boxes / attack_boxes）を倍率で変換した複製を返す。
/// path はこの関数では更新しないので、呼び出し側で `sprite.path = basename` を別途行う。
/// 座標範囲は数千 px 程度で truncation の懸念はないので allow する。
#[allow(clippy::cast_possible_truncation)]
fn scale_sprite(sprite: &Sprite, scale: f64) -> Sprite {
    let scale_coord = |v: i32| (f64::from(v) * scale).round() as i32;
    let scale_body_boxes = |opt: &Option<Vec<HitBox>>| -> Option<Vec<HitBox>> {
        opt.as_ref()
            .map(|v| v.iter().map(|b| b.scaled(scale)).collect())
    };
    // AttackBox は hitbox 部分のみスケール、meta は座標非依存なのでそのまま保持する。
    let scale_attack_boxes = |opt: &Option<Vec<AttackBox>>| -> Option<Vec<AttackBox>> {
        opt.as_ref().map(|v| {
            v.iter()
                .map(|ab| AttackBox {
                    hitbox: ab.hitbox.scaled(scale),
                    meta: ab.meta,
                })
                .collect()
        })
    };
    let scaled_dims = sprite
        .dimensions
        .map(|[w, h]| [scale_dim(w, scale), scale_dim(h, scale)]);
    Sprite {
        index: sprite.index,
        path: sprite.path.clone(),
        pivot_point: [
            scale_coord(sprite.pivot_point[0]),
            scale_coord(sprite.pivot_point[1]),
        ],
        body_boxes: scale_body_boxes(&sprite.body_boxes),
        attack_boxes: scale_attack_boxes(&sprite.attack_boxes),
        // path は呼び出し側で実 PNG を再生成して差し替えるが、寸法も同じスケールで変わるので
        // 計算しておく。実 PNG 生成と数値の整合は呼び出し側の責任。
        dimensions: scaled_dims,
    }
}

/// 寸法 (u32) を倍率でスケールする。0 にならないよう最低 1 を返す。
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn scale_dim(v: u32, scale: f64) -> u32 {
    let scaled = (f64::from(v) * scale).round();
    scaled.max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite_with_boxes() -> Sprite {
        Sprite {
            index: 0,
            path: "walk_001.png".into(),
            pivot_point: [10, 20],
            body_boxes: Some(vec![HitBox::new(2, 4, 6, 8)]),
            attack_boxes: Some(vec![AttackBox::from_hitbox(HitBox::new(1, 1, 3, 3))]),
            dimensions: Some([32, 48]),
        }
    }

    #[test]
    fn scale_sprite_doubles_pivot_and_boxes() {
        let s = sprite_with_boxes();
        let scaled = scale_sprite(&s, 2.0);
        assert_eq!(scaled.pivot_point, [20, 40]);
        let body = scaled.body_boxes.as_ref().expect("body_boxes set");
        assert_eq!(body[0].top_left(), [4, 8]);
        assert_eq!(body[0].bottom_right(), [12, 16]);
        let attack = scaled.attack_boxes.as_ref().expect("attack_boxes set");
        assert_eq!(attack[0].hitbox.top_left(), [2, 2]);
        assert_eq!(attack[0].hitbox.bottom_right(), [6, 6]);
        // path は変えない（呼び出し側の責任）
        assert_eq!(scaled.path, "walk_001.png");
    }

    #[test]
    fn scale_sprite_with_one_is_identity_for_coords() {
        let s = sprite_with_boxes();
        let scaled = scale_sprite(&s, 1.0);
        assert_eq!(scaled.pivot_point, s.pivot_point);
        assert_eq!(scaled.body_boxes, s.body_boxes);
        assert_eq!(scaled.attack_boxes, s.attack_boxes);
    }

    #[test]
    fn scale_sprite_handles_none_boxes() {
        let mut s = sprite_with_boxes();
        s.body_boxes = None;
        s.attack_boxes = None;
        let scaled = scale_sprite(&s, 3.0);
        assert!(scaled.body_boxes.is_none());
        assert!(scaled.attack_boxes.is_none());
    }
}
