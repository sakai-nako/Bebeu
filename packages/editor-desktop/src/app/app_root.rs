use std::path::{Path, PathBuf};
use std::sync::Arc;

use dioxus::prelude::*;

use super::asset_handler::use_workspace_asset_handler;
use super::routes::Routes;
use crate::entities::character::{
    CharacterRepository, FilesystemCharacterRepository, use_characters_refresh_provider,
};
use crate::entities::keybinding::use_keyboard_action_provider;
use crate::entities::level::{
    FilesystemLevelRepository, LevelRepository, use_levels_refresh_provider,
};
use crate::entities::navigation_guard::use_navigation_guard_provider;
use crate::entities::preference::{
    FilesystemPreferencesRepository, Preferences, PreferencesRepository, use_preferences_provider,
};
use crate::entities::project::{
    FilesystemProjectRepository, ProjectRepository, use_projects_refresh_provider,
};
use crate::shared::{
    Config, ToastHost, apply_locale, detect_default_locale, translate, translate_args,
    use_toast_provider,
};

#[component]
pub fn AppRoot() -> Element {
    // Preferences をロードする前 (Config エラー含む) でも翻訳が効くように OS locale で初期化。
    // Preferences ロード後は AppMain の use_effect が `apply_locale` を上書きする。
    apply_locale(detect_default_locale());
    match Config::load() {
        Ok(config) => {
            let workspace_dir = config.workspace_dir().clone();
            rsx! {
                AppMain { workspace_dir }
            }
        }
        Err(e) => {
            let message = translate_args("app.startup_error_message", &[("error", &e.to_string())]);
            rsx! {
                div {
                    h1 { "{translate(\"app.startup_error_title\")}" }
                    p { "{message}" }
                }
            }
        }
    }
}

#[component]
fn AppMain(workspace_dir: PathBuf) -> Element {
    use_workspace_asset_handler(workspace_dir.clone());

    use_context_provider({
        let workspace_dir = workspace_dir.clone();
        move || {
            Arc::new(FilesystemCharacterRepository::new(workspace_dir))
                as Arc<dyn CharacterRepository>
        }
    });

    use_characters_refresh_provider();

    // Level: master pool の YAML を読み書きする Repository を context に置く。
    // `/levels` ページが list / get / create / save / rename / delete を行うので、
    // Characters と同様に refresh trigger も載せる。
    use_context_provider({
        let workspace_dir = workspace_dir.clone();
        move || Arc::new(FilesystemLevelRepository::new(&workspace_dir)) as Arc<dyn LevelRepository>
    });
    use_levels_refresh_provider();

    // Project: workspace に紐づくプロジェクト群。Repository だけを context に置く。
    // 「アクティブ Project」の概念は持たず、ProjectDetailPage が URL から個別にロードする
    // (engine 起動時の Project 解決は engine 側の `--project` flag / 対話プロンプトで完結する)。
    use_context_provider({
        let workspace_dir = workspace_dir.clone();
        move || {
            Arc::new(FilesystemProjectRepository::new(&workspace_dir)) as Arc<dyn ProjectRepository>
        }
    });
    use_projects_refresh_provider();

    // 旧形式 (workspace/data/project.yml) を検出したら warn ログを出す (auto-migration はしない)。
    warn_if_legacy_project_yml(&workspace_dir);

    // Preferences: Repository を context に置きつつ初期値を Signal に注入
    let preferences_repo: Arc<dyn PreferencesRepository> =
        match FilesystemPreferencesRepository::new() {
            Ok(repo) => Arc::new(repo),
            Err(e) => {
                tracing::warn!(
                    "Preferences ストレージの初期化に失敗: {} (InMemory にフォールバック)",
                    e
                );
                Arc::new(crate::entities::preference::InMemoryPreferencesRepository::new())
            }
        };
    let initial_prefs = preferences_repo.load().unwrap_or_else(|e| {
        tracing::warn!("Preferences のロードに失敗: {} (default を使用)", e);
        Preferences::default()
    });

    use_context_provider(move || preferences_repo.clone());
    let preferences = use_preferences_provider(initial_prefs);

    // キーボードショートカットのディスパッチャ (グローバル listener と画面側 hook 両方が読み書きする)
    use_keyboard_action_provider();

    // ナビゲーションガード (未保存編集を破棄する前に confirm を出す)
    use_navigation_guard_provider();

    // 全画面共通のトースト通知 queue (画面右下に ToastHost で描画)
    use_toast_provider();

    // テーマを Signal に追従させて data-theme 属性を更新（初期適用 + 切替時の再適用）
    use_effect(move || {
        let theme = preferences.read().theme.as_str();
        document::eval(&format!(
            "document.documentElement.setAttribute('data-theme', '{theme}')"
        ));
    });

    // locale を Signal に追従させて rust_i18n の thread-local を更新する (ADR-0042)。
    use_effect(move || {
        apply_locale(preferences.read().locale);
    });

    rsx! {
        document::Stylesheet { href: asset!("/assets/tailwind.css") }

        ErrorBoundary {
            handle_error: |errors: ErrorContext| {
                rsx! {
                    div { class: "flex items-center justify-center min-h-screen",
                        div { role: "alert", class: "alert alert-error max-w-lg",
                            span { "{translate(\"app.unexpected_error\")}" }
                            if let Some(error) = errors.error() {
                                p { class: "text-sm opacity-80", "{error}" }
                            }
                        }
                    }
                }
            },
            Router::<Routes> {}
        }
        ToastHost {}
    }
}

/// 旧形式 `workspace/data/project.yml` が残っていて新ディレクトリ `workspace/data/projects/` が
/// 空なら warn ログを出す。auto-migration はしない (ユーザー単独運用なので手動で move してもらう)。
fn warn_if_legacy_project_yml(workspace_dir: &Path) {
    let legacy_path = workspace_dir.join("data").join("project.yml");
    let projects_dir = workspace_dir.join("data").join("projects");
    if !legacy_path.exists() {
        return;
    }
    let new_has_entries = projects_dir.exists()
        && std::fs::read_dir(&projects_dir).is_ok_and(|d| d.flatten().count() > 0);
    if !new_has_entries {
        tracing::warn!(
            "旧形式の {} を検出しました。data/projects/<任意の名前>.yml に手動で移動してください。",
            legacy_path.display()
        );
    }
}
