# ADR-0001: Adopt Feature-Sliced Design (Rust/Dioxus port)

## Status

Accepted

## Context

editor は Rust + Dioxus desktop アプリで、複数のドメイン（Character, SpriteGroup, Animation, ...）と多数の CRUD 系 feature を抱えて成長していく予定。明示的なアーキテクチャを敷かないと、ファイル配置がドリフトし「この処理はどこに置くべきか」を毎回判断することになり、feature 同士の依存も絡まる。

## Decision

**Feature-Sliced Design** を Rust の facade パターンに翻案して採用する:

- レイヤー（上 → 下）: `app` / `pages` / `widgets` / `features` / `entities` / `shared`
- 上位レイヤーのみ下位レイヤーを import 可。同レイヤー間の slice 依存は禁止
- Public API は `{slice}.rs` の facade ファイルで `mod` + `pub use` の組で表現する。**`pub mod segment;` で内部を晒さない**
- ファイル命名は Rust 2018+ の `slice.rs` + `slice/` ディレクトリ形式（`mod.rs` を使わない）

詳細仕組みと実例は `.claude/docs/fsd.md` 参照。

## Alternatives Considered

- **プレーンな Rust モジュール構成**: cargo 流（`src/<topic>.rs`）。レイヤーが強制されないため、規模が大きくなると相互依存に戻る。
- **垂直スライス（レイヤーなし）**: 小規模では綺麗。Character / Animation / Effect が相互参照し始めると同レイヤー依存が発生する。
- **Hexagonal / Clean Architecture**: 抽象度が高くファイル配置への指示が弱い。FSD は配置レベルまで落ちている。

## Consequences

**得られたもの**

- Rust の `mod` 可視性で FSD の encapsulation principle を **コンパイラが強制** する。deep-import 違反は build error。
- 「どこに置くか」が決定木（layer → slice → segment）で機械的に決まる。
- 同レイヤー間の独立性が保証されているので、複数 feature の並行作業が安全。

**支払うコスト**

- facade のボイラープレート（`mod x; pub use x::Y;`）。
- 「`mod.rs` を使わない」流儀がチーム/個人の慣れを要求する。
- aggregate と slice の対応を意識する必要がある（→ ADR-0003）。
