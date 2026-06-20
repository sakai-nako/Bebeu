use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::repository::{CharacterRepository, ImportOutcome};
use crate::entities::character::{Animation, Character, SoundGroup, SpriteGroup};
use crate::shared::{WavInfo, read_wav_info};

pub struct FilesystemCharacterRepository {
    base_dir: PathBuf,
}

/// 1 ディレクトリ直下の `*.yml` を全部読み込んで `Vec<T>` にする。dir が存在しなければ
/// 空ベクタを返す。サブディレクトリは無視する (画像・子集約には触らない)。
fn read_yaml_dir<T: DeserializeOwned>(dir: &Path) -> Result<Vec<T>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yml") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        let item: T = serde_saphyr::from_str(&content)?;
        items.push(item);
    }
    Ok(items)
}

/// `dir/{name_of(item)}.yml` を `items` で完全に同期する。`items` に含まれない
/// `*.yml` は削除し、含まれるものは serialize して書き込む。サブディレクトリ
/// (画像等) には一切触らないので、「直下 yml だけ」が責務範囲になる。
fn sync_yaml_dir<T: Serialize>(
    dir: &Path,
    items: &[T],
    name_of: impl Fn(&T) -> &str,
) -> Result<()> {
    fs::create_dir_all(dir)?;
    let current_names: HashSet<&str> = items.iter().map(&name_of).collect();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yml") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if !current_names.contains(stem) {
            fs::remove_file(&path)?;
        }
    }
    for item in items {
        let yaml = serde_saphyr::to_string(item)?;
        fs::write(dir.join(format!("{}.yml", name_of(item))), yaml)?;
    }
    Ok(())
}

/// `dir/{old}.yml` と `dir/{old}/` のセットを `{new}` 名にリネームする。`set_name` は
/// yml の name フィールドを書き換えるためのフック (例: `|c, n| c.name = n`)。
fn rename_yaml_aggregate<T>(
    dir: &Path,
    old_name: &str,
    new_name: &str,
    kind: &str,
    set_name: impl FnOnce(&mut T, String),
) -> Result<()>
where
    T: DeserializeOwned + Serialize,
{
    let old_yml = dir.join(format!("{old_name}.yml"));
    let new_yml = dir.join(format!("{new_name}.yml"));
    let old_subdir = dir.join(old_name);
    let new_subdir = dir.join(new_name);

    if !old_yml.exists() {
        return Err(anyhow!("{kind} '{old_name}' not found"));
    }
    if new_yml.exists() || new_subdir.exists() {
        return Err(anyhow!("{kind} '{new_name}' already exists"));
    }

    let content = fs::read_to_string(&old_yml)?;
    let mut item: T = serde_saphyr::from_str(&content)?;
    set_name(&mut item, new_name.to_string());
    let yaml = serde_saphyr::to_string(&item)?;
    fs::write(&new_yml, yaml)?;

    if old_subdir.exists() {
        fs::rename(&old_subdir, &new_subdir)?;
    }
    fs::remove_file(&old_yml)?;
    Ok(())
}

/// `dir/{name}.yml` と `dir/{name}/` のセットを削除する。yml が無ければ not_found
/// エラー。サブディレクトリは存在すれば再帰削除、無ければスキップ。
fn delete_yaml_aggregate(dir: &Path, name: &str, kind: &str) -> Result<()> {
    let yml = dir.join(format!("{name}.yml"));
    let subdir = dir.join(name);
    if !yml.exists() {
        return Err(anyhow!("{kind} '{name}' not found"));
    }
    fs::remove_file(&yml)?;
    if subdir.exists() {
        fs::remove_dir_all(&subdir)?;
    }
    Ok(())
}

impl FilesystemCharacterRepository {
    #[must_use]
    pub fn new(workspace_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: workspace_dir.as_ref().join("data").join("characters"),
        }
    }

    fn metadata_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!("{name}.yml"))
    }

    fn child_dir(&self, name: &str) -> PathBuf {
        self.base_dir.join(name)
    }

    fn sprite_groups_dir(&self, name: &str) -> PathBuf {
        self.child_dir(name).join("sprite-groups")
    }

    fn animations_dir(&self, name: &str) -> PathBuf {
        self.child_dir(name).join("animations")
    }

    fn sound_groups_dir(&self, name: &str) -> PathBuf {
        self.child_dir(name).join("sound-groups")
    }

    /// `{character}/sprite-groups/{group}/sprites/` を返す。画像 import / delete /
    /// backup 系の操作で共通に使うパス。
    fn sprite_image_dir(&self, character_name: &str, sprite_group_name: &str) -> PathBuf {
        self.sprite_groups_dir(character_name)
            .join(sprite_group_name)
            .join("sprites")
    }

    /// `{character}/sound-groups/{group}/sounds/` を返す。wav import / delete で使うパス。
    fn sound_file_dir(&self, character_name: &str, sound_group_name: &str) -> PathBuf {
        self.sound_groups_dir(character_name)
            .join(sound_group_name)
            .join("sounds")
    }

    fn read_character_metadata(path: &Path) -> Result<Character> {
        let content = fs::read_to_string(path)?;
        let character: Character = serde_saphyr::from_str(&content)?;
        Ok(character)
    }

    fn write_character(&self, character: &Character) -> Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        let yaml = serde_saphyr::to_string(character)?;
        fs::write(self.metadata_path(&character.name), yaml)?;

        // sprite-groups/ と animations/ の直下 *.yml だけを更新する。
        // サブディレクトリ（{group}/sprites/*.png 等の実バイナリ）は別オペレーションの
        // 管轄なので触らない（remove_dir_all で丸ごと消すと画像もろとも吹き飛ぶ）。
        sync_yaml_dir(
            &self.sprite_groups_dir(&character.name),
            &character.sprite_groups,
            |g| g.name.as_str(),
        )?;
        sync_yaml_dir(
            &self.animations_dir(&character.name),
            &character.animations,
            |a| a.name.as_str(),
        )?;
        sync_yaml_dir(
            &self.sound_groups_dir(&character.name),
            &character.sound_groups,
            |g| g.name.as_str(),
        )?;
        Ok(())
    }
}

impl CharacterRepository for FilesystemCharacterRepository {
    fn list(&self) -> Result<Vec<Character>> {
        read_yaml_dir(&self.base_dir)
    }

    fn get(&self, name: &str) -> Result<Option<Character>> {
        let metadata_path = self.metadata_path(name);
        if !metadata_path.exists() {
            return Ok(None);
        }
        let mut character = Self::read_character_metadata(&metadata_path)?;
        let mut sprite_groups: Vec<SpriteGroup> = read_yaml_dir(&self.sprite_groups_dir(name))?;
        sprite_groups.sort_by_key(|g| g.number);
        // 各 sprite の dimensions を PNG header から取得して埋める。
        // SpriteCanvas / AnimationCanvas は CSS の transform: scale(zoom) を使わず、image
        // の width/height を `naturalSize × zoom` の CSS px で explicit に書く方針なので、
        // Rust 側で natural width/height を知っている必要がある (4K + 150% スケール対策。
        // 詳細は widgets/character/ui/README.md の「4K + 非整数 DPR」節)。
        // PNG が壊れてる / 別形式の場合は dimensions = None のままにして UI 側で fallback。
        for group in &mut sprite_groups {
            for sprite in &mut group.sprites {
                let path = self.sprite_image_dir(name, &group.name).join(&sprite.path);
                sprite.dimensions = crate::shared::read_png_dimensions(&path).ok();
            }
        }
        character.sprite_groups = sprite_groups;
        let mut animations: Vec<Animation> = read_yaml_dir(&self.animations_dir(name))?;
        // Animation の sort 順は yaml ファイル名 (Animation.name) 順。
        // number フィールドは廃止済みなので、名前順で安定した一覧を返す。
        animations.sort_by(|a, b| a.name.cmp(&b.name));
        character.animations = animations;
        let mut sound_groups: Vec<SoundGroup> = read_yaml_dir(&self.sound_groups_dir(name))?;
        sound_groups.sort_by_key(|g| g.number);
        character.sound_groups = sound_groups;
        Ok(Some(character))
    }

    fn create(&self, character: &Character) -> Result<()> {
        if self.metadata_path(&character.name).exists() {
            return Err(anyhow!("Character '{}' already exists", character.name));
        }
        self.write_character(character)
    }

    fn update(&self, character: &Character) -> Result<()> {
        if !self.metadata_path(&character.name).exists() {
            return Err(anyhow!("Character '{}' not found", character.name));
        }
        self.write_character(character)
    }

    fn update_metadata(&self, character: &Character) -> Result<()> {
        let path = self.metadata_path(&character.name);
        if !path.exists() {
            return Err(anyhow!("Character '{}' not found", character.name));
        }
        let yaml = serde_saphyr::to_string(character)?;
        fs::write(path, yaml)?;
        Ok(())
    }

    fn rename(&self, old_name: &str, new_name: &str) -> Result<()> {
        rename_yaml_aggregate::<Character>(
            &self.base_dir,
            old_name,
            new_name,
            "Character",
            |c, n| c.name = n,
        )
    }

    fn delete(&self, name: &str) -> Result<()> {
        delete_yaml_aggregate(&self.base_dir, name, "Character")
    }

    fn import_sprite_image(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        source: &Path,
    ) -> Result<String> {
        let basename = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("source path has no valid file name"))?
            .to_string();

        let target_dir = self.sprite_image_dir(character_name, sprite_group_name);
        fs::create_dir_all(&target_dir)?;

        fs::copy(source, target_dir.join(&basename))?;
        Ok(basename)
    }

    fn delete_sprite_image(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        basename: &str,
    ) -> Result<()> {
        let target = self
            .sprite_image_dir(character_name, sprite_group_name)
            .join(basename);
        if target.exists() {
            fs::remove_file(target)?;
        }
        Ok(())
    }

    fn import_sprite_image_with_backup(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        source: &Path,
    ) -> Result<ImportOutcome> {
        let basename = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("source path has no valid file name"))?
            .to_string();

        let target_dir = self.sprite_image_dir(character_name, sprite_group_name);
        fs::create_dir_all(&target_dir)?;

        let target = target_dir.join(&basename);
        let backup = target_dir.join(format!("{basename}.bak"));

        // 既存 backup が残っていたら（前回 rollback 漏れ）潰しておく：今回の新規 backup と
        // 衝突しないため、また「最新の旧ファイル」が必ず backup に入るよう保証するため。
        if backup.exists() {
            fs::remove_file(&backup)?;
        }

        let outcome = if target.exists() {
            fs::rename(&target, &backup)?;
            ImportOutcome::Overwrote {
                basename: basename.clone(),
            }
        } else {
            ImportOutcome::Created {
                basename: basename.clone(),
            }
        };

        // 失敗したら backup を戻して例外を伝播する。
        match fs::copy(source, &target) {
            Ok(_) => Ok(outcome),
            Err(e) => {
                if backup.exists() {
                    let _ = fs::rename(&backup, &target);
                }
                Err(anyhow!("failed to copy sprite image: {e}"))
            }
        }
    }

    fn restore_sprite_image_backup(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        basename: &str,
    ) -> Result<()> {
        let dir = self.sprite_image_dir(character_name, sprite_group_name);
        let target = dir.join(basename);
        let backup = dir.join(format!("{basename}.bak"));
        if !backup.exists() {
            return Ok(());
        }
        // 新ファイル（取り込み後のもの）を消してから backup を戻す。
        if target.exists() {
            fs::remove_file(&target)?;
        }
        fs::rename(&backup, &target)?;
        Ok(())
    }

    fn discard_sprite_image_backup(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        basename: &str,
    ) -> Result<()> {
        let backup = self
            .sprite_image_dir(character_name, sprite_group_name)
            .join(format!("{basename}.bak"));
        if backup.exists() {
            fs::remove_file(backup)?;
        }
        Ok(())
    }

    fn rename_sprite_group(
        &self,
        character_name: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<()> {
        rename_yaml_aggregate::<SpriteGroup>(
            &self.sprite_groups_dir(character_name),
            old_name,
            new_name,
            "SpriteGroup",
            |g, n| g.name = n,
        )
    }

    fn delete_sprite_group(&self, character_name: &str, sprite_group_name: &str) -> Result<()> {
        delete_yaml_aggregate(
            &self.sprite_groups_dir(character_name),
            sprite_group_name,
            "SpriteGroup",
        )
    }

    fn rename_animation(&self, character_name: &str, old_name: &str, new_name: &str) -> Result<()> {
        rename_yaml_aggregate::<Animation>(
            &self.animations_dir(character_name),
            old_name,
            new_name,
            "Animation",
            |a, n| a.name = n,
        )
    }

    fn delete_animation(&self, character_name: &str, animation_name: &str) -> Result<()> {
        delete_yaml_aggregate(
            &self.animations_dir(character_name),
            animation_name,
            "Animation",
        )
    }

    fn update_animation(&self, character_name: &str, animation: &Animation) -> Result<()> {
        let dir = self.animations_dir(character_name);
        let target_path = dir.join(format!("{}.yml", animation.name));
        if !target_path.exists() {
            return Err(anyhow!(
                "Animation '{}' not found for character '{character_name}'",
                animation.name,
            ));
        }
        let yaml = serde_saphyr::to_string(animation)?;
        fs::write(&target_path, yaml)?;
        Ok(())
    }

    fn rename_sound_group(
        &self,
        character_name: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<()> {
        rename_yaml_aggregate::<SoundGroup>(
            &self.sound_groups_dir(character_name),
            old_name,
            new_name,
            "SoundGroup",
            |g, n| g.name = n,
        )
    }

    fn delete_sound_group(&self, character_name: &str, sound_group_name: &str) -> Result<()> {
        delete_yaml_aggregate(
            &self.sound_groups_dir(character_name),
            sound_group_name,
            "SoundGroup",
        )
    }

    fn update_sound_group(&self, character_name: &str, sound_group: &SoundGroup) -> Result<()> {
        let dir = self.sound_groups_dir(character_name);
        let target_path = dir.join(format!("{}.yml", sound_group.name));
        if !target_path.exists() {
            return Err(anyhow!(
                "SoundGroup '{}' not found for character '{character_name}'",
                sound_group.name,
            ));
        }
        let yaml = serde_saphyr::to_string(sound_group)?;
        fs::write(&target_path, yaml)?;

        // 参照されない wav を sounds/ から削除して orphan を防ぐ。これによって yml = disk が
        // Save 時点で必ず一致する。fail-soft: 個別ファイルの削除失敗はログだけ残して続行する。
        let sounds_dir = self.sound_file_dir(character_name, &sound_group.name);
        if sounds_dir.exists() {
            let referenced: HashSet<&str> =
                sound_group.sounds.iter().map(|s| s.path.as_str()).collect();
            for entry in fs::read_dir(&sounds_dir)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if !referenced.contains(name)
                    && let Err(e) = fs::remove_file(&path)
                {
                    tracing::warn!("orphan wav の削除に失敗: {} ({e})", path.display());
                }
            }
        }
        Ok(())
    }

    fn import_sound_file(
        &self,
        character_name: &str,
        sound_group_name: &str,
        source: &Path,
    ) -> Result<String> {
        let basename = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("source path has no valid file name"))?
            .to_string();

        let target_dir = self.sound_file_dir(character_name, sound_group_name);
        fs::create_dir_all(&target_dir)?;
        let target = target_dir.join(&basename);

        // 既存ファイルがあれば error。Cancel 時の rollback で committed 済み wav を消す
        // 事故（pending_imports に積まれた basename を `delete_sound_file` で消すと、もし
        // 同名 committed wav を上書きしていた場合は復元不能）を防ぐ。
        if target.exists() {
            return Err(anyhow!(
                "'{basename}' は既に sounds/ に存在します。先に Sound を削除して保存するか、別ファイル名でインポートしてください"
            ));
        }
        fs::copy(source, &target)?;
        Ok(basename)
    }

    fn delete_sound_file(
        &self,
        character_name: &str,
        sound_group_name: &str,
        basename: &str,
    ) -> Result<()> {
        let target = self
            .sound_file_dir(character_name, sound_group_name)
            .join(basename);
        if target.exists() {
            fs::remove_file(target)?;
        }
        Ok(())
    }

    fn read_sound_metadata(
        &self,
        character_name: &str,
        sound_group_name: &str,
        basename: &str,
    ) -> Result<WavInfo> {
        let path = self
            .sound_file_dir(character_name, sound_group_name)
            .join(basename);
        read_wav_info(&path)
    }
}
