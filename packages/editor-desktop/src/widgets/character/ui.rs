mod character_detail;
pub use character_detail::CharacterDetail;

mod physics_section;
pub use physics_section::PhysicsSection;

mod characters_sidebar;
pub use characters_sidebar::CharactersSidebar;

mod frame_thumbnail;
pub use frame_thumbnail::FrameThumbnail;

mod sprite_thumbnail;
pub use sprite_thumbnail::SpriteThumbnail;

// Canvas 共通部品 (helpers / DragState / EditorBoxOverlay)
mod canvas_common;

// HitBox 編集 input の共通コンポーネント
mod hitbox_inputs;

// AttackBox.meta (Damage / KnockbackDamage / HitstunExtra / Knockback Vec3) の共通入力
mod attack_meta_inputs;

// Sprite 編集 (Specialized Editor View)
mod sprite_canvas;
pub use sprite_canvas::SpriteCanvas;

mod sprite_reference;
pub use sprite_reference::{ReferenceLayer, ReferenceSection, SpriteReference};

mod sprite_editor_sidebar;
pub use sprite_editor_sidebar::SpriteEditorSidebar;

mod sprite_group_editor;
pub use sprite_group_editor::SpriteGroupEditor;

mod sprite_property_panel;
pub use sprite_property_panel::SpritePropertyPanel;

// Animation 編集 (Specialized Editor View)
mod animation_canvas;
pub use animation_canvas::AnimationCanvas;

mod canvas_visibility;
pub use canvas_visibility::{CanvasVisibility, CanvasVisibilityBar};

mod animation_editor;
pub use animation_editor::AnimationEditor;

mod animation_property_panel;
pub use animation_property_panel::AnimationPropertyPanel;

mod animation_timeline;
pub use animation_timeline::AnimationTimeline;

// SoundGroup 編集 (Specialized Editor View)
mod sound_group_editor;
pub use sound_group_editor::SoundGroupEditor;
