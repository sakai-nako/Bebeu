# テスト戦略

editor-desktop / engine いずれも [cargo-nextest](https://nexte.st/) でテストを走らせる。
justfile 経由で実行するのが既定で、`NEXTEST_NO_TESTS := "pass"` を export しているので、
テストの無いターゲットがあっても非ゼロ終了しない。

```sh
cargo install cargo-nextest --locked   # 初回のみ
```

| コマンド | 用途 |
|---|---|
| `just test [args]` | workspace 全 crate (editor + engine) |
| `just ed-d-test [args]` | editor-desktop のみ |
| `just en-test [args]` | engine のみ |
| `just verify` | fmt → clippy → build → test を順次 (PR 前) |
| `just en-test --no-capture` | `tracing` / `println!` を見たいとき |
| `just en-test entities::project` | nextest フィルタ (path substring match) |

## 何をテストするか / しないか

editor / engine で大枠の流儀は共通: 「副作用に近い境界 (Repository / ローダー)
は実 FS で round-trip、純粋ロジックは inline、UI / 描画は手で触る」。

### editor-desktop (Dioxus)

| 層 | テストする | しない |
|---|---|---|
| `shared/` | utility ロジック (純粋関数 / 状態機械) | Dioxus runtime に依存する hook の挙動 |
| `entities/{name}/model.rs` | serde の round-trip、parse、不変条件 | (UI 側の表示) |
| `entities/{name}/api.rs` | Repository trait の契約 (InMemory + Filesystem の双方) | 永続化に絡まないアクセサ |
| `features/{slice}/ui/*.rs` | feature 内の純粋ロジック (画像 list、scale 計算 等) | Dioxus コンポーネントの RSX |
| `widgets/`, `pages/`, `app/` | — | Dioxus コンポーネント全般 |

UI コンポーネントを自動テストしない判断:

- Dioxus 0.7 はコンポーネント rendering を切り出して assert する公式手段が薄い (`dioxus-ssr` で文字列を作っても event handler や Signal の挙動を再現するには別途 harness が必要)
- editor は単一ユーザーの操作面が中心で、UX 検証は `just ed-d-dev` で手で触るほうが得るものが多い
- そのぶん、**UI から呼ばれるロジックは UI と分離して `#[cfg(test)]` でテストする** ようにしている (例: `reimport_sprites_scaled.rs` の `scale_sprite`、`folder_image_picker.rs` の `list_sorted_image_files`)

新機能を入れるとき UI に直接ロジックを書かず、テスト可能な純粋関数 / 構造体に切り出すのを既定路線にする。

### engine (Bevy)

| 層 | テストする | しない |
|---|---|---|
| `shared/` | path 組み立て / 設定解釈 (純粋ロジック) | `RuntimePaths::resolve` のような env / FS を触る関数 (env 並列競合のため `serial_test` 等を入れるまで保留) |
| `entities/{name}/model.rs` | serde の Default / 不変条件、Resolution / Physics などの定数整合 | (Bevy `Component` の Spawn 挙動) |
| `entities/{name}/api.rs` | `load_from_file` の round-trip、name 上書き、empty YAML default、fail-soft 線引き (壊れ YAML はエラーで返す) | (現状なし) |
| `features/{slice}/*.rs` | Bevy `App::new() + MinimalPlugins` でのヘッドレス test (system 単位) | 描画パイプライン全体 |
| `scenes/{name}.rs` | — | Bevy 描画 / Scene 遷移は手動 / Computer Use で確認 |
| `app/entrypoint.rs` | — | Bevy `App` ブートストラップ全体 |

editor と同じく **描画系は自動テストしない**。動作確認は `just en-run --project=main` で手で
触るか、Computer Use で画面を観察する。`features/` の system は中身がまだ薄いので、headless
test の整備は system が育ってから検討する。

engine の Repository は editor のような trait 抽象 (InMemory / Filesystem 双方を契約 test) では
なく、`load_from_file` を直接呼ぶ素朴な API。テストは Filesystem (tempdir) ベースのみで十分。

## Repository / ローダーは実 FS を使う

ADR-0011 で「Filesystem YAML が一次ストレージ」と決めているので、Repository / ローダーの
テストは **mock せず `tempfile::tempdir()` で実 FS に書く**。理由:

- 永続化の振る舞い (ディレクトリ生成、子集約 yml の追従、画像ファイルの巻き添え削除回避 等) は disk 操作が本体。mock すると一番見たい振る舞いが見えなくなる
- ADR-0002 の「同期のみ」方針と整合: Repository は同期 I/O なので tempdir で十分速い (workspace 全件で 1 秒台前半)
- mock/prod 乖離リスクを避ける

dev-dep は `tempfile` のみで、両 crate で共有するためルートの `[workspace.dependencies]` に
集約している。新たに mock crate (mockall 等) は導入しない。

### Repository contract test パターン (editor 流)

trait の振る舞いは **実装非依存のシナリオ関数** にまとめ、InMemory / Filesystem の双方をその同じシナリオに通す。サンプル:

```rust
// entities/character/api/tests.rs
fn run_repository_scenarios<R: CharacterRepository>(repo: &R) -> Result<()> {
    assert!(repo.list()?.is_empty());
    let foo = sample_character("foo", 100, vec![sample_sprite_group("walk", 1)]);
    repo.create(&foo)?;
    assert_eq!(repo.get("foo")?, Some(foo.clone()));
    // 重複 create / 不在 update / 不在 delete のエラー判定...
    Ok(())
}

#[test]
fn in_memory_repository_satisfies_contract() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    run_repository_scenarios(&repo)
}

#[test]
fn filesystem_repository_satisfies_contract() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    run_repository_scenarios(&repo)
}
```

これに加えて、**実装固有の振る舞い** (Filesystem の disk layout、画像ファイル巻き添え保護、子集約 yml の削除追従 等) は `filesystem_*` 単独テストで個別に書く。

### engine のローダーは Filesystem のみ

engine 側の `Project::load_from_file` / `Character::load_from_file` は読み込みのみで、
trait 抽象も in-memory 実装も持たない。テストは tempdir に YAML を書いて読む単純な
round-trip 形式 — round-trip / name の埋まり方 / empty YAML での default / 不在ファイル /
壊れ YAML、の 5 観点を最小セットとしている。

## fail-soft テスト

ユーザー設定や一次データのロードでは、**壊れていてもアプリが起動できること** を明示テストする。書き手の意図 ("ここは緩く受ける") を後から読む人に伝える役割が大きい。

```rust
#[test]
fn filesystem_load_returns_default_when_yaml_is_broken() -> Result<()> {
    // fail-soft: 壊れた yml でも default を返してアプリは起動できる
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("preferences.yml");
    fs::write(&path, "this is :: not a valid yaml :: at all")?;
    let repo = FilesystemPreferencesRepository::from_path(path);
    assert_eq!(repo.load()?, Preferences::default());
    Ok(())
}
```

`preferences` (ユーザー設定) は default フォールバックする一方、`character` 集約 / `project`
(一次データ) は壊れた YAML をエラーとして返すのが正解。レイヤーごとに「何を fail-soft に
するか」を ADR-0012 / `entities/preference/README.md` で線引きしている。

## リグレッションテスト

バグ修正と一緒にテストを書くときは、**コメントで何のリグレッションかを残す** (commit message だけだと長期で参照しにくい):

```rust
#[test]
fn filesystem_update_preserves_sprite_image_files() -> Result<()> {
    // Character.update() で sprite_groups/{group}/sprites/*.png 等の実バイナリが
    // 巻き添えで消えないこと。リグレッションテスト。
    ...
}
```

## テストの配置

| テストの規模 | 配置 |
|---|---|
| 純粋ロジック (5 ケース以下) | 同ファイル内 `#[cfg(test)] mod tests` |
| Repository / 大きめユーティリティ | `api/tests.rs` のように `mod tests` を別ファイル化 |
| 複数ファイルにまたがる integration | (現状なし) |

slice / segment 構造を破らないために **`tests/` 直下のクレート外 integration test は使わない**。FSD の facade 経由でアクセスすることになり、Repository の implementation type (`FilesystemCharacterRepository` 等) が `pub use` されていない場合に外から触れない。 `#[cfg(test)]` 内なら crate-internal な path で直接構築できる。

## 関連 ADR / リファレンス

- ADR-0002: Synchronous-only data flow（テストも同期前提）
- ADR-0011: Filesystem YAML as primary storage（実 FS を使う根拠）
- ADR-0012: Two-tier configuration files（fail-soft の線引き）
- `.claude/docs/data-flow.md`: Repository / Signal / Refresh のレイヤ
