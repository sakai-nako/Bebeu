# キーバインド

エディタ内のショートカット既定値。`Edit Key Bindings` モーダルから個別に変更でき、変更内容は `preferences.yml` の `key_bindings` セクションに保存される。

## 共通

| キー | アクション |
|---|---|
| `Ctrl + S` | 保存 (Save) |
| `Ctrl + Z` | 元に戻す (Undo) |
| `Ctrl + Shift + Z` | やり直す (Redo) |

Undo / Redo は active な Editor 1 つに対して作用する。

## SpriteGroup Editor

| キー | アクション |
|---|---|
| `Ctrl + ←` / `Ctrl + →` | 前の / 次の Sprite を選択 |
| `Ctrl + Home` / `Ctrl + End` | 最初の / 最後の Sprite を選択 |
| `Ctrl + Shift + ←` / `Ctrl + Shift + →` | 選択中 Sprite を前へ / 後ろへ移動 |
| `Shift + ↑` / `Shift + ↓` | Pivot を下へ / 上へ移動 |
| `Shift + ←` / `Shift + →` | Pivot を右へ / 左へ移動 |

(Pivot 移動はキー方向と画面上の挙動が逆になる組み合わせを既定値にしている。違和感があればモーダルで差し替える前提。)

## Animation Editor

| キー | アクション |
|---|---|
| `Ctrl + ←` / `Ctrl + →` | 前の / 次の Frame を選択 |
| `Ctrl + Home` / `Ctrl + End` | 最初の / 最後の Frame を選択 |
| `Space` | Animation を再生 / 一時停止 |
| `Shift + Space` | 再生停止 (先頭フレームへ戻る) |
