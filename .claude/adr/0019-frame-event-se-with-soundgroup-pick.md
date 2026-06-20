# ADR-0019: SE は Frame.sound + SoundGroup 重み付き Pick で発火する

## Status

Accepted (Stage 3c で Stage 3b の state-event SE 方式を置換)

## Context

Stage 3b で combat に効果音 (pain / death) を導入した時、SE は state 遷移 (`combat.TakeHit` 内で「ダメージ受けたら pain 鳴らす」「死んだら death 鳴らす」) に紐づける形で実装した。具体的には:

- `Combatant` が `eventPain` / `eventDead` の bool フラグを持ち、`TakeHit` でセット
- `Character` に `PainSoundNumber` / `DeathSoundNumber` の特殊フィールドがあり、`battle.Scene` がそれを読んで再生

Stage 3c で次の不満が出た:

1. **タイミングが state 遷移 = アニメ frame 0 に固定される**: 「被弾モーションの 2 フレーム目で痛がる声を出したい」「攻撃の振り下ろし frame で剣の音を出したい」のような演出が表現できない
2. **音の種類が pain / death 2 つに固定**: 攻撃ヒット音 / 攻撃の空振り音 / 着地音などを足すたび、`Combatant` のフラグと `Character` のフィールドを増やす必要がある (= Open/Closed 原則を満たさない)
3. **同じ用途で複数音をローテート再生したい** (例: pain_001.wav と pain_002.wav を交互に / ランダムに) が、Path 1 つしか持てない構造ではバリエーションを出せない

editor 側 (`packages/editor/`) で音編集 UI を作る前に、engine 側のモデルを再設計する必要があった。

## Decision

**SE は Animation の Frame に紐づく event として表現し、その値は `SoundGroup.Number` への参照とする。** 同 SoundGroup 内の複数 `Sound` から `Weight` で重み付き乱選して 1 つ再生する。state 遷移ベースの API (`eventPain` / `eventDead` / `Pain/DeathSoundNumber`) は削除する。

```go
// entities/character/model.go
type Frame struct {
    // ...
    Sound *FrameSound `yaml:"sound,omitempty"` // nil = 無音
}

type FrameSound struct {
    Number  uint32 `yaml:"number"`             // SoundGroup.Number
    DelayMs uint32 `yaml:"delay_ms,omitempty"` // frame 進入から再生開始までの遅延
}

type SoundGroup struct {
    Name   string  `yaml:"name"`
    Number uint32  `yaml:"number"` // Frame.sound から参照される
    Sounds []Sound `yaml:"sounds"`
}

type Sound struct {
    Index  uint32  `yaml:"index"`
    Path   string  `yaml:"path"`
    Volume float32 `yaml:"volume"`
    Weight float32 `yaml:"weight,omitempty"` // 省略 / 0 / 負値 → 1.0
}
```

エンジン側のディスパッチ機構:

- `Combatant` は `prevFrameIndex` を持ち、Update 末尾で frame 進入を検知
- 進入した frame に `Sound` があれば `pendingFrameSound` に latch (DelayMs と一緒に)
- 毎 tick `pendingDelayMs` を消化、0 で `eventFrameSound` に流す
- `Scene` が tick 毎に `Combatant.ConsumeFrameSound()` で number を取り出し、`Character.FindSoundGroup(number)` → `SoundGroup.Pick(rng)` で 1 音選んで再生

pending スロットは 1 つだけ。`switchTo` (Animation 切替) では pending をクリアし、新アニメ frame 0 の Sound が改めて latch される (= キャンセルされたアクションの SE は鳴らさない)。

pain / death の SE は `Hit` / `Dead` アニメの該当 frame に `sound: <number>` を仕込むことで再現する。

## Alternatives Considered

- **state-event 方式の継続 (Stage 3b の延長)**:
    - 新しい SE 種別を足すたびにモデルが膨張する。攻撃 / 着地 / ジャンプなどを Stage 4 で増やす予定があり破綻する
    - 「frame 単位のタイミング」を表現するには結局 frame に何かを書く必要がある

- **Frame.sound に WAV path を直接書く**:
    - YAML が単純になる利点はある
    - 同じ用途で複数音 (pain_001/002/003) をランダム再生したいケースで、毎 frame に同じバリエーション一覧を書く羽目になる
    - 音量 / 重みのメタデータを path 文字列に詰め込めない

- **Animation 単位で SE を持つ**:
    - 「このアニメに入った瞬間に鳴らす」モデル。実装は単純
    - frame 単位のタイミングが取れないので Stage 3c の主目的を満たさない

- **event queue (pending を複数持つ)**:
    - 「短い frame に Sound を 2 連で並べた場合に両方鳴らす」が表現できる
    - しかし 1 アニメ内で 2 連 SE を出したいケースが現状無く、queue を入れると「キャンセル時にどこまで残すか」「同 frame 内 2 個目をいつ消化するか」など仕様が増える
    - 1 スロット上書き (新 frame 進入で前 pending を捨てる) でも実用上問題ないと判断

- **重み付きではなく一様乱数 (Weight 廃止)**:
    - 一番シンプルだが、「pain_002 のほうが自然な音だから 2 倍出やすくしたい」のようなチューニングが効かない
    - YAML 上 weight 省略時に 1.0 で正規化する設計にすれば、シンプルケースは同じ書き方で済むので Weight ありを採用

## Consequences

**得られたもの**

- 新 SE 種別を足すコードコストが 0 (YAML に SoundGroup を追加して Frame.sound でその number を指すだけ)
- frame 単位の precise なタイミング (DelayMs での後ずらしも可能)
- 同用途 SE のバリエーション + 重み付きランダム再生が標準で表現できる
- `Combatant` が `*uint32` 1 つを抱えるだけで全 SE を扱える (state-event 方式での bool フラグ複数撤廃)
- Hit / Dead だけでなく Idle / Walk / Attack の任意 frame で音を鳴らせる (足音、構え時のセリフ等の余地)

**支払うコスト / 注意点**

- 1 スロット pending: delay 中に同じ anim 内で次の Sound 持ち frame に進むと前 Sound が破棄される。「毎 frame Sound を打つ」高密度シーケンスでは取りこぼしが起きる (現状そういうデータは無い)
- Animation 切替 (= switchTo) で pending をクリアする規約を厳守する必要がある。違反すると古いアニメの SE が新アニメ中に飛んでくる
- `prevFrameIndex` の初期値が **0**: 起動時に Idle frame 0 の Sound が誤発火しないようにするため。一方 switchTo は **-1** にリセットすることで、新アニメ frame 0 の Sound を発火させる。この使い分けは combat.go のコメントとテスト (`TestCombat_FrameSound_*`) で固定する
- 浮動小数点誤差での `Pick` のフォールバック (累積比較で末尾を抜けたケースで先頭 / 末尾を返す) は `sound_test.go` で検証必須
- editor 側に SoundGroup の概念を入れるコストが発生する (Stage 3c-editor シリーズで段階的に実装済み)

**今後の拡張余地**

- 「3D positional audio」「distance attenuation」を入れる場合、`flushSoundEvents` で entity の world 位置を audio engine に渡せば良い (Stage 3c では `Volume` のみ)
- 「直前と同じ Sound は連続で選ばない (anti-repeat)」は `SoundGroup.Pick` に前回 index を渡す形で拡張可能
- editor 側で「SoundGroup を Frame に貼ったときの preview 再生」機能を追加するときは `SoundGroup.Pick` + Volume をそのまま流用できる
