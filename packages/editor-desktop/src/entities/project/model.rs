use serde::{Deserialize, Serialize};

/// 論理解像度のデフォルト幅 (px)。
const DEFAULT_RESOLUTION_WIDTH: u32 = 640;

/// 論理解像度のデフォルト高 (px)。
const DEFAULT_RESOLUTION_HEIGHT: u32 = 360;

/// 1 つのプロジェクト設定。`workspace/data/projects/{name}.yml` に永続化される。
///
/// 1 workspace に複数 Project を並べ、engine 起動時に `--project <name>` で指定する。
/// Character / Level の master pool は workspace/data/characters/ と workspace/data/levels/
/// に共有で置かれ、Editor 上では Project を介さず直接編集できる。Project は engine 起動の
/// preset (どの player / opponent / level で起動するか) としてのみ機能し、Editor 内の
/// Character / Level 一覧をフィルタすることはしない。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// ディレクトリ名 (= ファイル名 stem) から復元される。YAML には書かない。
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub resolution: Resolution,
    #[serde(default)]
    pub players: Vec<String>,
    #[serde(default)]
    pub opponents: Vec<String>,
    #[serde(default)]
    pub levels: Vec<String>,
}

/// 論理解像度（描画バッファのサイズ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            width: DEFAULT_RESOLUTION_WIDTH,
            height: DEFAULT_RESOLUTION_HEIGHT,
        }
    }
}
