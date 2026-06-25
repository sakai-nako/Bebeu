# Architecture Decision Records

このディレクトリには、本リポジトリのアーキテクチャに関する重要な意思決定 (ADR) を記録します。

ADR は前身リポジトリ `local-game-workspace` (Go + ebitengine engine, Rust + Dioxus editor)
の `d7c7b06` 時点のものから、Rust Only 構成 (Bevy engine, Dioxus editor-desktop) に
持って来れるものを抽出して再採番しています。再採番時に除外した ADR と理由:

- 旧 ADR-0016 (Go + ebitengine ランタイム導入): Rust/Bevy 構成では前提が変わったため除外
- 旧 ADR-0018 (FSD の Go 写像 — facade なし): Go の package モデル前提なため除外
- 旧 ADR-0022 (debug-only feature を build tag で差し替える): Go の build tag 機構前提なため
  除外。Rust の `cfg(feature)` でどう実現するかは将来 ADR として書き直す予定。

| ADR  | Title | Status |
|------|-------|--------|
| 0001 | [Adopt Feature-Sliced Design (Rust/Dioxus port)](0001-adopt-feature-sliced-design.md) | Accepted |
| 0002 | [Synchronous-only data flow (no async/.await)](0002-synchronous-only-data-flow.md) | Accepted |
| 0003 | [Aggregate root maps to FSD slice](0003-aggregate-root-as-slice.md) | Accepted |
| 0004 | [Refresh trigger as wrapping `u64` counter](0004-refresh-trigger-wrapping-counter.md) | Accepted |
| 0005 | [WebView asset handler with 1-hour `Cache-Control`](0005-webview-asset-handler-cache.md) | Accepted |
| 0006 | [Pivot manipulation via dedicated marker only](0006-pivot-marker-exclusive-drag.md) | Accepted |
| 0007 | [Drag tracking uses `client_coordinates`](0007-drag-client-coordinates.md) | Accepted |
| 0008 | [NavigationGuard for unsaved-edit confirmation](0008-navigation-guard-for-unsaved-edits.md) | Accepted |
| 0009 | [Keyboard shortcut dispatch (Action enum + counter Signal)](0009-keyboard-shortcut-dispatch.md) | Accepted |
| 0010 | [Undo/Redo を editor session-scope の snapshot 履歴で実装する](0010-session-scope-snapshot-history.md) | Accepted |
| 0011 | [Filesystem YAML を primary storage にする](0011-filesystem-yaml-as-primary-storage.md) | Accepted |
| 0012 | [設定ファイルを「workspace pointer」と「user preferences」の二層に分ける](0012-two-tier-configuration-files.md) | Accepted |
| 0013 | [Toast の auto-dismiss を CSS animation で実装する](0013-toast-css-animation-auto-dismiss.md) | Accepted |
| 0014 | [Frame override の 3-state encoding (`Option<Vec<HitBox>>`)](0014-frame-override-three-state-encoding.md) | Accepted |
| 0015 | [画像 URL の cache busting を `?v={N}` クエリで行う](0015-image-cache-busting-url-query.md) | Accepted |
| 0016 | [engine の設定値はハイブリッド配置（engine config + workspace data）](0016-engine-config-hybrid-placement.md) | Accepted |
| 0017 | [world 軸 (X / Y=高さ / Z=奥行き) と 2.5D 投影](0017-world-axes-and-25d-projection.md) | Superseded by ADR-0023 |
| 0018 | [hit 判定を screen 空間 AABB で行う](0018-screen-space-aabb-hit-detection.md) | Refined by ADR-0021 |
| 0019 | [SE は Frame.sound + SoundGroup 重み付き Pick で発火する](0019-frame-event-se-with-soundgroup-pick.md) | Accepted |
| 0020 | [ファイル import の atomic 化を pending list + unmount rollback で行う](0020-pending-imports-unmount-rollback.md) | Accepted |
| 0021 | [hit 判定を world 3D AABB + per-box / character depth に移行する](0021-world-3d-aabb-with-per-box-depth.md) | Accepted (ADR-0018 を refine) |
| 0022 | [Level Area as one-side-parallel trapezoid list with OR composition](0022-level-area-one-side-parallel-trapezoid-or.md) | Refined by ADR-0023 |
| 0023 | [base 画像ピクセル = world (X, Z) = screen 座標の一本化](0023-image-pixel-world-screen-unification.md) | Accepted (Supersedes ADR-0017) |
| 0024 | [被弾を Knockback ゲージ + AttackBoxMeta 駆動の吹っ飛びシステムにする](0024-knockback-gauge-attackboxmeta-driven-hit.md) | Accepted |
| 0025 | [吹っ飛びフローを単一 Action 列で表し、方向・致命傷を Animation 解決層で分離する](0025-knockback-flow-single-action-animation-resolution.md) | Accepted |
| 0026 | [pixel-perfect 拡大を中間 render texture で分離する (window は yml 駆動)](0026-pixel-perfect-via-intermediate-render-target.md) | Accepted |
| 0027 | [ジャンプと空中攻撃を独立 State として導入し、Knockback と Y 軸物理を共有する](0027-jump-and-aerial-combat.md) | Accepted |
| 0028 | [Guard を gauge 1 本で表現し、GuardBreak は Knockback フローに合流させる](0028-guard-gauge-and-guard-break.md) | Accepted |
| 0029 | [HUD レイアウトは Project YAML に持つ](0029-hud-layout-in-project-yaml.md) | Accepted |

## 書き方

新しい ADR は次の番号 (`0030-` から) を使い、テンプレートは ADR-0001 を参考にしてください。
Status は Accepted / Superseded by ADR-XXXX / Refined by ADR-XXXX / Deprecated のいずれか。
