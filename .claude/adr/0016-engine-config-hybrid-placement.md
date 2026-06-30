# ADR-0016: engine の設定値はハイブリッド配置（engine config + workspace data）

## Status

Accepted (2026-06-28 追記: `bebeu-engine.yml` の `window: {width, height}` セクションを
ADR-0041 で Bevy App Settings に移管したため、本 ADR の対象から除外。
`bebeu-engine.yml` は `workspace_dir` のみを持つ最小 config に縮小された)

> 注: 本リポジトリ (`Bebeu`) では engine は Rust + Bevy で実装する。本 ADR の
> 「engine 起動時にエンジン固有の config を読みつつ、editor 編集の workspace data も読む」
> というハイブリッド配置の方針は engine 実装言語によらず有効。原文中の `Go` / `ebitengine`
> 由来の表現はそのままだが、Rust/Bevy 構成にも同じ規約が適用される。

## Context

engine はゲームを動かすために、editor が編集する `workspace/data/` だけでは賄えない設定値が必要になる:

1. **キャラクター固有のゲームパラメータ**: 移動速度、ジャンプ初速、state（idle / walk / attack / hit / dead）→ アニメ番号のマッピング、各 state の遷移条件
2. **入力マッピング**: キーボード／パッド → 抽象 Action のバインド
3. **ゲーム全体設定**: 論理解像度、デフォルトステージ、デフォルトキャラ
4. **ステージ定義**: 背景画像、地面ライン、敵スポーン位置

editor は現状これらを編集 UI に持っていない。`workspace/data/characters/{name}.yml` には `name` / `thumbnail_path` / `hp` しか書かれていない。

選択肢:

1. すべて engine 専用 `packages/engine/config/` に置く
2. すべて `workspace/data/` 側に拡張する（editor の Rust struct も拡張）
3. ハイブリッド（一部は engine config、一部は data）

要件:

- editor 側のスキーマ変更を伴う作業は副作用が大きい（既存 yml の互換性、editor の serialize round-trip）。Stage 1 の MVP では editor は触らずに済むようにしたい
- 一方、「キャラクター固有のゲームパラメータ」は将来的に editor で編集したい（state machine UI を editor に乗せる構想）
- 入力マッピングやゲーム全体設定はキャラ固有ではないので、editor で編集する意義が薄い

## Decision

**ハイブリッド方針** を採用する。

| 設定値 | 当面の置き場（Stage 1〜3） | 将来の置き場（Stage 4 以降） |
|---|---|---|
| キャラ固有ゲームパラメータ（move_speed, state→anim マッピング等） | `packages/engine/config/characters/{name}.yml` | `workspace/data/characters/{name}.yml` に migrate |
| 入力マッピング | `packages/engine/config/input.yml` | engine 側のまま |
| ゲーム全体設定（解像度等） | `packages/engine/config/game.yml` | engine 側のまま |
| ステージ定義（背景、地面ライン等） | `packages/engine/config/stages.yml` | engine 側のまま（または `workspace/data/stages/` 新設も将来選択肢） |

### Stage 1 段階での妥協

Stage 1 では engine config の YAML loader をまだ書かず、`internal/app/bootstrap.go` で **hardcode 値**を使う（`defaultMoveSpeed = 120` 等）。`packages/engine/config/*.yml` は将来の loader 実装時に用意する。Stage 2 か 3 で loader を実装して hardcode を引退させる。

### Stage 4 での migrate（キャラ固有のみ）

Stage 4（state machine 完備）に到達した時点で:

1. editor の `entities/character/model.rs::Character` に `move_speed: Option<f32>` `state_machine: Option<HashMap<String, StateMapping>>` 等を `#[serde(default)]` 付きで追加
2. engine の `internal/entities/character/model.go::Character` に同名フィールドを追加
3. engine の loader が `workspace/data/characters/{name}.yml` の同フィールドを優先し、無ければ `packages/engine/config/characters/{name}.yml` のフォールバックを読む
4. データを移行したら `packages/engine/config/characters/` の該当ファイルを削除

editor 側の UI は段階的に対応（最初は YAML を直接編集、後で State Machine UI を作る）。

## Alternatives Considered

- **すべて engine config に閉じる（方針 1）**: editor を一切変更せずに済むが、ゲームデザイナーが「キャラのゲーム挙動」を編集する手段が「engine 側 yml を直接書く」しかなくなる。editor がデータ作成の中心であるという思想に逆行する。

- **すべて workspace data に押し込む（方針 2）**: 美しいが Stage 1 から editor のスキーマ変更が必要で、editor 側の serialize テスト・データ互換性検証・UI 表示（無視するフィールドの扱い）の作業が膨らむ。Stage 1 完成までの距離が伸びる。

- **engine 起動時に editor を呼び出して設定編集**: プロセス起動コストが高い。editor 起動 → 編集 → 保存 → engine 再起動、というフローは現状の monorepo 構成で十分カバーできるので新規仕組み不要。

- **`workspace/.local-gw/` 配下に第 3 層を足して engine 専用 yml を置く**: workspace が editor / engine 双方の権威データという立場が崩れる。engine 専用設定は engine リポジトリの一部として配布したほうが整合性がある（バージョン管理しやすい）。

## Consequences

**得られたもの**

- Stage 1〜3 の間 editor を一切変更せずに engine 機能を増やせる。editor / engine それぞれの PR が独立に進められる
- engine 開発初期に「engine 固有の試行錯誤」を engine config 内で完結できる。早期に決めすぎる必要がない
- Stage 4 で「キャラ固有ゲームパラメータを editor 編集対象にする」明確な移行点を設定できる
- 入力マッピング・ゲーム全体設定は engine 側に閉じるので、ゲームデザイナーが触る対象から除外できる（運用上のスコープが明確）

**支払うコスト**

- `workspace/data/characters/{name}.yml` と `packages/engine/config/characters/{name}.yml` の **2 ファイルが同一キャラを参照** する状態が一時的に発生する（Stage 1〜3）。どちらに何が書かれているか把握する必要がある
- migrate のタイミング（Stage 4 着手時）を逃すと engine config が残り続けて二重管理が長期化する。ADR にタイミングを明記してリマインダ化する
- engine config の loader は Stage 1 では未実装（hardcode 値で代用）。Stage 2 か 3 で loader を書くまでの間、engine config の `*.yml` は「読まれない placeholder」として存在する状態になる

**今後の拡張余地**

- 入力マッピングはキャラ固有ではなくユーザー固有（プレイヤーごとのキーバインド）の可能性もある。その場合は editor の `preferences.yml`（→ ADR-0012）と同じ場所に engine 用入力プロファイルを置く拡張ができる
- ステージ定義は将来 editor で編集対象になる可能性がある。その場合は `workspace/data/stages/` 配下に移し、editor 側に Stage 集約 slice を新設する

## 関連

- ADR-0012: Two-tier configuration files（`local-gw.yml` の役割を engine も継承）
<!-- 旧 ADR-0016 (Go + ebitengine ランタイム導入) はリポジトリ再構築時 (Rust/Bevy 移行) に
     除外され、本 ADR の前提も「Rust/Bevy で実装される engine」に置き換わっている。 -->
- `packages/engine/README.md`: engine の config レイアウト
- `packages/engine/internal/app/bootstrap.go`: Stage 1 の hardcode 値
