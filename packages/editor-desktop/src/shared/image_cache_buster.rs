use dioxus::prelude::*;

/// 画像 URL に追加するキャッシュバスタ用カウンタ。
///
/// 同じ basename のまま disk 上で画像を上書き差し替えしたとき、webview の HTTP キャッシュは
/// 古い画像を持ち続けるので、URL に `?v={N}` を付けて N を bump することで再フェッチを促す。
///
/// 配布は context 経由 (`use_image_cache_buster_provider`)、読み取りは
/// `use_image_cache_buster` で行う。provider が無いスコープでは `None` が返り、
/// `versioned_asset_url` は素の URL を返す（= 表示崩れにはならない）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImageCacheBuster(pub u64);

impl ImageCacheBuster {
    pub fn bump(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}

/// SpriteGroupEditor などの編集サーフェスで呼び、配下の画像コンポーネントに
/// cache buster signal を共有する。戻り値の signal 経由で `bump()` を呼ぶと
/// 配下の画像 URL が一斉に再フェッチされる。
pub fn use_image_cache_buster_provider() -> Signal<ImageCacheBuster> {
    use_context_provider(|| Signal::new(ImageCacheBuster::default()))
}

/// provider が居れば signal を、居なければ `None`。
/// 画像表示コンポーネントは Editor 外（一覧画面など）でも使われるので、
/// 不在を許容する設計にしてある。
#[must_use]
pub fn use_image_cache_buster() -> Option<Signal<ImageCacheBuster>> {
    try_consume_context::<Signal<ImageCacheBuster>>()
}

/// 画像 URL にバージョンクエリを追加する。version が 0 なら追加しない（初期表示で
/// 余計なクエリ付き URL を生まないため）。
#[must_use]
pub fn versioned_asset_url(url: String, version: u64) -> String {
    if version == 0 {
        url
    } else {
        format!("{url}?v={version}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_asset_url_keeps_url_when_version_is_zero() {
        assert_eq!(
            versioned_asset_url("/workspace-asset/foo.png".into(), 0),
            "/workspace-asset/foo.png"
        );
    }

    #[test]
    fn versioned_asset_url_appends_query_when_version_is_nonzero() {
        assert_eq!(
            versioned_asset_url("/workspace-asset/foo.png".into(), 7),
            "/workspace-asset/foo.png?v=7"
        );
    }

    #[test]
    fn bump_increments_counter() {
        let mut b = ImageCacheBuster::default();
        assert_eq!(b.0, 0);
        b.bump();
        b.bump();
        assert_eq!(b.0, 2);
    }

    #[test]
    fn bump_wraps_around_on_overflow() {
        let mut b = ImageCacheBuster(u64::MAX);
        b.bump();
        assert_eq!(b.0, 0);
    }
}
