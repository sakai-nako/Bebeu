use std::sync::Arc;

use dioxus::prelude::*;

use crate::entities::character::{Character, CharacterRepository, use_characters_refresh};
use crate::widgets::character::{
    AnimationEditor, CharacterDetail, SoundGroupEditor, SpriteGroupEditor,
};

#[component]
pub fn CharactersIndex() -> Element {
    rsx! {
        div { class: "h-full flex items-center justify-center text-base-content/50",
            "サイドバーから Character を選択してください。"
        }
    }
}

#[component]
pub fn CharacterDetailPage(name: ReadSignal<String>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let refresh = use_characters_refresh();
    let mut character = use_signal(|| None::<Character>);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        // 編集後 (refresh.bump()) にも再フェッチさせるため subscribe する
        let _ = refresh.subscribe();
        match repo.get(&name()) {
            Ok(found) => {
                character.set(found);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    });

    rsx! {
        if let Some(message) = error() {
            div { role: "alert", class: "alert alert-error",
                span { "{message}" }
            }
        }

        if let Some(c) = character() {
            CharacterDetail { character: c }
        } else if error().is_none() {
            div { class: "loading loading-spinner" }
        }
    }
}

#[component]
pub fn AnimationEditorPage(name: ReadSignal<String>, anim: ReadSignal<String>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let refresh = use_characters_refresh();
    let mut character = use_signal(|| None::<Character>);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        let _ = refresh.subscribe();
        match repo.get(&name()) {
            Ok(found) => {
                character.set(found);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    });

    rsx! {
        if let Some(message) = error() {
            div { role: "alert", class: "alert alert-error",
                span { "{message}" }
            }
        }

        if let Some(c) = character() {
            if let Some(animation) = c.animations.iter().find(|a| a.name == anim()).cloned() {
                AnimationEditor { character: c.clone(), animation }
            } else {
                div { class: "text-base-content/60 italic",
                    "Animation '{anim}' が '{name}' に見つかりません。"
                }
            }
        } else if error().is_none() {
            div { class: "loading loading-spinner" }
        }
    }
}

#[component]
pub fn SpriteGroupEditorPage(name: ReadSignal<String>, group: ReadSignal<String>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let refresh = use_characters_refresh();
    let mut character = use_signal(|| None::<Character>);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        let _ = refresh.subscribe();
        match repo.get(&name()) {
            Ok(found) => {
                character.set(found);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    });

    rsx! {
        if let Some(message) = error() {
            div { role: "alert", class: "alert alert-error",
                span { "{message}" }
            }
        }

        if let Some(c) = character() {
            if let Some(sg) = c.sprite_groups.iter().find(|sg| sg.name == group()).cloned() {
                SpriteGroupEditor { character: c.clone(), sprite_group: sg }
            } else {
                div { class: "text-base-content/60 italic",
                    "Sprite Group '{group}' が '{name}' に見つかりません。"
                }
            }
        } else if error().is_none() {
            div { class: "loading loading-spinner" }
        }
    }
}

#[component]
pub fn SoundGroupEditorPage(name: ReadSignal<String>, group: ReadSignal<String>) -> Element {
    let repo = use_context::<Arc<dyn CharacterRepository>>();
    let refresh = use_characters_refresh();
    let mut character = use_signal(|| None::<Character>);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        let _ = refresh.subscribe();
        match repo.get(&name()) {
            Ok(found) => {
                character.set(found);
                error.set(None);
            }
            Err(e) => error.set(Some(e.to_string())),
        }
    });

    rsx! {
        if let Some(message) = error() {
            div { role: "alert", class: "alert alert-error",
                span { "{message}" }
            }
        }

        if let Some(c) = character() {
            if let Some(sg) = c.sound_groups.iter().find(|sg| sg.name == group()).cloned() {
                SoundGroupEditor { character: c.clone(), sound_group: sg }
            } else {
                div { class: "text-base-content/60 italic",
                    "Sound Group '{group}' が '{name}' に見つかりません。"
                }
            }
        } else if error().is_none() {
            div { class: "loading loading-spinner" }
        }
    }
}
