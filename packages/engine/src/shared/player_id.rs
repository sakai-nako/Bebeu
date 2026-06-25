//! Player 識別子 (ADR-0030)。
//!
//! Local co-op を見越し、HUD config (`entities/project`) と ECS の Player marker
//! (`features/character`) の両方から参照する横断データ型として `shared` 層に置く。
//! 4 人までの理由は beat-em-up local co-op の実用上限。
use serde::{Deserialize, Serialize};

/// Player の論理 id。YAML key は `p1` / `p2` / `p3` / `p4`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerId {
    #[default]
    P1,
    P2,
    P3,
    P4,
}
