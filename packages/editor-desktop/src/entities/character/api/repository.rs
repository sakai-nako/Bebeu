use std::path::Path;

use anyhow::Result;

use crate::entities::character::{Animation, Character, SoundGroup};
use crate::shared::WavInfo;

/// `import_sprite_image_with_backup` の戻り値。Created なら新規ファイル、
/// Overwrote なら既存 basename と同名で上書き発生（`{basename}.bak` に旧ファイルが退避された）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportOutcome {
    Created { basename: String },
    Overwrote { basename: String },
}

impl ImportOutcome {
    #[must_use]
    pub fn basename(&self) -> &str {
        match self {
            Self::Created { basename } | Self::Overwrote { basename } => basename,
        }
    }
}

pub trait CharacterRepository: Send + Sync {
    /// Character 一覧を返す。`sprite_groups` は **空のまま** で返る（一覧軽量化）。
    fn list(&self) -> Result<Vec<Character>>;
    /// 単一 Character を取得。`sprite_groups` も完全にロードされる。
    fn get(&self, name: &str) -> Result<Option<Character>>;
    /// 既存なら error。character.yml + sprite-groups/*.yml を一括書き込み。
    fn create(&self, character: &Character) -> Result<()>;
    /// 不在なら error。character.yml + sprite-groups/*.yml を一括書き直し。
    fn update(&self, character: &Character) -> Result<()>;
    /// {name}.yml だけを書き直す軽量更新。sprite-groups/ は触らない。
    /// inline 編集など、Character 自身のフィールドだけを変更する用途。
    fn update_metadata(&self, character: &Character) -> Result<()>;
    /// {old_name}.yml と {old_name}/ ディレクトリを {new_name}.yml / {new_name}/ に移動し、
    /// yml 内の name フィールドも更新する。重複や不在は error。
    fn rename(&self, old_name: &str, new_name: &str) -> Result<()>;
    /// character.yml と {name}/ ディレクトリを削除。
    fn delete(&self, name: &str) -> Result<()>;

    /// 外部画像を `{workspace}/data/characters/{character_name}/sprite-groups/{sprite_group_name}/sprites/`
    /// にコピーし、SpriteGroup の `Sprite.path` に書く basename を返す。同名ファイルは上書き。
    fn import_sprite_image(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        source: &Path,
    ) -> Result<String>;

    /// `import_sprite_image` で生成したファイルを削除する（create 失敗時のロールバック用）。
    fn delete_sprite_image(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        basename: &str,
    ) -> Result<()>;

    /// 外部画像を取り込むが、対象 basename が既に存在すれば事前に `{basename}.bak` に
    /// rename してバックアップを作る。戻り値で「新規作成」「上書き」を区別する。
    /// `restore_sprite_image_backup` / `discard_sprite_image_backup` と組で使い、
    /// 倍率再インポートのような上書き操作の atomicity を担保する。
    fn import_sprite_image_with_backup(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        source: &Path,
    ) -> Result<ImportOutcome>;

    /// `import_sprite_image_with_backup` で作られたバックアップ（`{basename}.bak`）を
    /// `{basename}` に戻す。Cancel / unmount の rollback 経路で使う。
    /// バックアップが無ければ no-op。
    fn restore_sprite_image_backup(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        basename: &str,
    ) -> Result<()>;

    /// `import_sprite_image_with_backup` で作られたバックアップ（`{basename}.bak`）を
    /// 削除して上書きを確定する。Save commit 経路で使う。バックアップが無ければ no-op。
    fn discard_sprite_image_backup(
        &self,
        character_name: &str,
        sprite_group_name: &str,
        basename: &str,
    ) -> Result<()>;

    /// SpriteGroup をリネーム。
    /// {character}/sprite-groups/{old}.yml → {new}.yml （内容の name フィールドも書き換え）、
    /// {character}/sprite-groups/{old}/ → {new}/ （画像ディレクトリも移動）。
    /// 重複・不在は error。
    fn rename_sprite_group(
        &self,
        character_name: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<()>;

    /// SpriteGroup を削除。
    /// {character}/sprite-groups/{name}.yml と {name}/ ディレクトリ（画像含む）を丸ごと削除。
    /// Layer の sprite_group_number 参照は validation しない（UI 側の警告で対応）。
    fn delete_sprite_group(&self, character_name: &str, sprite_group_name: &str) -> Result<()>;

    /// Animation をリネーム。{old}.yml → {new}.yml + name フィールド更新。
    /// 将来 {old}/ ディレクトリが存在すれば {new}/ に移動する。
    fn rename_animation(&self, character_name: &str, old_name: &str, new_name: &str) -> Result<()>;

    /// Animation を削除。{name}.yml と存在すれば {name}/ ディレクトリを削除。
    fn delete_animation(&self, character_name: &str, animation_name: &str) -> Result<()>;

    /// 単一 Animation の YAML を上書き保存する。AnimationEditor の Save ライフサイクル用。
    /// {character}/animations/{animation.name}.yml を直接書き直し、
    /// 他の animations/*.yml や character.yml には触らない。
    /// 対象 yml が存在しなければ error（rename とは責務を分ける）。
    fn update_animation(&self, character_name: &str, animation: &Animation) -> Result<()>;

    /// SoundGroup をリネーム。
    /// {character}/sound-groups/{old}.yml → {new}.yml （内容の name フィールドも書き換え）、
    /// {character}/sound-groups/{old}/ → {new}/ （wav 入りディレクトリも移動）。
    /// 重複・不在は error。
    fn rename_sound_group(
        &self,
        character_name: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<()>;

    /// SoundGroup を削除。
    /// {character}/sound-groups/{name}.yml と {name}/ ディレクトリ（wav 含む）を丸ごと削除。
    /// Frame.sound 参照は validation しない（UI 側の警告で対応）。
    fn delete_sound_group(&self, character_name: &str, sound_group_name: &str) -> Result<()>;

    /// 単一 SoundGroup の YAML を上書き保存する。SoundGroupEditor の Save ライフサイクル用。
    /// {character}/sound-groups/{sound_group.name}.yml を直接書き直し、
    /// 他の sound-groups/*.yml や character.yml には触らない。
    /// 対象 yml が存在しなければ error（rename とは責務を分ける）。
    ///
    /// **副作用**: `{sound_group.name}/sounds/` 配下の wav ファイルのうち、
    /// `sound_group.sounds[*].path` に含まれないものを削除する（orphan 防止）。
    /// これにより Save 時点で yml と disk が必ず一致する。
    fn update_sound_group(&self, character_name: &str, sound_group: &SoundGroup) -> Result<()>;

    /// 外部 wav ファイルを `{workspace}/data/characters/{character_name}/sound-groups/{sound_group_name}/sounds/`
    /// にコピーし、`Sound.path` に書く basename を返す。
    ///
    /// 同名ファイルが既に存在する場合は **error**（上書きしない）。これは Cancel 時の
    /// rollback で committed 済みファイルを誤削除する事故を防ぐため。同名で取り込みたい場合は
    /// 先に Sound を削除して保存（disk からも自動で消える）した後に再 import する。
    fn import_sound_file(
        &self,
        character_name: &str,
        sound_group_name: &str,
        source: &Path,
    ) -> Result<String>;

    /// `import_sound_file` で生成したファイルを削除する（Cancel 時のロールバック用）。
    fn delete_sound_file(
        &self,
        character_name: &str,
        sound_group_name: &str,
        basename: &str,
    ) -> Result<()>;

    /// `{character}/sound-groups/{sound_group}/sounds/{basename}` の WAV ヘッダから
    /// メタデータ（sample_rate / channels / bits / duration）を読む。SoundGroupEditor
    /// の表示用。InMemory 実装はファイル実体が無いので常に error を返す。
    fn read_sound_metadata(
        &self,
        character_name: &str,
        sound_group_name: &str,
        basename: &str,
    ) -> Result<WavInfo>;
}
