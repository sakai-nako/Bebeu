use dioxus::prelude::*;

use crate::entities::character::{Animation, CharacterPhysics, Role, TerminatorKind};
use crate::shared::UseHistory;

/// Animation の Role + Variant + (Custom 時の) Export Number を編集するセクション。
/// Role が single-cardinality (Idle/Walk/Block/Custom) のときは variant 入力を disabled にする。
/// Custom role のときだけ Export Number 入力欄を出す (ikemen export 用の独自 Action 番号)。
///
/// Phase 6 で「終了条件ステータス行」を追加。選択中の Role と Animation の is_loop、Character の
/// Physics 値 (lie_down_duration_ms / rise_duration_ms) を組み合わせて、エンジン側でどう終了
/// 判定されるかを文章化する。Signal で全要素にリアクティブ追随する。
#[component]
pub(super) fn AnimationRoleSection(
    mut draft: Signal<Animation>,
    mut history: UseHistory<Animation>,
    physics: CharacterPhysics,
) -> Element {
    let (role, variant, export_number, is_loop, anim_total_ticks) = {
        let snap = draft.read();
        let ticks: u32 = snap.frames.iter().map(|f| f.ticks).sum();
        (
            snap.role,
            snap.variant,
            snap.export_number,
            snap.is_loop,
            ticks,
        )
    };
    // physics.lie_down_duration_ms / rise_duration_ms との比較は ms 単位で出すため、
    // ticks を 60Hz 想定で ms 換算してから渡す (ticks * 1000 / 60)。
    let anim_duration_ms =
        u32::try_from(u64::from(anim_total_ticks) * 1000 / 60).unwrap_or(u32::MAX);
    let terminator_text = build_terminator_text(role, is_loop, anim_duration_ms, &physics);
    let single = role.is_single_cardinality();
    let export_value = export_number.map(|n| n.to_string()).unwrap_or_default();

    let on_role = move |evt: Event<FormData>| {
        let v = evt.value();
        let Some(new_role) = Role::from_yaml_value(&v) else {
            return;
        };
        let mut updated = draft();
        if updated.role == new_role {
            return;
        }
        history.record();
        updated.role = new_role;
        // single-cardinality に切り替えたときは variant=0 に正規化する。
        if new_role.is_single_cardinality() {
            updated.variant = 0;
        }
        // Custom 以外には export_number を持たせない。
        if new_role != Role::Custom {
            updated.export_number = None;
        }
        draft.set(updated);
    };

    let on_variant = move |evt: Event<FormData>| {
        let Ok(v) = evt.value().trim().parse::<u32>() else {
            return;
        };
        let mut updated = draft();
        if updated.variant == v {
            return;
        }
        history.record();
        updated.variant = v;
        draft.set(updated);
    };

    let on_export_number = move |evt: Event<FormData>| {
        let raw = evt.value();
        let new_value = if raw.trim().is_empty() {
            None
        } else {
            match raw.trim().parse::<u32>() {
                Ok(n) => Some(n),
                Err(_) => return,
            }
        };
        let mut updated = draft();
        if updated.export_number == new_value {
            return;
        }
        history.record();
        updated.export_number = new_value;
        draft.set(updated);
    };

    rsx! {
        div { class: "space-y-2",
            h3 { class: "font-semibold", "Role" }
            div { class: "grid grid-cols-[auto_1fr] gap-x-2 gap-y-2 items-center",
                label { class: "text-xs", "Role" }
                select {
                    class: "select select-bordered select-sm w-full",
                    value: "{role.yaml_value()}",
                    onchange: on_role,
                    for r in Role::all().iter().copied() {
                        option { value: r.yaml_value(), selected: r == role, "{r.selector_label()}" }
                    }
                }
                label { class: "text-xs", "Variant" }
                input {
                    r#type: "number",
                    class: "input input-bordered input-sm w-24",
                    min: "0",
                    value: "{variant}",
                    disabled: single,
                    onchange: on_variant,
                }
                if role == Role::Custom {
                    label {
                        class: "text-xs",
                        title: "ikemen export 時に独自 CNS state controller (ChangeAnim 等) から参照する独自 Action 番号。空のままなら ikemen に出力しない",
                        "Export Number"
                    }
                    input {
                        r#type: "number",
                        class: "input input-bordered input-sm w-24",
                        min: "0",
                        value: "{export_value}",
                        onchange: on_export_number,
                    }
                }
            }
            // Phase 6: 終了条件ステータス行。Role / is_loop / Physics の組み合わせから
            // エンジン側でどう終了判定されるかを 1 行で表示する。Generic Role (Idle/Walk 等) は
            // 終了条件が呼び出し側依存なので表示しない。
            if !matches!(role.terminator_kind(), TerminatorKind::Generic) {
                p { class: "text-xs text-base-content/70 italic", "{terminator_text}" }
            }
        }
    }
}

// Role 別の終了条件説明を組み立てる。is_loop / Animation の総長 / Physics の値も加味する。
//
// Generic な Role (Idle/Walk/Attack/Hit/Block/Jump/Custom) は呼び出し側依存なので、ここでは
// 「終了条件は呼び出し側で決まる」だけ返す (UI 側で出さない判断は呼び出し側がする)。
fn build_terminator_text(
    role: Role,
    is_loop: bool,
    anim_duration_ms: u32,
    physics: &CharacterPhysics,
) -> String {
    match role {
        // 物理駆動: VelY 符号 / X/Z 摩擦で次へ進む。Animation は is_loop=true 推奨 (装飾扱い)。
        // Back 系 / DeadBack 系 (背後被弾) も物理ステージは正面版と同一なので同じ文言を共有する。
        Role::KnockbackUp
        | Role::BackKnockbackUp
        | Role::DeadKnockbackUp
        | Role::DeadBackKnockbackUp => {
            "終了条件: 物理 (VelY ≤ 0 で KnockbackDown へ)。is_loop は推奨 true (物理が終了判定)"
                .to_string()
        }
        Role::KnockbackDown
        | Role::BackKnockbackDown
        | Role::DeadKnockbackDown
        | Role::DeadBackKnockbackDown => {
            "終了条件: 物理 (着地で BounceUp or Slide へ)。is_loop は推奨 true (物理が終了判定)"
                .to_string()
        }
        Role::BounceUp | Role::BackBounceUp | Role::DeadBounceUp | Role::DeadBackBounceUp => {
            "終了条件: 物理 (VelY ≤ 0 で BounceDown へ)。is_loop は推奨 true (物理が終了判定)"
                .to_string()
        }
        Role::BounceDown
        | Role::BackBounceDown
        | Role::DeadBounceDown
        | Role::DeadBackBounceDown => {
            "終了条件: 物理 (着地で 次 Bounce or Slide へ)。is_loop は推奨 true (物理が終了判定)"
                .to_string()
        }
        Role::Slide | Role::BackSlide | Role::DeadSlide | Role::DeadBackSlide => {
            "終了条件: 物理 (VelX/VelZ ≈ 0 で LieDown へ)。is_loop は推奨 true (物理が終了判定)"
                .to_string()
        }
        Role::LieDown | Role::BackLieDown => {
            if is_loop {
                format!(
                    "終了条件: lie_down_duration_ms (現在: {}ms) で Rise へ",
                    physics.lie_down_duration_ms
                )
            } else if anim_duration_ms > 0 {
                format!("終了条件: Animation 終端 ({anim_duration_ms}ms) で Rise へ")
            } else {
                format!(
                    "終了条件: Animation 未登録なので lie_down_duration_ms (現在: {}ms) で Rise へ",
                    physics.lie_down_duration_ms
                )
            }
        }
        Role::Rise | Role::BackRise => {
            if is_loop {
                format!(
                    "終了条件: rise_duration_ms (現在: {}ms) で Idle へ",
                    physics.rise_duration_ms
                )
            } else if anim_duration_ms > 0 {
                format!("終了条件: Animation 終端 ({anim_duration_ms}ms) で Idle へ")
            } else {
                format!(
                    "終了条件: Animation 未登録なので rise_duration_ms (現在: {}ms) で Idle へ",
                    physics.rise_duration_ms
                )
            }
        }
        Role::DeadLieDown | Role::DeadBackLieDown => {
            "終了条件: Animation 末尾で永続停止 (Rise には進まない)".to_string()
        }
        Role::DownHit => {
            if is_loop {
                format!(
                    "終了条件: lie_down_duration_ms (現在: {}ms) で LieDown へ戻る",
                    physics.lie_down_duration_ms
                )
            } else if anim_duration_ms > 0 {
                format!("終了条件: Animation 終端 ({anim_duration_ms}ms) で LieDown へ戻る")
            } else {
                format!(
                    "終了条件: Animation 未登録なので lie_down_duration_ms (現在: {}ms) で LieDown へ戻る",
                    physics.lie_down_duration_ms
                )
            }
        }
        // Generic (Idle/Walk/Attack/Hit/Block/Jump/DownAttack/Custom)。呼び出し側で表示を抑止する。
        Role::Idle
        | Role::Walk
        | Role::Attack
        | Role::Hit
        | Role::Block
        | Role::Jump
        | Role::DownAttack
        | Role::Custom => "終了条件: 呼び出し側で決定".to_string(),
    }
}

// Role <-> YAML 表現は `Role::yaml_value` / `Role::from_yaml_value` (role.rs) に集約してある。
// 旧 `dead` の DeadLieDown 読み替えもそこで対応。
