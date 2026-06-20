# ADR-0020: ファイル import の atomic 化を pending list + unmount rollback で行う

## Status

Accepted

## Context

editor では SpriteGroupEditor / SoundGroupEditor のような「Stay-on-screen Editor」(Pattern D) 上で、ユーザーが yml の中身 (Sprite / Sound 一覧) を編集しつつ、外部ファイルを取り込んで disk にコピーする操作を扱う:

- SpriteGroupEditor: 画像の追加 import、倍率指定での同名上書き再 import、削除
- SoundGroupEditor: WAV の追加 import、削除
- AnimationEditor: 既存 sprite の参照のみ (= disk への新規 import 無し)

これらの editor は Save / Cancel / Undo / Redo のライフサイクルを持つ (→ `packages/editor/src/features/character/ui/README.md` Pattern D)。問題は:

1. 画像 / WAV を取り込むと **その瞬間に disk が変わる** (yml に commit する前)。Cancel するとユーザーは「変更が無かったこと」を期待するが、disk には残る → orphan ファイル
2. 同 basename での上書き import を許すと、Cancel で「上書き前の画像」に戻したいが、上書きしてしまった後では復元できない
3. Undo すると yml 上は Sprite が消えるが、disk からも消すと「Redo した時 (= もう 1 回 import せず) yml だけ戻すと参照先が無い」ので、disk は editor セッション内で持ち越したい
4. SoundGroupEditor を unmount (= ブラウザ的にページ遷移) したときも Cancel と同じ rollback 挙動が必要

「Save した分だけ disk に残す」を Repository の write API 側で完全に保証する案 (= editor は disk を一切触らず、Save 時にメモリ上の draft + 添付バイト列を repository に渡す) も検討したが、画像 / WAV のサイズと枚数が増えると現実的でなく、preview 表示も困難。

## Decision

**editor 側で「pending list (= disk に居るが yml に commit されていないファイル)」を session signal として保持し、Save で commit、unmount で rollback する。**

### SpriteGroupEditor: `SpriteDiskOps` (3 種の pending を集約)

```rust
pub struct SpriteDiskOps {
    pub pending_imports: Vec<String>,    // disk にコピー済 / yml 未 commit
    pub pending_deletions: Vec<String>,  // yml から消す予定 / disk はまだ存在
    pub pending_overwrites: Vec<String>, // 同 basename 上書き → {basename}.bak がある
}
```

| 状態遷移 | Save (commit) | Cancel / unmount (rollback) |
|---|---|---|
| `pending_imports` | 何もしない (disk 残置 = 確定) → list クリア | `delete_sprite_image` で disk から消す |
| `pending_deletions` | `delete_sprite_image` で実際に消す | 何もしない (disk 残置 = 元の状態) |
| `pending_overwrites` | `discard_sprite_image_backup` で .bak を消す | `restore_sprite_image_backup` で .bak から戻す |

### SoundGroupEditor: 同パターンの簡易版 + repository 側 sweep

```rust
let pending_imports = use_signal(Vec::<String>::new);

use_drop(move || {
    for basename in pending_imports.peek().iter() {
        let _ = repo.delete_sound_file(...);  // unmount rollback
    }
});
```

加えて、orphan WAV (Sound 行を draft 上で消したのに disk に残ったファイル) は **`update_sound_group` (= Save 時の repository write) が `sounds/` ディレクトリ全体を yml と差分突き合わせて自動削除** する。これにより `pending_deletions` を editor 側で持たなくて済む。

### 上書き禁止の追加ルール (SoundGroup)

`import_sound_file` は同名 basename がある場合 **error を返す** (= 上書きしない)。Cancel rollback で committed 済みファイルが消える事故を防ぐため。同名で取り込みたい場合は先に Sound を draft から消して Save (disk からも消える) → 再 import の 2 段階で行う。

### unmount rollback の実装

- Dioxus の `use_drop` (component のライフサイクル末尾で呼ばれる closure) で pending list を読み、各 basename に対応する rollback API を呼ぶ
- Save 成功時は editor の Actions コンポーネントが pending list を空にしてから navigate するので、`use_drop` は no-op になる

## Alternatives Considered

- **Save 時にまとめて disk 操作 (editor は disk を一切触らない)**:
    - 一番シンプルだが、import 直後の preview 表示や倍率再 import の副作用 (画像差し替え後の box scale) を「メモリ上の仮想 disk」で表現する必要があり、エンジニアリングコストが過大
    - 大量画像の場合、Save 時にまとめて I/O が走るため UI が固まる懸念

- **import 即 yml commit (atomic 性を諦める)**:
    - editor のコードは単純化するが、ユーザーの「Cancel = 変更が無かった」期待を破る
    - Undo もトランザクションを跨ぐので意味が壊れる

- **OS 提供のトランザクション API (Windows TxF など) を使う**:
    - クロスプラットフォームで使えない (TxF は deprecated、APFS の clone は Mac のみ)
    - 「open editor が複数ある」「OS のクラッシュ復元」の文脈ではないので、application-level の rollback で十分

- **pending list を Repository 側で保持**:
    - editor を replace しても rollback が効く利点があるが、Repository が「session 中の状態」を持つことになり、ステートレスな contract が崩れる
    - 同時に複数 editor が開いていない前提なら、editor 側で持つほうが責務が局所化する

## Consequences

**得られたもの**

- import / 上書き / 削除のいずれを行っても **Cancel / ページ遷移で disk が元の状態に戻る** (atomic)
- `pending_overwrites` + `.bak` 方式により、同 basename の上書き再 import まで rollback 可能 (倍率再 import の主要ユースケース)
- SoundGroup では repository 側の `update_sound_group` sweep で orphan を自動掃除するため、editor が `pending_deletions` を持つ必要がなく実装が簡素 (= 同パターンの簡易版で動く)
- `use_drop` による rollback は「ユーザーが何もせず別ページへ移動した」ケースでも自動で走る (NavigationGuard が許可した場合のみ unmount するので、未保存変更の取り扱いとも整合)

**支払うコスト / 注意点**

- editor を実装する側は「disk を触る操作」と「pending list の更新」をペアで行う規約を厳守する必要がある (SpriteDiskOps の `add_pending_import` / `add_pending_overwrite` の呼び忘れ = orphan の温床)
- `.bak` ファイルが残った状態でアプリがクラッシュすると、disk に `{basename}.bak` が残る (次回 import で衝突 / 表示は素直)。手動掃除が必要だが、頻度が低いので許容
- SoundGroup の同名上書き禁止は UX として「分かりにくい error」を出す可能性がある。toast に「先に削除してから再 import してください」と書いてある前提で受け入れる
- pending list は Signal なので、Save → 別 SoundGroup を開く、のような遷移時に確実にクリアされる必要がある (= component が unmount されて再 mount すれば自動リセットされるが、同 component の中で対象を切り替える設計にすると壊れる。現状は URL ベースで component が再生成される構造なので問題なし)

**今後の拡張余地**

- 同パターンが必要な future feature (例: animation export 時の一時ファイル) は同じ shared 構造体 (`SpriteDiskOps` の汎用版) を流用
- multi-file 同時 import (現状は 1 ファイル選択だけ) を入れる場合、pending list は `Vec` のまま追記して unmount で全部消すだけで動く
- save / cancel を明示する旧来の dialog 駆動エディタに戻す場合でも、commit/rollback の API は repository 側で `discard_*_backup` / `restore_*_backup` / `delete_*` の 3 つに揃っているので流用可能
