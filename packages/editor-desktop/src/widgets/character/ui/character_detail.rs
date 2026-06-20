use dioxus::prelude::*;

use crate::entities::character::{Animation, Character, Role, SoundGroup, SpriteGroup};
use crate::features::character::{
    ChangeThumbnailButton, CreateAnimationButton, CreateSoundGroupButton, CreateSpriteGroupButton,
    DeleteAnimationButton, DeleteCharacterButton, DeleteSoundGroupButton, DeleteSpriteGroupButton,
    EditAnimationRoleInline, EditDepthInline, EditHpInline, EditSoundGroupNumberInline,
    EditSpriteGroupNumberInline, RenameAnimationButton, RenameCharacterButton,
    RenameSoundGroupButton, RenameSpriteGroupButton,
};
use crate::shared::workspace_asset_url;

use super::PhysicsSection;

#[component]
pub fn CharacterDetail(character: Character) -> Element {
    let thumbnail_url = if character.thumbnail_path.is_empty() {
        String::new()
    } else {
        workspace_asset_url(&format!(
            "data/characters/{}/{}",
            character.name, character.thumbnail_path
        ))
    };

    let sprite_groups_count = character.sprite_groups.len();
    let animations_count = character.animations.len();
    let sound_groups_count = character.sound_groups.len();

    rsx! {
        div { class: "space-y-6",
            div { class: "breadcrumbs text-sm",
                ul {
                    li { "characters" }
                    li { "{character.name}" }
                }
            }

            // Header
            div { class: "flex items-center gap-3",
                h1 { class: "text-3xl font-bold", "{character.name}" }
                RenameCharacterButton { character: character.clone() }
                DeleteCharacterButton { name: character.name.clone() }
            }

            // Top: Thumbnail + Properties
            div { class: "flex flex-wrap items-start gap-6",
                // Thumbnail
                div { class: "flex flex-col gap-2 items-start",
                    if character.thumbnail_path.is_empty() {
                        div { class: "w-64 h-64 flex items-center justify-center rounded-box border border-base-300 bg-base-200 text-base-content/60 italic",
                            "サムネイル未設定"
                        }
                    } else {
                        img {
                            src: "{thumbnail_url}",
                            alt: "{character.name} thumbnail",
                            class: "w-64 h-64 object-contain rounded-box border border-base-300 bg-base-200",
                        }
                    }
                    ChangeThumbnailButton { character: character.clone() }
                }

                // Properties
                div { class: "flex-1 min-w-64",
                    h2 { class: "text-xl font-semibold mb-3", "Properties" }
                    dl { class: "grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 items-center",
                        dt { class: "font-semibold text-base-content/70", "HP" }
                        dd {
                            EditHpInline { character: character.clone() }
                        }
                        dt {
                            class: "font-semibold text-base-content/70",
                            title: "world Z 軸 (奥行き) の厚み。HitBox.depth 未設定時のフォールバック値",
                            "Depth (Z)"
                        }
                        dd {
                            EditDepthInline { character: character.clone() }
                        }
                    }
                }
            }

            // Physics セクション (重力 / ジャンプ / Knockback / バウンス / 摩擦 / timer)。
            // 通常は閉じておく (作家が初期に触る頻度は低いため)、必要時に展開する。
            PhysicsSection { character: character.clone() }

            // Sub-aggregates: Sprite Groups + Animations + Sound Groups
            div { class: "flex flex-wrap gap-4",
                // Sprite Groups
                div { class: "max-w-md w-full",
                    div { class: "flex items-center justify-between mb-2",
                        h2 { class: "text-xl font-semibold", "Sprite Groups ({sprite_groups_count})" }
                        CreateSpriteGroupButton { character: character.clone() }
                    }
                    div { class: "collapse collapse-arrow bg-base-200",
                        input { r#type: "checkbox", checked: true }
                        div { class: "collapse-title text-sm text-base-content/70",
                            "{sprite_groups_count} 件"
                        }
                        div { class: "collapse-content",
                            if character.sprite_groups.is_empty() {
                                div { class: "text-base-content/60 italic",
                                    "Sprite Group がありません。"
                                }
                            } else {
                                ListColumnHeaders {}
                                ul { class: "list bg-base-100 rounded-box w-full",
                                    for group in character.sprite_groups.iter() {
                                        SpriteGroupRow {
                                            key: "{group.name}",
                                            character: character.clone(),
                                            sprite_group: group.clone(),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Animations
                div { class: "max-w-md w-full",
                    div { class: "flex items-center justify-between mb-2",
                        h2 { class: "text-xl font-semibold", "Animations ({animations_count})" }
                        CreateAnimationButton { character: character.clone() }
                    }
                    div { class: "collapse collapse-arrow bg-base-200",
                        input { r#type: "checkbox", checked: true }
                        div { class: "collapse-title text-sm text-base-content/70",
                            "{animations_count} 件"
                        }
                        div { class: "collapse-content",
                            if character.animations.is_empty() {
                                div { class: "text-base-content/60 italic",
                                    "Animation がありません。"
                                }
                            } else {
                                // Animation には Number 列が無いので独自のヘッダ。Name を主、Role を従に置く。
                                // 行末の ✎ / Rename / Delete と同幅の invisible スペーサーを置いて、
                                // ROLE ラベルを行の Role badge の上に揃える (右に寄りすぎないように)。
                                div { class: "flex items-center gap-2 px-3 pb-1 text-xs text-base-content/60 font-semibold uppercase",
                                    span { class: "flex-1 min-w-0", "Name" }
                                    span { "Role" }
                                    div {
                                        class: "flex items-center gap-2 invisible",
                                        aria_hidden: "true",
                                        button {
                                            r#type: "button",
                                            class: "btn btn-ghost btn-xs",
                                            "✎"
                                        }
                                        button {
                                            r#type: "button",
                                            class: "btn btn-primary btn-outline btn-sm",
                                            "Rename"
                                        }
                                        button {
                                            r#type: "button",
                                            class: "btn btn-error btn-outline btn-sm",
                                            "Delete"
                                        }
                                    }
                                }
                                ul { class: "list bg-base-100 rounded-box w-full",
                                    for animation in character.animations.iter() {
                                        AnimationRow {
                                            key: "{animation.name}",
                                            character: character.clone(),
                                            animation: animation.clone(),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Sound Groups
                div { class: "max-w-md w-full",
                    div { class: "flex items-center justify-between mb-2",
                        h2 { class: "text-xl font-semibold", "Sound Groups ({sound_groups_count})" }
                        CreateSoundGroupButton { character: character.clone() }
                    }
                    div { class: "collapse collapse-arrow bg-base-200",
                        input { r#type: "checkbox", checked: true }
                        div { class: "collapse-title text-sm text-base-content/70",
                            "{sound_groups_count} 件"
                        }
                        div { class: "collapse-content",
                            if character.sound_groups.is_empty() {
                                div { class: "text-base-content/60 italic",
                                    "Sound Group がありません。"
                                }
                            } else {
                                ListColumnHeaders {}
                                ul { class: "list bg-base-100 rounded-box w-full",
                                    for group in character.sound_groups.iter() {
                                        SoundGroupRow {
                                            key: "{group.name}",
                                            character: character.clone(),
                                            sound_group: group.clone(),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Role Map: 役割ごとに Animation がどう割り当たっているかを俯瞰する。
            // 必須 role (Idle/Walk) が未設定なら warn icon、duplicate / gap は validate_animations の結果として表示。
            RoleMapCard { character: character.clone() }
        }
    }
}

/// SpriteGroup / Animation リスト共通の列見出し行。Name と Number のラベルだけ表示。
/// 行末の "✎ / Rename / Delete" と同じ構造の invisible スペーサーを置いて、
/// Number ラベルを行の数値列の上に揃える。
#[component]
fn ListColumnHeaders() -> Element {
    rsx! {
        div { class: "flex items-center gap-2 px-3 pb-1 text-xs text-base-content/60 font-semibold uppercase",
            span { class: "flex-1 min-w-0", "Name" }
            span { "Number" }
            div {
                class: "flex items-center gap-2 invisible",
                aria_hidden: "true",
                button { r#type: "button", class: "btn btn-ghost btn-xs", "✎" }
                button {
                    r#type: "button",
                    class: "btn btn-primary btn-outline btn-sm",
                    "Rename"
                }
                button {
                    r#type: "button",
                    class: "btn btn-error btn-outline btn-sm",
                    "Delete"
                }
            }
        }
    }
}

/// SpriteGroup 1 件分のリスト行。Number inline 編集中は Rename / Delete を隠す。
#[component]
fn SpriteGroupRow(character: Character, sprite_group: SpriteGroup) -> Element {
    let editing = use_signal(|| false);
    rsx! {
        li { class: "flex items-center gap-2 px-3 py-2 hover:bg-base-200 rounded",
            Link {
                to: "/characters/{character.name}/sprite-groups/{sprite_group.name}",
                class: "font-medium truncate flex-1 min-w-0",
                "{sprite_group.name}"
            }
            EditSpriteGroupNumberInline {
                character: character.clone(),
                sprite_group: sprite_group.clone(),
                editing,
            }
            if !editing() {
                RenameSpriteGroupButton {
                    character_name: character.name.clone(),
                    sprite_group: sprite_group.clone(),
                }
                DeleteSpriteGroupButton {
                    character_name: character.name.clone(),
                    sprite_group_name: sprite_group.name.clone(),
                }
            }
        }
    }
}

/// Animation 1 件分のリスト行。Name を主に行頭へ、Role badge を従にその後ろへ置く。
/// Role badge 隣の ✎ で inline 編集に切り替え、その場で engine への semantic な紐付け
/// (Role / Variant) を変更できる。編集中は badge/✎ → select、Rename/Delete → Save/Cancel と
/// 同位置で差し替え、非編集時とのレイアウト差を最小化する。
#[component]
fn AnimationRow(character: Character, animation: Animation) -> Element {
    let mut editing = use_signal(|| false);
    rsx! {
        li { class: "flex flex-wrap items-center gap-2 px-3 py-2 hover:bg-base-200 rounded",
            // Name を主に行頭へ。Role 表示 / 編集は Name の後ろ。
            Link {
                to: "/characters/{character.name}/animations/{animation.name}",
                class: "font-medium truncate flex-1 min-w-0",
                "{animation.name}"
            }
            if editing() {
                EditAnimationRoleInline {
                    character_name: character.name.clone(),
                    animation: animation.clone(),
                    editing,
                }
            } else {
                // Role badge + 編集トリガー (✎)。badge / ✎ / Rename / Delete を li 直下の兄弟に
                // 並べることで、編集時の [select][(variant)][Save][Cancel] と位置を 1:1 で対応させる。
                RoleBadge { role: animation.role, variant: animation.variant }
                button {
                    r#type: "button",
                    class: "btn btn-ghost btn-xs",
                    onclick: move |_| editing.set(true),
                    title: "Role を編集",
                    "✎"
                }
                RenameAnimationButton {
                    character_name: character.name.clone(),
                    animation: animation.clone(),
                }
                DeleteAnimationButton {
                    character_name: character.name.clone(),
                    animation_name: animation.name.clone(),
                }
            }
        }
    }
}

/// Role badge の daisyUI クラス。RoleBadge と Role Map の role ラベルで共有し、両画面で
/// 同じ見た目に揃える。Role 数が多く category 別の色分けは Role Map で色のコントラストが
/// 強すぎたため、色分けはせず一律の塗り badge にする (Role 種別は左右の配置・ラベルで判別する)。
fn role_badge_class() -> &'static str {
    "badge badge-sm"
}

/// Role + Variant を表す badge。Multi-cardinality role では `Attack #1` のように variant を併記する。
#[component]
fn RoleBadge(role: Role, variant: u32) -> Element {
    let label = role.display_label().to_string();
    let formatted = if role.is_single_cardinality() {
        label
    } else {
        format!("{label} #{variant}")
    };
    rsx! {
        span { class: "{role_badge_class()} whitespace-nowrap", "{formatted}" }
    }
}

/// Character の全 Animation を role × variant の行列で俯瞰するカード。
/// 必須 role (Idle/Walk) が未設定なら warn、duplicate / variant gap は validate_animations の出力を流す。
#[component]
fn RoleMapCard(character: Character) -> Element {
    use crate::entities::character::{RoleViolation, Severity, validate_animations};

    let violations = validate_animations(&character.animations);
    let required = [Role::Idle, Role::Walk];
    let mut missing_required: Vec<Role> = Vec::new();
    for r in required {
        if !character.animations.iter().any(|a| a.role == r) {
            missing_required.push(r);
        }
    }

    rsx! {
        div { class: "max-w-3xl w-full",
            div { class: "flex items-center justify-between mb-2",
                h2 { class: "text-xl font-semibold", "Animation Role Map" }
            }
            div { class: "collapse collapse-arrow bg-base-200",
                input { r#type: "checkbox", checked: true }
                div { class: "collapse-title text-sm text-base-content/70",
                    "engine 各種への semantic な紐付け"
                }
                div { class: "collapse-content space-y-3",
                    for role in Role::all().iter().copied() {
                        RoleMapRow {
                            key: "{role.display_label()}",
                            role,
                            animations: character.animations.clone(),
                        }
                    }
                    if !missing_required.is_empty() {
                        div {
                            role: "alert",
                            class: "alert alert-warning text-sm",
                            for r in missing_required.iter().copied() {
                                div { "必須 role が未設定: {r.display_label()}" }
                            }
                        }
                    }
                    if !violations.is_empty() {
                        div { class: "space-y-1",
                            for v in violations.iter() {
                                RoleViolationLine { violation: v.clone() }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn RoleMapRow(role: Role, animations: Vec<Animation>) -> Element {
    let mut matched: Vec<&Animation> = animations.iter().filter(|a| a.role == role).collect();
    matched.sort_by_key(|a| a.variant);
    rsx! {
        div { class: "grid grid-cols-[12rem_1fr] gap-2 items-start text-sm",
            // role ラベルは Animations と同じ塗り badge で表示し「塗り badge = Role」を一致させる
            // (右側の outline badge = Animation 名と意味を見分けやすくする)。
            div {
                span { class: "{role_badge_class()} whitespace-nowrap", "{role.display_label()}" }
            }
            div { class: "flex flex-wrap gap-2",
                if matched.is_empty() {
                    span { class: "text-base-content/40 italic", "—" }
                } else {
                    for a in matched.iter() {
                        span { class: "badge badge-sm badge-outline whitespace-nowrap",
                            if role.is_single_cardinality() {
                                "{a.name}"
                            } else {
                                "{a.name} #{a.variant}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn RoleViolationLine(violation: crate::entities::character::RoleViolation) -> Element {
    use crate::entities::character::{RoleViolation, Severity};
    let severity = violation.severity();
    let class = match severity {
        Severity::Error => "alert alert-error text-sm",
        Severity::Warn => "alert alert-warning text-sm",
    };
    let message = match &violation {
        RoleViolation::DuplicateExportNumber {
            export_number,
            animation_names,
        } => format!(
            "Custom Animation の Export Number {} が重複: {}",
            export_number,
            animation_names.join(", ")
        ),
        RoleViolation::DuplicateSingleRole {
            role,
            animation_names,
        } => format!(
            "Role '{}' は 1 個まで。重複: {}",
            role.display_label(),
            animation_names.join(", ")
        ),
        RoleViolation::DuplicateRoleVariant {
            role,
            variant,
            animation_names,
        } => format!(
            "Role '{}' の variant {} が重複: {}",
            role.display_label(),
            variant,
            animation_names.join(", ")
        ),
        RoleViolation::VariantGap { role, missing } => format!(
            "Role '{}' の variant に飛び: {} が未設定",
            role.display_label(),
            missing
        ),
    };
    rsx! {
        div { role: "alert", class: "{class}",
            span { "{message}" }
        }
    }
}

/// SoundGroup 1 件分のリスト行。Number inline 編集中は Rename / Delete を隠す。
/// 名前 Link は SoundGroupEditorPage (`/characters/:name/sound-groups/:group`) に飛ぶ。
#[component]
fn SoundGroupRow(character: Character, sound_group: SoundGroup) -> Element {
    let editing = use_signal(|| false);
    rsx! {
        li { class: "flex items-center gap-2 px-3 py-2 hover:bg-base-200 rounded",
            Link {
                to: "/characters/{character.name}/sound-groups/{sound_group.name}",
                class: "font-medium truncate flex-1 min-w-0",
                span { "{sound_group.name}" }
            }
            EditSoundGroupNumberInline {
                character: character.clone(),
                sound_group: sound_group.clone(),
                editing,
            }
            if !editing() {
                RenameSoundGroupButton {
                    character_name: character.name.clone(),
                    sound_group: sound_group.clone(),
                }
                DeleteSoundGroupButton {
                    character_name: character.name.clone(),
                    sound_group_name: sound_group.name.clone(),
                }
            }
        }
    }
}
