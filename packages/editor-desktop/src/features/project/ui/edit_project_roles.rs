use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::project::{Project, ProjectRepository};

/// Project の役割フィールド (players / opponents / levels) を識別する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectRole {
    Players,
    Opponents,
    Levels,
}

impl ProjectRole {
    fn label(self) -> &'static str {
        match self {
            Self::Players => "Players",
            Self::Opponents => "Opponents",
            Self::Levels => "Levels",
        }
    }
}

/// Project の players / opponents / levels に name を toggle する純関数。
///
/// 既に選択されていて checked=true、または含まれず checked=false の場合は no-op。
pub fn toggle_role(project: &mut Project, role: ProjectRole, name: &str, checked: bool) {
    let vec = match role {
        ProjectRole::Players => &mut project.players,
        ProjectRole::Opponents => &mut project.opponents,
        ProjectRole::Levels => &mut project.levels,
    };
    let pos = vec.iter().position(|n| n == name);
    match (checked, pos) {
        (true, None) => vec.push(name.to_string()),
        (false, Some(i)) => {
            vec.remove(i);
        }
        _ => {}
    }
}

/// 指定された name が Project の対応役割に含まれているか判定する純関数。
fn is_selected(project: &Project, role: ProjectRole, name: &str) -> bool {
    match role {
        ProjectRole::Players => project.players.iter().any(|n| n == name),
        ProjectRole::Opponents => project.opponents.iter().any(|n| n == name),
        ProjectRole::Levels => project.levels.iter().any(|n| n == name),
    }
}

/// 指定 Project Signal の役割を、available の name 群を checkbox 一覧として編集する UI。
///
/// available は master pool の Character / Level 名。Project にチェックされた name が
/// players / opponents / levels Vec に保持される。
#[component]
pub fn ProjectRoleSelector(
    role: ProjectRole,
    available: ReadSignal<Vec<String>>,
    project: Signal<Project>,
) -> Element {
    let repo = use_context::<Arc<dyn ProjectRepository>>();
    let error = use_signal(|| None::<String>);

    rsx! {
        fieldset { class: "fieldset",
            legend { class: "fieldset-legend", "{role.label()}" }
            if available().is_empty() {
                p { class: "text-base-content/60 italic text-sm", "選択肢がありません。" }
            } else {
                div { class: "flex flex-col gap-1",
                    for name in available() {
                        {
                            let row_name = name.clone();
                            let toggle_name = name.clone();
                            let key_name = name.clone();
                            let display_name = name.clone();
                            let repo = repo.clone();
                            let checked = is_selected(&project.read(), role, &row_name);
                            rsx! {
                                label { key: "{key_name}", class: "label cursor-pointer justify-start gap-2",
                                    input {
                                        r#type: "checkbox",
                                        class: "checkbox checkbox-sm",
                                        checked,
                                        oninput: move |evt: Event<FormData>| {
                                            commit_toggle(role, &toggle_name, evt.checked(), &repo, project, error);
                                        },
                                    }
                                    span { class: "label-text", "{display_name}" }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(message) = error() {
                p { class: "text-error text-xs mt-1", "{message}" }
            }
        }
    }
}

fn commit_toggle(
    role: ProjectRole,
    name: &str,
    checked: bool,
    repo: &Arc<dyn ProjectRepository>,
    mut project: Signal<Project>,
    mut error: Signal<Option<String>>,
) {
    let mut next = project.peek().clone();
    toggle_role(&mut next, role, name, checked);
    match repo.update(&next) {
        Ok(()) => {
            project.set(next);
            error.set(None);
        }
        Err(e) => error.set(Some(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_with(players: &[&str], opponents: &[&str], levels: &[&str]) -> Project {
        use crate::entities::project::Resolution;
        Project {
            name: "test".to_string(),
            resolution: Resolution::default(),
            players: players.iter().copied().map(String::from).collect(),
            opponents: opponents.iter().copied().map(String::from).collect(),
            levels: levels.iter().copied().map(String::from).collect(),
            ..Project::default()
        }
    }

    #[test]
    fn toggle_role_adds_when_checked_and_not_present() {
        let mut p = project_with(&[], &[], &[]);
        toggle_role(&mut p, ProjectRole::Players, "MooR_01", true);
        assert_eq!(p.players, vec!["MooR_01".to_string()]);
    }

    #[test]
    fn toggle_role_removes_when_unchecked_and_present() {
        let mut p = project_with(&["A", "B"], &[], &[]);
        toggle_role(&mut p, ProjectRole::Players, "A", false);
        assert_eq!(p.players, vec!["B".to_string()]);
    }

    #[test]
    fn toggle_role_no_op_when_already_in_desired_state() {
        let mut p = project_with(&["A"], &[], &[]);
        toggle_role(&mut p, ProjectRole::Players, "A", true);
        assert_eq!(p.players, vec!["A".to_string()]);

        toggle_role(&mut p, ProjectRole::Players, "X", false);
        assert_eq!(p.players, vec!["A".to_string()]);
    }

    #[test]
    fn toggle_role_targets_correct_field() {
        let mut p = project_with(&[], &[], &[]);
        toggle_role(&mut p, ProjectRole::Opponents, "B", true);
        toggle_role(&mut p, ProjectRole::Levels, "ct", true);
        assert!(p.players.is_empty());
        assert_eq!(p.opponents, vec!["B".to_string()]);
        assert_eq!(p.levels, vec!["ct".to_string()]);
    }

    #[test]
    fn is_selected_targets_correct_field() {
        let p = project_with(&["A"], &["B"], &["C"]);
        assert!(is_selected(&p, ProjectRole::Players, "A"));
        assert!(!is_selected(&p, ProjectRole::Players, "B"));
        assert!(is_selected(&p, ProjectRole::Opponents, "B"));
        assert!(is_selected(&p, ProjectRole::Levels, "C"));
    }
}
