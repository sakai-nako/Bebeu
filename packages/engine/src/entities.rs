//! Engine 側のドメインエンティティ (旧 Go `internal/entities/*` に対応)。
//!
//! editor-desktop も同名スライス (entities/{character,level,project}) を持つが、
//! 両者で独立して進め、共通化できる shape が見えた段階で改めて crate 切り出しを検討する。
//! engine 側は Bevy `Component` / `Resource` への自然な配線を意識した型に育てていく。
pub mod character;
pub mod level;
pub mod project;
