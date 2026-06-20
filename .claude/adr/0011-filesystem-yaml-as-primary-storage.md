# ADR-0011: Filesystem YAML を primary storage にする

## Status

Accepted

## Context

editor が扱うデータ（Character / SpriteGroup / Animation）の永続化方式を決める必要がある。要件:

1. **個人ワークフロー前提**: 1 ユーザー 1 マシン、同時編集なし、トランザクション要件なし
2. **Git で履歴管理する**: workspace ディレクトリ自体が（別の）Git リポジトリで管理される運用。差分が読めて、コンフリクト解消も人間ができる形式が必須
3. **エディタを介さない手編集ができる**: 大量リネームや一括書き換えを `sed` / VSCode のマルチカーソルでやる
4. **export/import が要らない**: 「テキスト → 別フォーマット → 別ツール」の経路を作らず、テキスト 1 形式で完結する
5. **ドメイン構造に追従する**: 集約ルートが Character、子集約が SpriteGroup / Animation という DDD 的な構造を素直に表現したい
6. **テスタビリティ**: Repository 層は filesystem 抜きでもテストできる

## Decision

`workspace/data/` 配下を Single Source of Truth とし、ドメイン集約を `*.yml` として直接保存する。

### ファイルレイアウト

```
workspace/data/characters/
├── MooR_01.yml                   ← Character metadata (name / hp / thumbnail_path)
└── MooR_01/
    ├── sprite-groups/
    │   ├── walk.yml              ← SpriteGroup metadata + sprites リスト
    │   └── walk/sprites/*.png    ← 実バイナリ
    └── animations/
        └── walk.yml              ← Animation metadata + frames + layers
```

集約ルート（Character）は metadata のみ、子集約（SpriteGroup / Animation）は別ファイル。`{name}.yml` には `sprite_groups` / `animations` を `#[serde(skip)]` で書かない。詳細は `entities/character/README.md`。

### YAML パーサ

`serde_saphyr` を全 yml 読み書きに使う。`serde_yaml` ではなく `saphyr` を選択する。

### Repository による抽象

集約ごとに trait（`CharacterRepository` / `PreferencesRepository`）を定義し、`Filesystem*` と `InMemory*` の 2 実装を持つ。テストは contract-style で書き、両実装で同じシナリオを通す（`*_repository_satisfies_contract`）。app の DI は production で `Filesystem`、unit test で `InMemory` を inject する。

### 物理ファイル副作用は専用メソッドで分離

`update()` は metadata yml のみを書き、`sprite-groups/{group}/sprites/*.png` などの実バイナリは触らない（`remove_dir_all` で吹き飛ばさない）。画像の取り込み / 削除は `import_sprite_image` / `delete_sprite_image` という専用メソッドで分け、partial-failure 時は呼び出し側がロールバックする（`features/character/ui/import_sprites.rs`、`features/character/ui/create_character.rs`）。

## Alternatives Considered

- **SQLite**: トランザクション・スキーマ強制という利点はあるが、(2) Git diff が読めない、(3) `sed` で一括編集できない、(4) 別ツールでデータを開けない、という 3 つの要件を全部落とす。1 ユーザー前提でトランザクションが要らない以上、利点を活かせない。マイグレーション運用も追加コスト。

- **JSON**: コメントを書けない（人間が `# TODO: pivot 再調整` のようなメモを残せない）。yml なら inline コメントで「この値は仕様 X による」と書ける。プログラム的に読み書きする限りでは大差ない。

- **TOML**: ネスト配列・配列の中の構造体の表現が冗長。`SpriteGroup.sprites` のような `Vec<Sprite>` は yml の方が素直に書ける。

- **単一 bundle ファイル（全 Character を 1 yml に）**: Character 単位の git diff が肥大化し、コンフリクトの粒度が悪化する。集約単位 1 ファイルなら「`MooR_01.yml` だけ変わった」と一目で分かる。

- **MessagePack / 独自バイナリ**: Git diff も手編集もできず (2)(3) を落とす。サイズも個人ツール規模では問題にならない。

- **`serde_yaml`（保守停止）**: `serde_yaml` は 0.9 以降メンテされず警告も出る。`saphyr` ベースの後継として `serde_saphyr` を採用する。出力が圧倒的に綺麗なのも理由。

- **Repository を立てず直接 `fs::read_to_string` を散らす**: テストで filesystem を mock するのが面倒。`InMemoryCharacterRepository` を 1 つ持っておけば in-process でテストが書けて高速。

## Consequences

**得られたもの**

- workspace を丸ごと Git で履歴管理できる。コンフリクトが起きても yml をマージするだけで解消可能
- バックアップは「workspace ディレクトリをコピー」で完結。export/import を作る必要なし
- テスト時は `InMemoryCharacterRepository` で disk 抜きの単体テストが書ける。filesystem 実装は `tempfile::tempdir()` で integration test
- 「`{name}.yml` のみ更新（`update_metadata`）」「子集約のみ更新（`rename_sprite_group` 等）」のような細粒度 write を Repository に追加する自由度がある（`entities/character/README.md` に整理）
- 手編集や外部スクリプトでデータを生成しても、yml さえ整合していれば editor がそのまま読み込める

**支払うコスト**

- partial-failure（複数ファイル書き込みの途中失敗）に対する原子性は提供されない。`import_sprite_image` で画像コピー → yml 更新の途中失敗時は呼び出し側がロールバックを書く必要がある（`features/character/ui/import_sprites.rs` 参照）
- ファイル名 = 集約名なので、OS の禁則文字（`/` や `:` 等）を集約名に含めると壊れる。現状 validation はゆるく、編集時に弾く設計
- Animation 内の Layer が SpriteGroup を `number` で参照する関係上、yml 直編集で `number` を変えると参照が切れる（→ 集約構造ドキュメントで規約化、editor からのリネームは保護される）
- スキーマ進化はコードと yml を同時に変える必要がある。マイグレーションは「古い yml を読めるよう `#[serde(default)]` で穴埋め」する戦略（`Preferences::key_bindings` がこの方式）

**今後の拡張余地**

- 同時編集が要件になったら（複数人 / 複数プロセス）、Filesystem 実装を捨てて SQLite ベースの新しい `CharacterRepository` 実装を足す。Repository trait のおかげで上層は影響を受けない
- 集約サイズが増えてロード時間が問題になったら、`get` を sub-aggregate のオンデマンド読み込みに変える（trait シグネチャは互換）
- 大量の画像で `read_dir` が遅くなったら、`{name}.yml` 内に sprite_group / animation の名前一覧をキャッシュとして書き、読み込み時のディレクトリ走査を省略する余地がある

## 関連

- ADR-0003: Aggregate root maps to FSD slice（集約 = slice の対応）
- `entities/character/README.md`: 永続化レイアウトと write API の責務範囲
- `.claude/docs/data-flow.md`: Repository / Signal / Refresh トリガーの全体像
