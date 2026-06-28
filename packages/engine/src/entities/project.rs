//! Project 集約 (FSD: Entity slice)。
mod api;
mod model;
pub use model::{
    EnemyHpBarConfig, EnemyOverheadHpBarConfig, EnemyTarget, FillDirection, GaugeStep, HexColor,
    Hud, HudAnchor, HudElement, HudElementAnchor, HudFrame, HudOffset, HudSize, IconShakeConfig,
    IconShakeParams, OverheadVerticalAnchor, PlayerHpBarConfig, PlayerHpRingConfig,
    PlayerIconConfig, Project, Resolution, RingDirection,
};
