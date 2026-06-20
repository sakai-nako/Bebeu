use std::collections::HashSet;
use std::sync::Arc;

use dioxus::prelude::*;

use super::folder_image_picker::list_sorted_image_files;
use crate::entities::character::{CharacterRepository, Sprite, SpriteGroup};
use crate::shared::{SpriteDiskOps, UseHistory, use_image_cache_buster, use_toast};

/// SpriteGroupEditor で SpriteGroup にフォルダ単位で画像を取り込むボタン。
///
/// ## アトミック性
///
/// - 画像は `import_sprite_image` で disk にコピーするが、その basename を
///   `disk_ops.pending_imports` に積んでおく
/// - yml の書き込みは Editor の Save まで遅延（`SpriteGroupEditorActions` が
///   pending_imports をクリアして commit する）
/// - Cancel / unmount 時は `SpriteGroupEditor` の `use_drop` が pending_imports を
///   `delete_sprite_image` で全削除する → disk が元の状態に戻る
///
/// ## その他
///
/// - フォルダを選び、その直下の画像ファイルを basename 昇順で取り込む（再帰しない）
/// - 既存 Sprite の path と重複する basename は **スキップ**（上書きはしない）
/// - 重複以外の理由で全件 0 件になった場合はエラー扱いで通知
/// - index は draft の既存 sprites の最大値 + 1 から自動採番
/// - import 失敗時は import 済み画像を delete_sprite_image でロールバック
/// - import 成功時は新 sprite を自動選択する（ユーザーがすぐ pivot 編集に入れるよう）
#[component]
pub fn ImportSpritesButton(
    character_name: String,
    sprite_group_name: String,
    mut draft: Signal<SpriteGroup>,
    mut history: UseHistory<SpriteGroup>,
    mut selected_sprite_index: Signal<usize>,
    mut disk_ops: Signal<SpriteDiskOps>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    // 「同名 SpriteGroup を消して作り直し → 同名 basename を import」のような流れで
    // webview の HTTP キャッシュが古い画像を返すケースを避けるため、import 成功時に bump する。
    let cache_buster = use_image_cache_buster();
    let mut toast = use_toast();

    let on_pick = move |_| {
        let Some(folder) = rfd::FileDialog::new()
            .set_title("Sprite 画像のフォルダを選択")
            .pick_folder()
        else {
            return;
        };

        let candidates = match list_sorted_image_files(&folder) {
            Ok(v) => v,
            Err(e) => {
                toast.error(e.to_string());
                return;
            }
        };
        if candidates.is_empty() {
            toast.error("フォルダ内に画像ファイルが見つかりません");
            return;
        }

        // 既存 path 集合との重複は静かに skip。完全重複（全部 skip）になった場合だけエラー扱い。
        let snapshot = draft.peek().clone();
        let existing_paths: HashSet<&str> =
            snapshot.sprites.iter().map(|s| s.path.as_str()).collect();

        let mut targets: Vec<std::path::PathBuf> = Vec::with_capacity(candidates.len());
        let mut skipped = 0_usize;
        for path in candidates {
            let Some(basename) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if existing_paths.contains(basename) {
                skipped += 1;
                continue;
            }
            targets.push(path);
        }
        if targets.is_empty() {
            toast.error(format!(
                "フォルダ内の {skipped} 枚すべてが既存 Sprite と重複しているため取り込めません"
            ));
            return;
        }

        // 既存 sprites の最大 index + 1 から採番（並べ替えで配列順が変わっていても index は保持される）
        let start_index = snapshot
            .sprites
            .iter()
            .map(|s| s.index)
            .max()
            .map_or(0, |m| m + 1);

        let mut imported: Vec<String> = Vec::with_capacity(targets.len());
        let mut new_sprites: Vec<Sprite> = Vec::with_capacity(targets.len());

        let rollback = |imported: &[String]| {
            for basename in imported {
                let _ = repo.delete_sprite_image(&character_name, &sprite_group_name, basename);
            }
        };

        for (offset, path) in targets.iter().enumerate() {
            let next_index = start_index + u32::try_from(offset).unwrap_or(u32::MAX);
            // import 前の元 PNG から dimensions を読んでおく (4K 描画の explicit sizing 用)。
            // 失敗した場合 (壊れた PNG / 別形式) は None。
            let dims = crate::shared::read_png_dimensions(path).ok();
            match repo.import_sprite_image(&character_name, &sprite_group_name, path) {
                Ok(basename) => {
                    imported.push(basename.clone());
                    new_sprites.push(Sprite {
                        index: next_index,
                        path: basename,
                        pivot_point: [0, 0],
                        body_boxes: None,
                        attack_boxes: None,
                        dimensions: dims,
                    });
                }
                Err(e) => {
                    rollback(&imported);
                    toast.error(e.to_string());
                    return;
                }
            }
        }

        // 全 import 成功。disk_ops に追加し（Cancel 時の rollback 対象）、draft も更新。
        let mut ops = disk_ops();
        for basename in &imported {
            ops.add_pending_import(basename.clone());
        }
        disk_ops.set(ops);

        // 履歴に記録してから draft を更新（Undo で push 直前まで戻れる）
        history.record();
        let mut updated = draft();
        let first_new_position = updated.sprites.len();
        updated.sprites.extend(new_sprites);
        draft.set(updated);
        // 新しく取り込んだ sprite の先頭を選択（pivot 編集にすぐ入れるよう）
        selected_sprite_index.set(first_new_position);
        // 同名 basename の disk 衝突に備えて cache buster を bump（Editor 配下の画像 URL を再フェッチ）
        if let Some(mut buster) = cache_buster {
            buster.write().bump();
        }
        let imported_count = imported.len();
        if skipped > 0 {
            toast.success(format!(
                "{imported_count} 枚を取り込み、{skipped} 枚は重複のためスキップ"
            ));
        } else {
            toast.success(format!("{imported_count} 枚を取り込みました"));
        }
    };

    rsx! {
        button { class: "btn btn-primary btn-sm", onclick: on_pick, "+ Import" }
    }
}
