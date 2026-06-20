use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

/// 画像として扱う拡張子（小文字比較）。
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg"];

/// 指定フォルダ直下の画像ファイルを basename 昇順で列挙する（再帰しない）。
/// 拡張子の判定は大文字小文字を無視する。
pub(super) fn list_sorted_image_files(folder: &Path) -> Result<Vec<PathBuf>> {
    if !folder.is_dir() {
        return Err(anyhow!(
            "選択されたパスがフォルダではありません: {}",
            folder.display()
        ));
    }

    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        let ext_lower = ext.to_ascii_lowercase();
        if IMAGE_EXTENSIONS.contains(&ext_lower.as_str()) {
            paths.push(path);
        }
    }

    // file_name 文字列で昇順ソート（OS の natural sort は使わない）
    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_image_files_sorted_by_basename() -> Result<()> {
        let dir = tempfile::tempdir()?;
        // 画像
        fs::write(dir.path().join("walk_002.png"), b"")?;
        fs::write(dir.path().join("walk_001.png"), b"")?;
        fs::write(dir.path().join("walk_010.PNG"), b"")?; // 大文字拡張子
        // 非画像
        fs::write(dir.path().join("notes.txt"), b"")?;
        fs::write(dir.path().join("ignore.bak"), b"")?;
        // サブフォルダ（再帰しない確認用）
        fs::create_dir(dir.path().join("nested"))?;
        fs::write(dir.path().join("nested/inside.png"), b"")?;

        let names: Vec<String> = list_sorted_image_files(dir.path())?
            .into_iter()
            .map(|p| {
                p.file_name()
                    .expect("path is from read_dir")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        assert_eq!(names, vec!["walk_001.png", "walk_002.png", "walk_010.PNG"]);
        Ok(())
    }

    #[test]
    fn returns_empty_for_dir_without_images() -> Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("a.txt"), b"")?;
        let result = list_sorted_image_files(dir.path())?;
        assert!(result.is_empty());
        Ok(())
    }

    #[test]
    fn errors_for_non_directory_path() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("a.png");
        fs::write(&file, b"")?;
        assert!(list_sorted_image_files(&file).is_err());
        Ok(())
    }
}
