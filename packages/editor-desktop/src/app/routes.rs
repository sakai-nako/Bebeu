use dioxus::prelude::*;

use super::characters_layout::CharactersLayout;
use super::levels_layout::LevelsLayout;
use super::projects_layout::ProjectsLayout;
use super::root_shell::RootShell;
use crate::pages::{
    AnimationEditorPage, CharacterDetailPage, CharactersIndex, LevelDetailPage, LevelsIndex,
    ProjectDetailPage, ProjectsIndex, SoundGroupEditorPage, SpriteGroupEditorPage,
};

#[rustfmt::skip]
#[derive(Clone, Debug, PartialEq, Routable)]
pub enum Routes {
    #[layout(RootShell)]
        #[layout(CharactersLayout)]
            #[redirect("/", || Routes::CharactersIndex {})]
            #[route("/characters")]
            CharactersIndex {},

            #[route("/characters/:name")]
            CharacterDetailPage { name: String },

            #[route("/characters/:name/sprite-groups/:group")]
            SpriteGroupEditorPage { name: String, group: String },

            #[route("/characters/:name/animations/:anim")]
            AnimationEditorPage { name: String, anim: String },

            #[route("/characters/:name/sound-groups/:group")]
            SoundGroupEditorPage { name: String, group: String },
        #[end_layout]

        #[layout(LevelsLayout)]
            #[route("/levels")]
            LevelsIndex {},

            #[route("/levels/:name")]
            LevelDetailPage { name: String },
        #[end_layout]

        #[layout(ProjectsLayout)]
            #[route("/projects")]
            ProjectsIndex {},

            #[route("/projects/:name")]
            ProjectDetailPage { name: String },
}
