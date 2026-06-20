use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use anyhow::{Result, anyhow};

use super::repository::{CharacterRepository, ImportOutcome};
use crate::entities::character::{Animation, Character, SoundGroup};
use crate::shared::WavInfo;

pub struct InMemoryCharacterRepository {
    storage: RwLock<HashMap<String, Character>>,
}

impl InMemoryCharacterRepository {
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryCharacterRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl CharacterRepository for InMemoryCharacterRepository {
    fn list(&self) -> Result<Vec<Character>> {
        let storage = self.storage.read().expect("RwLock poisoned");
        Ok(storage
            .values()
            .map(|c| Character {
                sprite_groups: Vec::new(),
                animations: Vec::new(),
                sound_groups: Vec::new(),
                ..c.clone()
            })
            .collect())
    }

    fn get(&self, name: &str) -> Result<Option<Character>> {
        let storage = self.storage.read().expect("RwLock poisoned");
        Ok(storage.get(name).cloned())
    }

    fn create(&self, character: &Character) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        if storage.contains_key(&character.name) {
            return Err(anyhow!("Character '{}' already exists", character.name));
        }
        storage.insert(character.name.clone(), character.clone());
        Ok(())
    }

    fn update(&self, character: &Character) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        if !storage.contains_key(&character.name) {
            return Err(anyhow!("Character '{}' not found", character.name));
        }
        storage.insert(character.name.clone(), character.clone());
        Ok(())
    }

    fn update_metadata(&self, character: &Character) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        let existing = storage
            .get_mut(&character.name)
            .ok_or_else(|| anyhow!("Character '{}' not found", character.name))?;
        // sprite_groups / animations / sound_groups は既存を保持し、metadata だけ上書きする。
        existing
            .thumbnail_path
            .clone_from(&character.thumbnail_path);
        existing.hp = character.hp;
        existing.depth = character.depth;
        Ok(())
    }

    fn rename(&self, old_name: &str, new_name: &str) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        if storage.contains_key(new_name) {
            return Err(anyhow!("Character '{new_name}' already exists"));
        }
        let mut character = storage
            .remove(old_name)
            .ok_or_else(|| anyhow!("Character '{old_name}' not found"))?;
        character.name = new_name.to_string();
        storage.insert(new_name.to_string(), character);
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        if storage.remove(name).is_none() {
            return Err(anyhow!("Character '{name}' not found"));
        }
        Ok(())
    }

    fn import_sprite_image(
        &self,
        _character_name: &str,
        _sprite_group_name: &str,
        source: &Path,
    ) -> Result<String> {
        // InMemory はファイルを置かないので basename だけ返す
        source
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from)
            .ok_or_else(|| anyhow!("source path has no valid file name"))
    }

    fn delete_sprite_image(
        &self,
        _character_name: &str,
        _sprite_group_name: &str,
        _basename: &str,
    ) -> Result<()> {
        // InMemory はファイルを置かないので no-op
        Ok(())
    }

    fn import_sprite_image_with_backup(
        &self,
        _character_name: &str,
        _sprite_group_name: &str,
        source: &Path,
    ) -> Result<ImportOutcome> {
        // InMemory はファイルを置かないので Created を返すだけ
        let basename = source
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from)
            .ok_or_else(|| anyhow!("source path has no valid file name"))?;
        Ok(ImportOutcome::Created { basename })
    }

    fn restore_sprite_image_backup(
        &self,
        _character_name: &str,
        _sprite_group_name: &str,
        _basename: &str,
    ) -> Result<()> {
        Ok(())
    }

    fn discard_sprite_image_backup(
        &self,
        _character_name: &str,
        _sprite_group_name: &str,
        _basename: &str,
    ) -> Result<()> {
        Ok(())
    }

    fn rename_sprite_group(
        &self,
        character_name: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        let character = storage
            .get_mut(character_name)
            .ok_or_else(|| anyhow!("Character '{character_name}' not found"))?;
        if character.sprite_groups.iter().any(|g| g.name == new_name) {
            return Err(anyhow!("SpriteGroup '{new_name}' already exists"));
        }
        let group = character
            .sprite_groups
            .iter_mut()
            .find(|g| g.name == old_name)
            .ok_or_else(|| anyhow!("SpriteGroup '{old_name}' not found"))?;
        group.name = new_name.to_string();
        Ok(())
    }

    fn delete_sprite_group(&self, character_name: &str, sprite_group_name: &str) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        let character = storage
            .get_mut(character_name)
            .ok_or_else(|| anyhow!("Character '{character_name}' not found"))?;
        let before = character.sprite_groups.len();
        character
            .sprite_groups
            .retain(|g| g.name != sprite_group_name);
        if character.sprite_groups.len() == before {
            return Err(anyhow!("SpriteGroup '{sprite_group_name}' not found"));
        }
        Ok(())
    }

    fn rename_animation(&self, character_name: &str, old_name: &str, new_name: &str) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        let character = storage
            .get_mut(character_name)
            .ok_or_else(|| anyhow!("Character '{character_name}' not found"))?;
        if character.animations.iter().any(|a| a.name == new_name) {
            return Err(anyhow!("Animation '{new_name}' already exists"));
        }
        let animation = character
            .animations
            .iter_mut()
            .find(|a| a.name == old_name)
            .ok_or_else(|| anyhow!("Animation '{old_name}' not found"))?;
        animation.name = new_name.to_string();
        Ok(())
    }

    fn delete_animation(&self, character_name: &str, animation_name: &str) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        let character = storage
            .get_mut(character_name)
            .ok_or_else(|| anyhow!("Character '{character_name}' not found"))?;
        let before = character.animations.len();
        character.animations.retain(|a| a.name != animation_name);
        if character.animations.len() == before {
            return Err(anyhow!("Animation '{animation_name}' not found"));
        }
        Ok(())
    }

    fn update_animation(&self, character_name: &str, animation: &Animation) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        let character = storage
            .get_mut(character_name)
            .ok_or_else(|| anyhow!("Character '{character_name}' not found"))?;
        let slot = character
            .animations
            .iter_mut()
            .find(|a| a.name == animation.name)
            .ok_or_else(|| anyhow!("Animation '{}' not found", animation.name))?;
        *slot = animation.clone();
        Ok(())
    }

    fn rename_sound_group(
        &self,
        character_name: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        let character = storage
            .get_mut(character_name)
            .ok_or_else(|| anyhow!("Character '{character_name}' not found"))?;
        if character.sound_groups.iter().any(|g| g.name == new_name) {
            return Err(anyhow!("SoundGroup '{new_name}' already exists"));
        }
        let group = character
            .sound_groups
            .iter_mut()
            .find(|g| g.name == old_name)
            .ok_or_else(|| anyhow!("SoundGroup '{old_name}' not found"))?;
        group.name = new_name.to_string();
        Ok(())
    }

    fn delete_sound_group(&self, character_name: &str, sound_group_name: &str) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        let character = storage
            .get_mut(character_name)
            .ok_or_else(|| anyhow!("Character '{character_name}' not found"))?;
        let before = character.sound_groups.len();
        character
            .sound_groups
            .retain(|g| g.name != sound_group_name);
        if character.sound_groups.len() == before {
            return Err(anyhow!("SoundGroup '{sound_group_name}' not found"));
        }
        Ok(())
    }

    fn update_sound_group(&self, character_name: &str, sound_group: &SoundGroup) -> Result<()> {
        let mut storage = self.storage.write().expect("RwLock poisoned");
        let character = storage
            .get_mut(character_name)
            .ok_or_else(|| anyhow!("Character '{character_name}' not found"))?;
        let slot = character
            .sound_groups
            .iter_mut()
            .find(|g| g.name == sound_group.name)
            .ok_or_else(|| anyhow!("SoundGroup '{}' not found", sound_group.name))?;
        *slot = sound_group.clone();
        Ok(())
    }

    fn import_sound_file(
        &self,
        _character_name: &str,
        _sound_group_name: &str,
        source: &Path,
    ) -> Result<String> {
        // InMemory はファイルを置かないので basename だけ返す
        source
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from)
            .ok_or_else(|| anyhow!("source path has no valid file name"))
    }

    fn delete_sound_file(
        &self,
        _character_name: &str,
        _sound_group_name: &str,
        _basename: &str,
    ) -> Result<()> {
        // InMemory はファイルを置かないので no-op
        Ok(())
    }

    fn read_sound_metadata(
        &self,
        _character_name: &str,
        _sound_group_name: &str,
        _basename: &str,
    ) -> Result<WavInfo> {
        // InMemory はファイル実体が無いので呼ばれない想定。SoundGroupEditor の UI 側で
        // Result が err なら "—" 表示にフォールバックする。
        Err(anyhow!("InMemory repository does not store wav files"))
    }
}
