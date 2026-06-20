# ADR-0003: Aggregate root maps to FSD slice

## Status

Accepted

## Context

FSD は同レイヤーの slice 同士の依存を禁止する（→ ADR-0001）。一方ドメインには相互に深く絡む概念がある: Character は SpriteGroup を所有し、SpriteGroup は Sprite を所有する。これらを「ドメイン概念ごとに 1 slice」と機械的に切ると `entities/character/` から `entities/sprite_group/` への import が必要になり、同レイヤー依存違反になる。

「では SpriteGroup を slice にしないのは何か基準があるのか？」を曖昧にすると、新しい概念を追加するたびに slice 化判断がブレる。

## Decision

**DDD の集約ルートと FSD の slice を 1:1 で対応させる**。集約の子エンティティはルートと同じ slice に同居する。

例:

- `Character` は集約ルート → `entities/character/` slice
- `SpriteGroup` / `Sprite` / `Animation` / `Layer` は Character の子 → `entities/character/model.rs` 内に同居

「これは独立 slice にすべきか？」の判定:

1. 既存集約の子なら → その slice に join
2. 独立した集約ルートなら → 新 slice

集約間で本当に協調が必要な場合は **上位レイヤー（widgets / pages）で合流させる**。同レイヤー横断は禁止のまま。

## Alternatives Considered

- **概念ごとに 1 slice**: SpriteGroup / Animation / Sprite を独立 slice 化。Character から参照する瞬間に同レイヤー依存違反。
- **トピック単位（`entities/data/`）**: 粗すぎる。ドメイン境界を失う。
- **slice 内での namespace 切り**: Rust では module 階層と一致するため事実上 slice を切ることに等しい。

## Consequences

- `entities/character/model.rs` には複数の struct（Character, SpriteGroup, Sprite, Animation, Layer）が同居する。ファイルサイズが膨らんだら segment（`model/character.rs`, `model/sprite.rs`, ...）に分割する余地はある。
- 新しいドメイン概念を入れるとき「集約ルートか？」を必ず判定する必要がある。
- 異なる集約間（例: 将来 UserPreference が Character を参照）は widgets / pages で合流。entities 間の cross-import は今後も発生しない設計。
