use anyhow::{Result, anyhow};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "bebeu-editor.yml";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    workspace_dir: PathBuf,
}

impl Config {
    pub fn workspace_dir(&self) -> &PathBuf {
        &self.workspace_dir
    }

    pub fn load() -> Result<Self> {
        let mut config_path = std::env::current_exe()?
            .parent()
            .ok_or(anyhow::anyhow!("カレントディレクトリの取得に失敗。"))?
            .join(CONFIG_FILE_NAME);

        if cfg!(debug_assertions) {
            config_path = std::env::current_dir()?.join(CONFIG_FILE_NAME);
        }

        if config_path.exists() {
            let config_content = fs::read_to_string(config_path)?;
            let config: Config = serde_saphyr::from_str(&config_content)?;
            Ok(config)
        } else {
            let config_dir = FileDialog::new()
                .set_title("設定ディレクトリを選択してください")
                .pick_folder()
                .ok_or(anyhow!("設定ディレクトリの選択に失敗。"))?;

            let workspace_dir = FileDialog::new()
                .set_title("ワークスペースディレクトリを選択してください")
                .pick_folder()
                .ok_or(anyhow!("ワークスペースディレクトリの選択に失敗。"))?;

            let config = Self { workspace_dir };

            let config_content = serde_saphyr::to_string(&config)?;

            config_path = config_dir.join(CONFIG_FILE_NAME);
            fs::write(config_path, config_content)?;

            Ok(config)
        }
    }
}
