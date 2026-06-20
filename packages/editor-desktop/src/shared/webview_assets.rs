/// `use_asset_handler` に登録するスキーム名
pub const WORKSPACE_ASSET_SCHEME: &str = "workspace-asset";

/// img src 等で使う URL プレフィックス（スキーム名の前後に `/` を付けたもの）
pub const WORKSPACE_ASSET_URL_PREFIX: &str = "/workspace-asset/";

/// workspace ディレクトリ配下の相対パスを、webview が読める workspace-asset URL に変換する。
///
/// 例: `workspace_asset_url("data/characters/MooR_01/foo.png")`
///   → `"/workspace-asset/data/characters/MooR_01/foo.png"`
///
/// 先頭の `/` は重複しないよう取り除かれる。
#[must_use]
pub fn workspace_asset_url(rel_path: &str) -> String {
    format!(
        "{WORKSPACE_ASSET_URL_PREFIX}{}",
        rel_path.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_prefix_matches_scheme() {
        // 2 つの定数は手動で同期させているので、形が合っているかチェック
        assert_eq!(
            WORKSPACE_ASSET_URL_PREFIX,
            format!("/{WORKSPACE_ASSET_SCHEME}/")
        );
    }

    #[test]
    fn workspace_asset_url_basic() {
        assert_eq!(
            workspace_asset_url("data/characters/foo.png"),
            "/workspace-asset/data/characters/foo.png"
        );
    }

    #[test]
    fn workspace_asset_url_strips_leading_slash() {
        assert_eq!(
            workspace_asset_url("/data/characters/foo.png"),
            "/workspace-asset/data/characters/foo.png"
        );
    }

    #[test]
    fn workspace_asset_url_empty() {
        assert_eq!(workspace_asset_url(""), "/workspace-asset/");
    }
}
