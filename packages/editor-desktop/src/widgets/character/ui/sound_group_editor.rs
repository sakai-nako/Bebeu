//! SoundGroup の編集画面。Sound 一覧 (path / volume / weight 編集 + 削除 + プレビュー再生)
//! と WAV import を扱う。SpriteGroupEditor / AnimationEditor の "Stay-on-screen Editor"
//! パターンを踏襲しているが、Canvas や複雑な state machine は要らないので構造はかなり簡素。
//!
//! ## アトミック性
//!
//! WAV import は disk への書き込みを伴うので、Cancel / unmount 時の rollback が必要。
//! - `pending_imports: Signal<Vec<String>>` をローカル状態として持ち、import 成功で basename を push
//! - Save 成功時 (`SoundGroupEditorActions`) に空にする = commit 完了
//! - unmount 時 (`use_drop`) に残っている basename を `delete_sound_file` で disk から消す = rollback
//!
//! ## Orphan wav の防止
//!
//! Sound を draft から消したり、import 後にすぐ削除した場合でも、disk に取り残された wav は
//! Save 時 (`update_sound_group`) に sounds/ ディレクトリ全体を yml と差分突き合わせる仕組みで
//! 自動削除される。UI 側は `on_delete` で draft を更新するだけで、disk の同期は repository の
//! 責務として完結する。
//!
//! また `import_sound_file` は同名既存ファイルがあれば error を返すため、committed 済みの
//! wav を誤って上書きする経路は塞がっている（pending_imports 経由の rollback で committed
//! ファイルが消える事故を防ぐため）。
//!
//! ## Sound 行のレイアウト
//!
//! 1 つの Sound を card として縦に積む 3 ブロック構成:
//! 1. ヘッダ行: 再生ボタン + Index バッジ + path（横長伸縮）+ 削除ボタン
//! 2. メタデータ行: WAV ヘッダから読んだ "44100 Hz · Stereo · 1.23s · 16 bit"
//! 3. 入力行: Volume / Weight の number input
//!
//! Volume / Weight / 削除 が右端に追いやられて見にくかったのを、この縦積みで前面に出す。
//!
//! ## 再生プレビュー
//!
//! 各行に `<audio>` を 1 個 mount し（id でユニーク化）、再生ボタンで `document::eval` 経由で
//! `audio.volume = sound.volume; audio.currentTime = 0; audio.play()` を叩く。Dioxus の
//! reactive 経路を介さない命令型操作なので、Signal にも history にも干渉しない。

use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{Character, CharacterRepository, Sound, SoundGroup};
use crate::entities::navigation_guard::use_navigation_guard;
use crate::features::character::{ImportSoundsButton, SoundGroupEditorActions};
use crate::shared::{UseHistory, WavInfo, use_history, workspace_asset_url};

const HISTORY_CAPACITY: usize = 50;

/// 削除 / 並び替え後に `Sound.index` を配列順に再採番する。SpriteGroup の Sprite と同じ規約で、
/// engine 側の参照は number / index ベースなので順序変更があれば必ず通す。
fn reindex(sounds: &mut [Sound]) {
    for (i, s) in sounds.iter_mut().enumerate() {
        s.index = u32::try_from(i).unwrap_or(u32::MAX);
    }
}

#[component]
pub fn SoundGroupEditor(character: Character, sound_group: SoundGroup) -> Element {
    let draft = use_signal(|| sound_group.clone());
    let history = use_history(draft, HISTORY_CAPACITY);
    let baseline = use_signal(|| sound_group.clone());
    let pending_imports = use_signal(Vec::<String>::new);
    let mut guard = use_navigation_guard();
    let nav = use_navigator();

    let character_url = format!("/characters/{}", character.name);

    // unmount 時に commit されてない wav を rollback する。Save 成功時は
    // SoundGroupEditorActions が pending_imports をクリアするのでここは no-op になる。
    {
        let repo = use_context::<Arc<dyn CharacterRepository>>();
        let character_name = character.name.clone();
        let sound_group_name = sound_group.name.clone();
        let pending_imports_handle = pending_imports;
        use_drop(move || {
            let pending = pending_imports_handle.peek().clone();
            for basename in &pending {
                let _ = repo.delete_sound_file(&character_name, &sound_group_name, basename);
            }
        });
    }

    rsx! {
        div { class: "flex flex-col gap-3 h-full",
            div { class: "flex items-center justify-between flex-wrap gap-2",
                div { class: "breadcrumbs text-sm",
                    ul {
                        li { "characters" }
                        li {
                            a {
                                class: "cursor-pointer",
                                onclick: move |_| guard.try_navigate(&nav, character_url.clone()),
                                "{character.name}"
                            }
                        }
                        li { "sound-groups" }
                        li { "{sound_group.name}" }
                    }
                }
                SoundGroupEditorActions {
                    character: character.clone(),
                    original_group: sound_group.clone(),
                    draft,
                    history,
                    baseline,
                    pending_imports,
                }
            }

            h1 { class: "text-2xl font-bold", "{sound_group.name}" }

            div { class: "flex items-center gap-2",
                h2 { class: "text-lg font-semibold", "Sounds" }
                ImportSoundsButton {
                    character_name: character.name.clone(),
                    sound_group_name: sound_group.name.clone(),
                    draft,
                    history,
                    pending_imports,
                }
            }

            SoundList {
                character_name: character.name.clone(),
                sound_group_name: sound_group.name.clone(),
                draft,
                history,
            }
        }
    }
}

#[component]
fn SoundList(
    character_name: String,
    sound_group_name: String,
    draft: Signal<SoundGroup>,
    history: UseHistory<SoundGroup>,
) -> Element {
    let sounds = draft.read().sounds.clone();

    if sounds.is_empty() {
        return rsx! {
            div { class: "text-base-content/60 italic px-2",
                "Sound がありません。'+ Import WAV' から WAV ファイルを取り込むか、yaml を直接編集してください。"
            }
        };
    }

    let total = sounds.len();
    rsx! {
        div { class: "flex flex-col gap-2",
            for (i, sound) in sounds.iter().enumerate() {
                SoundRow {
                    key: "{sound.index}-{sound.path}",
                    character_name: character_name.clone(),
                    sound_group_name: sound_group_name.clone(),
                    draft,
                    history,
                    row_index: i,
                    total,
                    sound: sound.clone(),
                }
            }
        }
    }
}

#[component]
fn SoundRow(
    character_name: String,
    sound_group_name: String,
    mut draft: Signal<SoundGroup>,
    mut history: UseHistory<SoundGroup>,
    row_index: usize,
    /// 同じ SoundGroup 内の Sound 総数。↑↓ ボタンの disabled 判定に使う。
    total: usize,
    sound: Sound,
) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();

    // WAV ヘッダ情報。path / character / group が変わったときだけ再読み込み。
    let metadata: Memo<Result<WavInfo, String>> = {
        let repo = repo.clone();
        let character_name = character_name.clone();
        let sound_group_name = sound_group_name.clone();
        let basename = sound.path.clone();
        use_memo(move || {
            repo.read_sound_metadata(&character_name, &sound_group_name, &basename)
                .map_err(|e| e.to_string())
        })
    };

    let audio_url = workspace_asset_url(&format!(
        "data/characters/{}/sound-groups/{}/sounds/{}",
        character_name, sound_group_name, sound.path
    ));
    // id は SoundGroupEditor 内でユニークなら良い。row_index は ASCII 数字なので
    // JS の getElementById に直接埋め込んでも安全。
    let audio_id = format!("sound-audio-row-{row_index}");

    let on_volume = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<f32>() else {
            return;
        };
        let mut updated = draft();
        let Some(s) = updated.sounds.get_mut(row_index) else {
            return;
        };
        if (s.volume - v).abs() < f32::EPSILON {
            return;
        }
        s.volume = v;
        history.record();
        draft.set(updated);
    };

    let on_weight = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<f32>() else {
            return;
        };
        let mut updated = draft();
        let Some(s) = updated.sounds.get_mut(row_index) else {
            return;
        };
        if (s.weight - v).abs() < f32::EPSILON {
            return;
        }
        s.weight = v;
        history.record();
        draft.set(updated);
    };

    let on_delete = move |_| {
        let mut updated = draft();
        if row_index < updated.sounds.len() {
            history.record();
            updated.sounds.remove(row_index);
            reindex(&mut updated.sounds);
            draft.set(updated);
        }
    };

    let on_move_up = move |_| {
        if row_index == 0 {
            return;
        }
        let mut updated = draft();
        if row_index < updated.sounds.len() {
            history.record();
            updated.sounds.swap(row_index - 1, row_index);
            reindex(&mut updated.sounds);
            draft.set(updated);
        }
    };

    let on_move_down = move |_| {
        let mut updated = draft();
        if row_index + 1 < updated.sounds.len() {
            history.record();
            updated.sounds.swap(row_index, row_index + 1);
            reindex(&mut updated.sounds);
            draft.set(updated);
        }
    };

    let on_play = {
        let audio_id = audio_id.clone();
        // Volume の最新値は draft から都度引く（input 更新後すぐ再生しても反映される）
        move |_| {
            let volume = draft
                .peek()
                .sounds
                .get(row_index)
                .map_or(1.0, |s| s.volume.clamp(0.0, 1.0));
            // audio_id は "sound-audio-row-{n}" の形で ASCII 英数 + ハイフンのみ。
            document::eval(&format!(
                "
                (function() {{
                    const el = document.getElementById('{audio_id}');
                    if (!el) return;
                    el.volume = {volume};
                    try {{ el.currentTime = 0; }} catch (e) {{}}
                    el.play();
                }})();
                "
            ));
        }
    };

    let metadata_label = match &*metadata.read() {
        Ok(info) => info.label(),
        Err(_) => "—".to_string(),
    };

    let is_first = row_index == 0;
    let is_last = row_index + 1 >= total;

    rsx! {
        div { class: "border border-base-300 rounded-lg p-3 bg-base-100 hover:bg-base-200 transition-colors",
            // 1) ヘッダ行: 再生 / 並び替え / index / path
            div { class: "flex items-center gap-2",
                button {
                    class: "btn btn-circle btn-primary btn-sm",
                    title: "再生",
                    onclick: on_play,
                    "▶"
                }
                // 並び替え（vertical pair で一塊に見せる）
                div { class: "join join-vertical",
                    button {
                        class: "btn btn-xs btn-ghost join-item",
                        title: "上へ",
                        disabled: is_first,
                        onclick: on_move_up,
                        "▲"
                    }
                    button {
                        class: "btn btn-xs btn-ghost join-item",
                        title: "下へ",
                        disabled: is_last,
                        onclick: on_move_down,
                        "▼"
                    }
                }
                span { class: "badge badge-neutral badge-sm font-mono", "#{sound.index}" }
                span {
                    class: "font-mono text-sm truncate flex-1 min-w-0",
                    title: "{sound.path}",
                    "{sound.path}"
                }
            }

            // 2) メタデータ行
            div { class: "text-xs text-base-content/60 mt-1 ml-10 font-mono", "{metadata_label}" }

            // 3) Volume / Weight 入力行 + 削除ボタン（編集 UI と一緒に配置して動線を短く）
            div { class: "flex items-center gap-4 mt-2 ml-10",
                label { class: "flex items-center gap-2 text-sm",
                    span { class: "text-base-content/70 w-14", "Volume" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        value: "{sound.volume}",
                        step: "0.05",
                        min: "0",
                        max: "1",
                        onchange: on_volume,
                    }
                }
                label { class: "flex items-center gap-2 text-sm",
                    span { class: "text-base-content/70 w-14", "Weight" }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        value: "{sound.weight}",
                        step: "0.1",
                        min: "0",
                        onchange: on_weight,
                    }
                }
                button {
                    class: "btn btn-sm btn-ghost text-error",
                    title: "削除",
                    onclick: on_delete,
                    "✕ Delete"
                }
            }

            // 再生用の hidden audio。preload="metadata" で重い再生バッファ確保は遅延。
            audio { id: "{audio_id}", src: "{audio_url}", preload: "metadata" }
        }
    }
}
