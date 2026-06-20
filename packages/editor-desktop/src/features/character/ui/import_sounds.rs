use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{CharacterRepository, Sound, SoundGroup};
use crate::shared::{UseHistory, use_toast};

/// SoundGroupEditor で WAV ファイルを SoundGroup に取り込むボタン。
///
/// ## アトミック性
///
/// - WAV は `import_sound_file` で disk にコピーするが、その basename を
///   `pending_imports` に積んでおく
/// - yml の書き込みは Editor の Save まで遅延（`SoundGroupEditorActions` が
///   pending_imports をクリアして commit する）
/// - Cancel / unmount 時は `SoundGroupEditor` の `use_drop` が pending_imports を
///   `delete_sound_file` で全削除する → disk が元の状態に戻る
///
/// ## その他
///
/// - 単一 WAV を選ぶ前提の simple フロー (複数 import は将来)
/// - 既存 Sound の path と同名 basename はエラーで弾く
/// - index は draft の既存 sounds の最大値 + 1 から自動採番
#[component]
pub fn ImportSoundsButton(
    character_name: String,
    sound_group_name: String,
    mut draft: Signal<SoundGroup>,
    mut history: UseHistory<SoundGroup>,
    mut pending_imports: Signal<Vec<String>>,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let mut toast = use_toast();

    let on_pick = move |_| {
        let Some(file) = rfd::FileDialog::new()
            .add_filter("WAV", &["wav"])
            .set_title("WAV ファイルを選択")
            .pick_file()
        else {
            return;
        };

        let Some(basename_preview) = file.file_name().and_then(|n| n.to_str()).map(String::from)
        else {
            toast.error("ファイル名を取得できませんでした");
            return;
        };

        // 既存 path との重複は弾く (上書きはしない)。Cancel rollback で巻き添えになるため。
        let snapshot = draft.peek().clone();
        if snapshot.sounds.iter().any(|s| s.path == basename_preview) {
            toast.error(format!(
                "'{basename_preview}' は既に SoundGroup に含まれています"
            ));
            return;
        }

        match repo.import_sound_file(&character_name, &sound_group_name, &file) {
            Ok(basename) => {
                let next_index = snapshot
                    .sounds
                    .iter()
                    .map(|s| s.index)
                    .max()
                    .map_or(0, |m| m + 1);

                history.record();
                let mut updated = draft();
                updated.sounds.push(Sound {
                    index: next_index,
                    path: basename.clone(),
                    volume: 1.0,
                    weight: 0.0,
                });
                draft.set(updated);

                let mut p = pending_imports();
                p.push(basename.clone());
                pending_imports.set(p);

                toast.success(format!("{basename} を取り込みました"));
            }
            Err(e) => toast.error(e.to_string()),
        }
    };

    rsx! {
        button { class: "btn btn-primary btn-sm", onclick: on_pick, "+ Import WAV" }
    }
}
