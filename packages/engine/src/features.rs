//! Engine features layer (FSD: Feature layer)。
//!
//! 各 slice は集約ドメインで切る (editor の `entities/character/` 等と同軸)。
//! 技術名 (animation / movement / state_machine) は slice 内のファイル分割に
//! とどめ、slice 名にはしない。
pub mod character;
