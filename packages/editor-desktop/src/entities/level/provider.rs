use std::collections::HashMap;

use dioxus::prelude::*;

use super::Level;

/// `Signal<HashMap<String, Level>>` を Dioxus context で配布する。
///
/// Level は複数インスタンスを持つので、name → Level のマップを source of truth にする。
/// 現状 `app_root.rs` からは呼ばれない (Level 編集 UI 未実装)。Stage 2+ で UI を載せる
/// タイミングで AppMain で 1 度だけ呼ぶ。
pub fn use_level_provider(initial: HashMap<String, Level>) -> Signal<HashMap<String, Level>> {
    use_context_provider(|| Signal::new(initial))
}

/// 配布済みの Signal を取得する。pages / widgets / features から呼ぶ。
pub fn use_level() -> Signal<HashMap<String, Level>> {
    use_context::<Signal<HashMap<String, Level>>>()
}
