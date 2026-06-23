# Debug ビルド

`just engine-run` (alias: `en-run`) は debug profile (`cargo run`) で起動する。release を使いたい場合は `just engine-run-release` (alias: `en-run-rel`)。

```sh
just engine-run                # debug    (cargo run)
just engine-run-release        # release  (cargo run --release)
```

## ログレベル

既定の log filter は `wgpu=error,wgpu_core=error,wgpu_hal=error,naga=warn,info`。Bevy `LogPlugin` の仕様により `RUST_LOG` を設定するとそちらが優先される。

```sh
RUST_LOG=debug just engine-run
```

吹っ飛びフロー (ADR-0024〜0025) や Guard 経路 (ADR-0028) のヒット解決は `tracing::info!` で発火条件・効果ベクトル・gauge 値を吐く。`RUST_LOG=info` ないし `debug` に上げると挙動の追跡に役立つ。

## デバッグ overlay

debug profile / release profile どちらでも有効。toggle は function キーで切り替え:

| キー | overlay |
|---|---|
| `F1` | Hitbox overlay — AttackBox (赤) / BodyBox (緑、無敵 frame は枠線のみ) / Pivot 点を Bevy `Gizmos` で描く |
| `F2` | State debug overlay — 各 Player / Enemy entity の頭上に `state / gauge / bounce / final_action / hit_from_behind / juggle / down_hit` を 1 行で表示 |

両 overlay とも初期状態は OFF。`tracing::info!` でも toggle 結果がログに残る。

## Pause / Frame advance

吹っ飛びアニメの 1 frame ごとの挙動を観察するための単発再生:

| キー | 動作 |
|---|---|
| `F3` | Pause を toggle (gameplay system 全停止)。Animation tick / hit 判定 / 物理積分すべて止まる |
| `F4` | Pause 中に 1 frame だけ進める (single step)。pause 中に押された `just_pressed` 入力 (例: `J` 攻撃) も同時に再点火される |

(実装上の細部: pause 中も input toggle 系と debug overlay 描画系は走らせる。pause 中に `just_pressed` を取りこぼさないよう `latch_paused_input` が積んでおき、`F4` で再放出する設計。詳細は `features/character/debug_control.rs` を参照。)

## hitbox / state overlay の役割分担

- **Hitbox overlay (`F1`)** — geometry レベルのデバッグ。攻撃が当たっているか / 無敵 frame が効いているか / Pivot のズレを目視で確認する
- **State overlay (`F2`)** — semantic レベルのデバッグ。なぜ吹っ飛ばないのか / なぜガードクラッシュしないのか / juggle cap に当たっているか、を gauge / counter で確認する
