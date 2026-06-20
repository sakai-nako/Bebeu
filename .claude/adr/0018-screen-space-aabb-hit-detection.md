# ADR-0018: hit 判定を screen 空間 AABB で行う

## Status

Refined by [ADR-0021](0021-world-3d-aabb-with-per-box-depth.md) — Stage 4 開始前に world 3D AABB へ移行した。box 解決ロジック (`ResolveScreenBoxes`) は debug overlay 描画用として残存している。本 ADR は「Stage 3a で screen AABB を選んだ理由」の歴史記録として保持する。

## Context

Stage 3a で combat (Idle / Walk / Attack / Hit / Dead) を入れる際、attacker.attack box と defender.body box のヒット判定アルゴリズムを決める必要があった。

前提:

- world は 3 軸 (X, Y=高さ, Z=奥行き)。`Projector` で screen 2D に投影する → ADR-0017
- HitBox は editor 側で sprite-pixel 単位に書かれており、Inherit / Override / Disable の 3 状態を持つ → ADR-0014
- Frame 単位で box が変わる (= 攻撃の出際 / 戻りで判定範囲が動く)
- box overlay (debug 表示) は既に screen 矩形に解決して描画している (Stage 2)

主な選択肢:

1. **world 3D AABB**: `(world_x, world_y, world_z)` で box を 3D 直方体に展開し、3D 重なりを取る
2. **world 2D AABB (XZ 平面)**: 高さを無視し、X / Z だけで 2D 重なり判定
3. **screen 2D AABB**: ResolveScreenBoxes で投影済みの screen 矩形同士で AABB 重なりを取る

## Decision

**Stage 3a では screen 2D AABB を採用する。** `features/attack.Hits` は `entities/character.ResolveScreenBoxes` で attacker / defender それぞれの screen 矩形を取り、`shared/hitbox.AnyOverlap` で重なりを判定する。

```go
// features/attack/attack.go
func Hits(...) bool {
    att := character.ResolveScreenBoxes(attackerWorld, attacker, attackerFrame)
    def := character.ResolveScreenBoxes(defenderWorld, defender, defenderFrame)
    return hitbox.AnyOverlap(att.Attack, def.Body)
}
```

box 解決は box overlay (Stage 2) と同じ関数 (`ResolveScreenBoxes`) を共有する。これにより「box overlay で重なって見える ≒ hit する」が見た目として一致する。

## Alternatives Considered

- **world 3D AABB**:
    - 厳密には正しい (高さ / 奥行きの違いがそのまま判定に反映される)
    - しかし editor の box 編集 UI は sprite-pixel 平面のみ。world Z 方向の厚みを編集する手段が無く、box ごとに「Z 厚み = 一定」の暗黙仕様を入れる必要がある
    - Stage 3a の目的は「2 体目を殴って Hit / Dead 状態に遷移させる」最小実装で、奥行きの厳密判定は要らない
    - 投影で screen Y にずれが乗るので、screen AABB でも実質「Z が近いほど当たる」挙動になる (後述)

- **world 2D AABB (XZ 平面)**:
    - 高さを無視するので jump 中のキャラに当たり放題になり、Stage 4 以降で破綻する
    - Y を含めない時点で screen 投影の式 (Y - Z*scale が screen Y) と整合せず、視覚と判定の乖離が起きる

- **円 / カプセル形状 + 距離判定**:
    - hit 判定が滑らかになるが、editor 側の box は矩形前提で書かれており editor の改修コストが大きい
    - 既存の `HitBox{TopLeft, BottomRight}` から円に変換するロジックを 1 段噛ませる必要が出る

## Consequences

**得られたもの**

- 実装が screen 矩形 1 種類で完結する: box overlay / hit 判定で `ResolveScreenBoxes` を共用、`hitbox.Overlap` の AABB 計算 1 回で済む
- editor で「画面上で見えている当たり方」がそのまま hit 判定になるため、調整サイクルが直感的 (overlay を出して殴って合うかを目視で確認できる)
- 「奥/手前のキャラには当たりにくい」が ADR-0017 の投影式によって自動的に得られる: 同じ Z 同士は screen Y が一致して殴りやすく、Z が離れると screen Y がずれて殴りにくい。投影が「奥行き許容度」を兼ねる
- screen flip (FlipH=true で `2*charX - rect.MinX/MaxX`) も `applyFlipScreen` で 1 ヶ所に集約され、attacker / defender の facing 違いも素直に処理できる

**支払うコスト / 注意点**

- 「実 world 距離」と「screen 距離」が ZScale の値で歪む。`level.ZScale` を大きくすると Z 違いに敏感、小さくすると鈍感、になる。ZScale を Level 単位で動かすと judging が変わる点を運用で意識する必要がある
- jump (world Y > 0) を入れると attacker / defender の screen Y が上下にずれる。これは「ジャンプ中は腰下を殴りにくい」など意図通りの挙動になる予定だが、初期 tuning 時に「想定より当たらない / 当たりすぎる」差分が出やすい
- self-hit (i == j) のスキップなど、呼び出し側 (`battle.Scene.resolveHits`) のガードを抜くと自分で自分を殴る。判定関数の責務外なのでテストでも別に検証する
- 高速移動で 1 tick 飛び越えする (= 接触フレームを取りこぼす) ケースは現状ノーガード。Stage 4 で必要になれば前後フレームの sweep を `attack` package 側に足す

**今後の拡張余地**

- world 3D 判定が必要になった場合 (= 編集 UI で box の Z 厚みを持たせる方針に転換した場合) は、`Hits` のシグネチャは保ったまま `ResolveScreenBoxes` を `ResolveWorldBoxes` に置き換える形で内部実装だけ差し替え可能
- 当たりの「先勝ち」「相打ち」など Stage 4 で必要になる priority 制御は、`features/combat` 側で TakeHit を呼ぶ前のフィルタとして足すのが自然 (`attack` は判定関数のままに保つ)
- AI を Stage 4 で入れる際、AI が「殴れる距離」を判断するためにも同じ `Hits` を hypothetical world で呼ぶ形で再利用できる
