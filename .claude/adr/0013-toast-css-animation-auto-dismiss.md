# ADR-0013: Toast の auto-dismiss を CSS animation で実装する

## Status

Accepted

## Context

`shared/toast.rs` に汎用トースト通知システムを入れた (Success / Error / Warning / Info の 4 種、画面右下に daisyUI `toast toast-end` で重ねる)。要件は次の 4 つ:

1. Success / Info は「数秒見せて自動で消える」UX (邪魔にならない)
2. Error / Warning は手動で `×` を押すまで残す (重要度の高い情報を見落とさない)
3. ユーザーがホバー中はカウントダウンを止めて読ませる時間を確保する
4. ADR-0002 の **同期方針 (no async/.await)** を破らない

問題は (1) と (4) の両立。「N 秒後に消す」は素直に書けば `tokio::time::sleep().await` か JavaScript `setTimeout` だが、いずれも追加の runtime / 副作用導入を伴う。

## Decision

**`assets/theme.css` で auto-dismiss 用の CSS keyframe アニメーションを定義し、トースト DOM 要素にそのクラスを付与する。アニメーション完了の `animationend` DOM イベントを Dioxus の `onanimationend` で拾って、Rust 側の queue から該当トーストを除去する。**

```css
@keyframes toast-auto-dismiss {
    0%, 75% {  opacity: 0.92; transform: translateX(0); }
    100%    {  opacity: 0;    transform: translateX(20px); }
}
.toast-auto-dismiss {
    animation: toast-auto-dismiss 4s forwards;
}
.toast-auto-dismiss:hover {
    animation-play-state: paused;
}
```

```rust
// shared/toast.rs ToastItem
onanimationend: move |_| {
    if auto_dismiss { queue.write().dismiss(id); }
},
```

`ToastKind::auto_dismiss()` が「Success / Info → true、Error / Warning → false」を返し、true のトーストだけ `toast-auto-dismiss` クラスが付く。Error / Warning は `animationend` が発火しないので残り続ける。

ホバーで `animation-play-state: paused` を効かせ、カーソルを外せば再開する。

## Alternatives Considered

- **`spawn(async { tokio::time::sleep().await })`**: 軽量だが ADR-0002 の同期方針を破る。`async` を 1 箇所許すと「タイマー以外でもなし崩し」になりやすく、線引きが曖昧になる。
- **`std::thread::spawn` + `thread::sleep` + channel で UI スレッドへ通知**: タイマーごとに OS スレッドを起こすのは重い。Signal の Send 制約や `Arc<Mutex<...>>` のラップも必要で、たかが UI タイマーには牛刀。
- **`document::eval()` で `setTimeout` を仕込み、custom event 経由で Rust に通知**: JS 側の lifecycle 管理 (clear タイミング、unmount 時の cleanup) を自前で書く必要があり、CSS animation より複雑。
- **自動消滅を実装しない (手動 close のみ)**: Success / Info も `×` で消す UX。当初はこれで進めたが、import 等の頻発する成功通知が画面を埋めるので不採用。
- **タイマーを持たず、トーストの DOM 自体に `transition` で fade out させて、`transitionend` で除去**: transition は class 変更などの状態遷移に紐づくので「N 秒後に発火」を直接表現できない。結局 CSS animation の出番。

## Consequences

**得られたもの**

- ADR-0002 の同期方針を破らずに auto-dismiss が成立する。Rust 側に async / runtime / channel が増えない。
- ホバー pause が CSS 1 行 (`animation-play-state: paused`) で書ける。Rust 側のタイマー一時停止ロジックは不要。
- Toast の表示時間 (現状 4 秒、最後の 25% で fade) は `theme.css` の keyframe を編集するだけで調整できる。Rust 側のコード変更不要。

**支払うコスト**

- 「タイマー」という概念が CSS 側にある。Rust の queue 寿命が DOM のアニメ完了に依存する形になる。Component が unmount されると `animationend` が発火しないので queue に残る可能性があるが、現状 ToastHost は app_root で常に mount されているので問題は出ない (将来 ToastHost を条件付き mount するなら要再検討)。
- 自動消滅の対象 (Success / Info か、Error / Warning か) が `ToastKind::auto_dismiss()` という Rust 側の判定だけで決まる。「同じ kind でも個別にタイマー指定したい」(例: 長文 Success は 8 秒) は現状サポートしない。必要になったら `Toast` struct に `duration: Option<Duration>` を生やして CSS 側に inline `style="animation-duration: ..."` を渡す形で拡張可能。
- `onanimationend` は他の CSS animation でも発火する。ToastItem 内に他のアニメが混ざらないよう注意 (現状 `toast-auto-dismiss` 以外のアニメは無いので問題なし、daisyUI の transition は animation ではなく transitionend を使う)。

**今後の拡張余地**

- 「重要なトーストは画面に残す + 補助的なものは自動消滅」というポリシーが明確化されたので、新しい `ToastKind` を増やすときも `auto_dismiss()` の分岐に揃える形で素直に拡張できる。
- ホバー pause の挙動をベースに、将来「クリックで pause」「pause インジケータ表示」等の UX 拡張も CSS / Rust 両側でほぼ独立に追加できる。
