use std::fs;

use anyhow::Result;

use super::{
    CharacterRepository, FilesystemCharacterRepository, ImportOutcome, InMemoryCharacterRepository,
};
use crate::entities::character::{
    Animation, Character, CharacterPhysics, DEFAULT_CHARACTER_DEPTH, Frame, FrameSound, Role,
    Sound, SoundGroup, SpriteGroup,
};

fn sample_sprite_group(name: &str, number: u32) -> SpriteGroup {
    SpriteGroup {
        name: name.to_string(),
        number,
        sprites: Vec::new(),
    }
}

fn sample_character(name: &str, hp: u32, sprite_groups: Vec<SpriteGroup>) -> Character {
    Character {
        name: name.to_string(),
        thumbnail_path: format!("sprite-groups/thumbnail/sprites/{name}.png"),
        hp,
        depth: DEFAULT_CHARACTER_DEPTH,
        physics: CharacterPhysics::default(),
        sprite_groups,
        animations: Vec::new(),
        sound_groups: Vec::new(),
    }
}

fn sample_animation(name: &str) -> Animation {
    Animation {
        name: name.to_string(),
        role: Role::Custom,
        variant: 0,
        export_number: None,
        is_loop: true,
        loop_start_index: 0,
        frames: Vec::new(),
    }
}

fn sample_animation_with_role(name: &str, role: Role, variant: u32) -> Animation {
    Animation {
        name: name.to_string(),
        role,
        variant,
        export_number: None,
        is_loop: true,
        loop_start_index: 0,
        frames: Vec::new(),
    }
}

fn sample_character_with_animations(
    name: &str,
    hp: u32,
    sprite_groups: Vec<SpriteGroup>,
    animations: Vec<Animation>,
) -> Character {
    Character {
        name: name.to_string(),
        thumbnail_path: format!("sprite-groups/thumbnail/sprites/{name}.png"),
        hp,
        depth: DEFAULT_CHARACTER_DEPTH,
        physics: CharacterPhysics::default(),
        sprite_groups,
        animations,
        sound_groups: Vec::new(),
    }
}

fn run_repository_scenarios<R: CharacterRepository>(repo: &R) -> Result<()> {
    assert!(repo.list()?.is_empty());
    assert!(repo.get("foo")?.is_none());

    let foo = sample_character("foo", 100, vec![sample_sprite_group("walk", 1)]);
    repo.create(&foo)?;
    assert_eq!(repo.list()?.len(), 1);
    assert_eq!(repo.get("foo")?, Some(foo.clone()));

    // 重複 create はエラー
    assert!(repo.create(&foo).is_err());

    // update で sprite_groups を 1 つ追加
    let foo_updated = sample_character(
        "foo",
        120,
        vec![
            sample_sprite_group("walk", 1),
            sample_sprite_group("idle", 2),
        ],
    );
    repo.update(&foo_updated)?;
    assert_eq!(repo.get("foo")?, Some(foo_updated));

    // 不在 update はエラー
    assert!(repo.update(&sample_character("bar", 0, vec![])).is_err());

    // 別 Character を追加
    repo.create(&sample_character("bar", 200, vec![]))?;
    let mut listed: Vec<String> = repo.list()?.into_iter().map(|c| c.name).collect();
    listed.sort();
    assert_eq!(listed, vec!["bar".to_string(), "foo".to_string()]);

    // delete
    repo.delete("foo")?;
    assert!(repo.get("foo")?.is_none());
    assert_eq!(repo.list()?.len(), 1);

    // 不在 delete はエラー
    assert!(repo.delete("foo").is_err());

    Ok(())
}

#[test]
fn in_memory_repository_satisfies_contract() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    run_repository_scenarios(&repo)
}

#[test]
fn filesystem_repository_satisfies_contract() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    run_repository_scenarios(&repo)
}

#[test]
fn list_returns_characters_without_sprite_groups() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character(
        "foo",
        10,
        vec![sample_sprite_group("walk", 1)],
    ))?;
    let listed = repo.list()?;
    assert_eq!(listed.len(), 1);
    assert!(listed[0].sprite_groups.is_empty());
    Ok(())
}

#[test]
fn get_loads_sprite_groups() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    let walk = sample_sprite_group("walk", 1);
    repo.create(&sample_character("foo", 10, vec![walk.clone()]))?;
    let got = repo.get("foo")?.expect("character should exist");
    assert_eq!(got.sprite_groups, vec![walk]);
    Ok(())
}

#[test]
fn filesystem_create_writes_character_and_sprite_groups() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![
            sample_sprite_group("walk", 1),
            sample_sprite_group("idle", 2),
        ],
    ))?;
    let chars_dir = workspace.path().join("data").join("characters");
    assert!(chars_dir.join("foo.yml").exists());
    assert!(
        chars_dir
            .join("foo")
            .join("sprite-groups")
            .join("walk.yml")
            .exists()
    );
    assert!(
        chars_dir
            .join("foo")
            .join("sprite-groups")
            .join("idle.yml")
            .exists()
    );
    Ok(())
}

#[test]
fn filesystem_get_loads_sprite_groups_in_number_order() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![
            sample_sprite_group("idle", 2),
            sample_sprite_group("walk", 1),
        ],
    ))?;
    let got = repo.get("foo")?.expect("character should exist");
    let names: Vec<_> = got.sprite_groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, vec!["walk", "idle"]); // number=1 が先
    Ok(())
}

#[test]
fn filesystem_update_preserves_sprite_image_files() -> Result<()> {
    // Character.update() で sprite_groups/{group}/sprites/*.png 等の実バイナリが
    // 巻き添えで消えないこと。リグレッションテスト。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    let walk = sample_sprite_group("walk", 1);
    repo.create(&sample_character("foo", 10, vec![walk.clone()]))?;

    let sg_dir = workspace.path().join("data/characters/foo/sprite-groups");
    let image_path = sg_dir.join("walk/sprites/walk_001.png");
    fs::create_dir_all(image_path.parent().expect("image_path has a parent"))?;
    fs::write(&image_path, b"fake-png-bytes")?;

    repo.update(&sample_character("foo", 999, vec![walk]))?;

    assert!(
        image_path.exists(),
        "sprite image was wiped by Character.update()"
    );
    assert!(
        sg_dir.join("walk.yml").exists(),
        "walk.yml should be re-written"
    );
    Ok(())
}

#[test]
fn filesystem_update_removes_yml_for_dropped_sprite_group() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![
            sample_sprite_group("walk", 1),
            sample_sprite_group("idle", 2),
        ],
    ))?;

    let sg_dir = workspace.path().join("data/characters/foo/sprite-groups");
    assert!(sg_dir.join("walk.yml").exists());
    assert!(sg_dir.join("idle.yml").exists());

    // idle を外して update
    repo.update(&sample_character(
        "foo",
        10,
        vec![sample_sprite_group("walk", 1)],
    ))?;

    assert!(sg_dir.join("walk.yml").exists());
    assert!(
        !sg_dir.join("idle.yml").exists(),
        "yml of dropped sprite_group should be removed"
    );
    Ok(())
}

#[test]
fn filesystem_import_sprite_image_copies_file_and_returns_basename() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    // 元画像を一時的に作成
    let source = workspace.path().join("source.png");
    fs::write(&source, b"png-bytes")?;

    let basename = repo.import_sprite_image("foo", "thumbnail", &source)?;
    assert_eq!(basename, "source.png");

    let target = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png");
    assert!(target.exists(), "target file should be created");
    assert_eq!(fs::read(&target)?, b"png-bytes");
    Ok(())
}

#[test]
fn filesystem_update_metadata_does_not_touch_sprite_groups() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![sample_sprite_group("walk", 1)],
    ))?;

    // 子集約ディレクトリに画像を置いておく
    let sg_dir = workspace.path().join("data/characters/foo/sprite-groups");
    let image_path = sg_dir.join("walk/sprites/walk_001.png");
    fs::create_dir_all(image_path.parent().expect("image_path has a parent"))?;
    fs::write(&image_path, b"fake-png-bytes")?;

    // sprite_groups を空にした Character で update_metadata を呼ぶ
    repo.update_metadata(&Character {
        name: "foo".into(),
        thumbnail_path: "new-thumb.png".into(),
        hp: 999,
        depth: DEFAULT_CHARACTER_DEPTH,
        physics: CharacterPhysics::default(),
        sprite_groups: Vec::new(),
        animations: Vec::new(),
        sound_groups: Vec::new(),
    })?;

    // {name}.yml は更新されている
    let got = repo.get("foo")?.expect("character should exist");
    assert_eq!(got.hp, 999);
    assert_eq!(got.thumbnail_path, "new-thumb.png");
    // sprite-groups/*.yml も画像も無事
    assert!(sg_dir.join("walk.yml").exists());
    assert!(image_path.exists());
    assert_eq!(got.sprite_groups.len(), 1);
    Ok(())
}

#[test]
fn filesystem_rename_moves_yml_dir_and_updates_name_field() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![sample_sprite_group("walk", 1)],
    ))?;

    repo.rename("foo", "bar")?;

    let chars_dir = workspace.path().join("data/characters");
    assert!(!chars_dir.join("foo.yml").exists());
    assert!(!chars_dir.join("foo").exists());
    assert!(chars_dir.join("bar.yml").exists());
    assert!(chars_dir.join("bar/sprite-groups/walk.yml").exists());

    // yml 内の name フィールドも更新されていること
    let got = repo.get("bar")?.expect("renamed character should exist");
    assert_eq!(got.name, "bar");
    assert_eq!(got.sprite_groups.len(), 1);
    Ok(())
}

#[test]
fn filesystem_rename_preserves_sprite_groups_and_images() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![sample_sprite_group("walk", 1)],
    ))?;

    // 画像も置いておく
    let old_image = workspace
        .path()
        .join("data/characters/foo/sprite-groups/walk/sprites/walk_001.png");
    fs::create_dir_all(old_image.parent().expect("image_path has a parent"))?;
    fs::write(&old_image, b"fake-png-bytes")?;

    repo.rename("foo", "bar")?;

    let new_image = workspace
        .path()
        .join("data/characters/bar/sprite-groups/walk/sprites/walk_001.png");
    assert!(!old_image.exists());
    assert!(new_image.exists());
    assert_eq!(fs::read(&new_image)?, b"fake-png-bytes");
    Ok(())
}

#[test]
fn filesystem_rename_rejects_existing_target() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character("foo", 10, vec![]))?;
    repo.create(&sample_character("bar", 20, vec![]))?;

    let result = repo.rename("foo", "bar");
    assert!(result.is_err(), "rename to existing name should fail");
    // 元のファイルが残っていること
    assert!(workspace.path().join("data/characters/foo.yml").exists());
    Ok(())
}

#[test]
fn in_memory_rename_changes_key_and_name() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character(
        "foo",
        10,
        vec![sample_sprite_group("walk", 1)],
    ))?;

    repo.rename("foo", "bar")?;

    assert!(repo.get("foo")?.is_none());
    let got = repo.get("bar")?.expect("renamed character should exist");
    assert_eq!(got.name, "bar");
    assert_eq!(got.sprite_groups.len(), 1);

    // 不在 rename
    assert!(repo.rename("nope", "baz").is_err());
    // 重複 rename
    repo.create(&sample_character("baz", 30, vec![]))?;
    assert!(repo.rename("bar", "baz").is_err());
    Ok(())
}

#[test]
fn filesystem_import_with_backup_returns_created_for_new_file() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    let source = workspace.path().join("source.png");
    fs::write(&source, b"new-bytes")?;

    let outcome = repo.import_sprite_image_with_backup("foo", "thumbnail", &source)?;
    assert!(matches!(outcome, ImportOutcome::Created { .. }));
    assert_eq!(outcome.basename(), "source.png");

    let target = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png");
    assert!(target.exists());
    // backup は作られない
    let backup = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png.bak");
    assert!(!backup.exists());
    Ok(())
}

#[test]
fn filesystem_import_with_backup_returns_overwrote_for_existing_file() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    // 1 回目: Created
    let source = workspace.path().join("source.png");
    fs::write(&source, b"v1")?;
    repo.import_sprite_image_with_backup("foo", "thumbnail", &source)?;

    // 2 回目: 上書き → Overwrote、backup に v1 が退避される
    fs::write(&source, b"v2")?;
    let outcome = repo.import_sprite_image_with_backup("foo", "thumbnail", &source)?;
    assert!(matches!(outcome, ImportOutcome::Overwrote { .. }));

    let target = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png");
    let backup = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png.bak");
    assert_eq!(fs::read(&target)?, b"v2");
    assert_eq!(fs::read(&backup)?, b"v1");
    Ok(())
}

#[test]
fn filesystem_restore_sprite_image_backup_brings_old_file_back() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    let source = workspace.path().join("source.png");
    fs::write(&source, b"v1")?;
    repo.import_sprite_image_with_backup("foo", "thumbnail", &source)?;
    fs::write(&source, b"v2")?;
    repo.import_sprite_image_with_backup("foo", "thumbnail", &source)?;

    repo.restore_sprite_image_backup("foo", "thumbnail", "source.png")?;

    let target = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png");
    let backup = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png.bak");
    assert_eq!(fs::read(&target)?, b"v1", "old file should be restored");
    assert!(!backup.exists(), "backup should be consumed by restore");
    Ok(())
}

#[test]
fn filesystem_restore_sprite_image_backup_is_noop_without_backup() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    // バックアップが無い状態でも error にならないこと
    repo.restore_sprite_image_backup("foo", "thumbnail", "missing.png")?;
    Ok(())
}

#[test]
fn filesystem_discard_sprite_image_backup_removes_bak_only() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    let source = workspace.path().join("source.png");
    fs::write(&source, b"v1")?;
    repo.import_sprite_image_with_backup("foo", "thumbnail", &source)?;
    fs::write(&source, b"v2")?;
    repo.import_sprite_image_with_backup("foo", "thumbnail", &source)?;

    repo.discard_sprite_image_backup("foo", "thumbnail", "source.png")?;

    let target = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png");
    let backup = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png.bak");
    assert_eq!(fs::read(&target)?, b"v2", "new file should remain");
    assert!(!backup.exists(), "backup should be removed");
    Ok(())
}

#[test]
fn filesystem_import_with_backup_overwrites_stale_backup() -> Result<()> {
    // 直前のセッションで rollback 漏れがあって `.bak` が残っているケース。
    // 新しい backup を作る前に古い backup を捨てて、最新の旧ファイルが必ず .bak に入るようにする。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    let sprites_dir = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites");
    fs::create_dir_all(&sprites_dir)?;
    fs::write(sprites_dir.join("source.png"), b"current")?;
    fs::write(sprites_dir.join("source.png.bak"), b"stale-bak")?;

    let source = workspace.path().join("incoming.png");
    fs::write(&source, b"new")?;
    // file_name の basename を "source.png" にするため別 dir で作って rename
    let renamed = workspace.path().join("source.png");
    fs::rename(&source, &renamed)?;
    repo.import_sprite_image_with_backup("foo", "thumbnail", &renamed)?;

    assert_eq!(fs::read(sprites_dir.join("source.png"))?, b"new");
    // 最新の旧ファイル（"current"）が backup に入っている
    assert_eq!(fs::read(sprites_dir.join("source.png.bak"))?, b"current");
    Ok(())
}

#[test]
fn filesystem_delete_sprite_image_removes_copied_file() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    let source = workspace.path().join("source.png");
    fs::write(&source, b"png-bytes")?;
    let basename = repo.import_sprite_image("foo", "thumbnail", &source)?;

    repo.delete_sprite_image("foo", "thumbnail", &basename)?;

    let target = workspace
        .path()
        .join("data/characters/foo/sprite-groups/thumbnail/sprites/source.png");
    assert!(!target.exists(), "target file should be removed");

    // 不在に対する delete は no-op (二重削除の安全性)
    repo.delete_sprite_image("foo", "thumbnail", &basename)?;
    Ok(())
}

#[test]
fn filesystem_create_writes_animations() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk"), sample_animation("idle")],
    ))?;

    let anim_dir = workspace.path().join("data/characters/foo/animations");
    assert!(anim_dir.join("walk.yml").exists());
    assert!(anim_dir.join("idle.yml").exists());
    Ok(())
}

#[test]
fn filesystem_get_loads_animations_in_name_order() -> Result<()> {
    // Animation.number 廃止後、loader は yaml ファイル名 (Animation.name) 順で安定 sort する。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk"), sample_animation("idle")],
    ))?;

    let got = repo.get("foo")?.expect("character should exist");
    let names: Vec<_> = got.animations.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, vec!["idle", "walk"]); // 名前順
    Ok(())
}

#[test]
fn filesystem_update_preserves_files_under_animation_subdirs() -> Result<()> {
    // Character.update で animations/{anim}/ 配下に置かれた将来の派生ファイル
    // (たとえば layer 別のキャッシュ画像など) が巻き添えで消えないこと。リグレッション。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    let walk = sample_animation("walk");
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![walk.clone()],
    ))?;

    let anim_dir = workspace.path().join("data/characters/foo/animations");
    let extra = anim_dir.join("walk/cache/preview.png");
    fs::create_dir_all(extra.parent().expect("extra has a parent"))?;
    fs::write(&extra, b"fake-cache")?;

    repo.update(&sample_character_with_animations(
        "foo",
        999,
        vec![],
        vec![walk],
    ))?;

    assert!(extra.exists(), "animation subdir file should be preserved");
    assert!(anim_dir.join("walk.yml").exists());
    Ok(())
}

// ---------- SpriteGroup rename / delete ----------

#[test]
fn in_memory_rename_sprite_group_changes_name() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character(
        "foo",
        10,
        vec![
            sample_sprite_group("walk", 1),
            sample_sprite_group("idle", 2),
        ],
    ))?;

    repo.rename_sprite_group("foo", "walk", "run")?;

    let got = repo.get("foo")?.expect("character should exist");
    let names: Vec<_> = got.sprite_groups.iter().map(|g| g.name.as_str()).collect();
    assert!(names.contains(&"run"));
    assert!(!names.contains(&"walk"));
    assert_eq!(got.sprite_groups.len(), 2);
    Ok(())
}

#[test]
fn in_memory_rename_sprite_group_rejects_duplicate_and_missing() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character(
        "foo",
        10,
        vec![
            sample_sprite_group("walk", 1),
            sample_sprite_group("idle", 2),
        ],
    ))?;
    assert!(repo.rename_sprite_group("foo", "walk", "idle").is_err());
    assert!(repo.rename_sprite_group("foo", "nope", "x").is_err());
    Ok(())
}

#[test]
fn in_memory_delete_sprite_group_removes_from_character() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character(
        "foo",
        10,
        vec![
            sample_sprite_group("walk", 1),
            sample_sprite_group("idle", 2),
        ],
    ))?;

    repo.delete_sprite_group("foo", "walk")?;

    let got = repo.get("foo")?.expect("character should exist");
    let names: Vec<_> = got.sprite_groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, vec!["idle"]);

    // 不在 delete はエラー
    assert!(repo.delete_sprite_group("foo", "walk").is_err());
    Ok(())
}

#[test]
fn filesystem_rename_sprite_group_moves_yml_and_dir() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![sample_sprite_group("walk", 1)],
    ))?;

    // 画像も置いておく
    let old_image = workspace
        .path()
        .join("data/characters/foo/sprite-groups/walk/sprites/walk_001.png");
    fs::create_dir_all(old_image.parent().expect("image_path has a parent"))?;
    fs::write(&old_image, b"fake-png-bytes")?;

    repo.rename_sprite_group("foo", "walk", "run")?;

    let sg_dir = workspace.path().join("data/characters/foo/sprite-groups");
    assert!(!sg_dir.join("walk.yml").exists());
    assert!(sg_dir.join("run.yml").exists());
    assert!(!sg_dir.join("walk").exists());
    let new_image = sg_dir.join("run/sprites/walk_001.png");
    assert!(new_image.exists(), "image should be moved");
    assert_eq!(fs::read(&new_image)?, b"fake-png-bytes");

    // 新 yml の name フィールドが更新されている
    let got = repo.get("foo")?.expect("character should exist");
    let group = got
        .sprite_groups
        .iter()
        .find(|g| g.name == "run")
        .expect("run group should exist");
    assert_eq!(group.name, "run");
    Ok(())
}

#[test]
fn filesystem_rename_sprite_group_rejects_existing() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![
            sample_sprite_group("walk", 1),
            sample_sprite_group("idle", 2),
        ],
    ))?;

    let result = repo.rename_sprite_group("foo", "walk", "idle");
    assert!(result.is_err());

    // 旧 yml が残っていること
    let sg_dir = workspace.path().join("data/characters/foo/sprite-groups");
    assert!(sg_dir.join("walk.yml").exists());
    assert!(sg_dir.join("idle.yml").exists());
    Ok(())
}

#[test]
fn filesystem_delete_sprite_group_removes_yml_and_subdir() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![sample_sprite_group("walk", 1)],
    ))?;

    let image = workspace
        .path()
        .join("data/characters/foo/sprite-groups/walk/sprites/walk_001.png");
    fs::create_dir_all(image.parent().expect("image has a parent"))?;
    fs::write(&image, b"fake-png-bytes")?;

    repo.delete_sprite_group("foo", "walk")?;

    let sg_dir = workspace.path().join("data/characters/foo/sprite-groups");
    assert!(!sg_dir.join("walk.yml").exists());
    assert!(!sg_dir.join("walk").exists());
    assert!(!image.exists());
    Ok(())
}

#[test]
fn filesystem_delete_sprite_group_with_no_subdir() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character(
        "foo",
        10,
        vec![sample_sprite_group("walk", 1)],
    ))?;

    // ディレクトリ未作成（画像未取込）の状態でも削除が成功すること
    let sg_dir = workspace.path().join("data/characters/foo/sprite-groups");
    assert!(!sg_dir.join("walk").exists());

    repo.delete_sprite_group("foo", "walk")?;

    assert!(!sg_dir.join("walk.yml").exists());

    // 不在 delete はエラー
    assert!(repo.delete_sprite_group("foo", "walk").is_err());
    Ok(())
}

// ---------- Animation rename / delete ----------

#[test]
fn in_memory_rename_animation_changes_name() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk"), sample_animation("idle")],
    ))?;

    repo.rename_animation("foo", "walk", "run")?;

    let got = repo.get("foo")?.expect("character should exist");
    let names: Vec<_> = got.animations.iter().map(|a| a.name.as_str()).collect();
    assert!(names.contains(&"run"));
    assert!(!names.contains(&"walk"));
    Ok(())
}

#[test]
fn in_memory_rename_animation_rejects_duplicate_and_missing() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk"), sample_animation("idle")],
    ))?;
    assert!(repo.rename_animation("foo", "walk", "idle").is_err());
    assert!(repo.rename_animation("foo", "nope", "x").is_err());
    Ok(())
}

#[test]
fn in_memory_delete_animation_removes_from_character() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk"), sample_animation("idle")],
    ))?;

    repo.delete_animation("foo", "walk")?;

    let got = repo.get("foo")?.expect("character should exist");
    let names: Vec<_> = got.animations.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, vec!["idle"]);

    assert!(repo.delete_animation("foo", "walk").is_err());
    Ok(())
}

#[test]
fn filesystem_rename_animation_moves_yml_and_rewrites_name() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk")],
    ))?;

    repo.rename_animation("foo", "walk", "run")?;

    let anim_dir = workspace.path().join("data/characters/foo/animations");
    assert!(!anim_dir.join("walk.yml").exists());
    assert!(anim_dir.join("run.yml").exists());

    let got = repo.get("foo")?.expect("character should exist");
    let animation = got
        .animations
        .iter()
        .find(|a| a.name == "run")
        .expect("run animation should exist");
    assert_eq!(animation.name, "run");
    Ok(())
}

#[test]
fn filesystem_rename_animation_moves_subdir_when_present() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk")],
    ))?;

    // {anim_name}/ サブディレクトリ（将来用）にファイルを置いておく
    let anim_dir = workspace.path().join("data/characters/foo/animations");
    let extra = anim_dir.join("walk/cache/preview.png");
    fs::create_dir_all(extra.parent().expect("extra has a parent"))?;
    fs::write(&extra, b"fake-cache")?;

    repo.rename_animation("foo", "walk", "run")?;

    assert!(!extra.exists());
    let new_extra = anim_dir.join("run/cache/preview.png");
    assert!(new_extra.exists());
    assert_eq!(fs::read(&new_extra)?, b"fake-cache");
    Ok(())
}

#[test]
fn filesystem_rename_animation_rejects_existing() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk"), sample_animation("idle")],
    ))?;

    assert!(repo.rename_animation("foo", "walk", "idle").is_err());

    let anim_dir = workspace.path().join("data/characters/foo/animations");
    assert!(anim_dir.join("walk.yml").exists());
    assert!(anim_dir.join("idle.yml").exists());
    Ok(())
}

// ---------- Animation update_animation ----------

fn sample_frame(index: u32, duration: u32) -> Frame {
    Frame {
        index,
        duration,
        flip: None,
        pivot_point_offset: None,
        body_box_overrides: None,
        attack_box_overrides: None,
        sound: None,
        layers: Vec::new(),
    }
}

#[test]
fn in_memory_update_animation_replaces_in_place() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk"), sample_animation("idle")],
    ))?;

    let mut updated = sample_animation("walk");
    updated.frames.push(sample_frame(0, 80));
    repo.update_animation("foo", &updated)?;

    let got = repo.get("foo")?.expect("character should exist");
    let walk = got
        .animations
        .iter()
        .find(|a| a.name == "walk")
        .expect("walk animation should exist");
    assert_eq!(walk.frames.len(), 1);
    assert_eq!(walk.frames[0].duration, 80);
    // 別の Animation には影響しない
    let idle = got
        .animations
        .iter()
        .find(|a| a.name == "idle")
        .expect("idle animation should exist");
    assert_eq!(idle.frames.len(), 0);
    Ok(())
}

#[test]
fn in_memory_update_animation_rejects_missing() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk")],
    ))?;
    assert!(
        repo.update_animation("foo", &sample_animation("nope"))
            .is_err()
    );
    assert!(
        repo.update_animation("missing-character", &sample_animation("walk"))
            .is_err()
    );
    Ok(())
}

#[test]
fn filesystem_update_animation_writes_only_target_yml() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk"), sample_animation("idle")],
    ))?;

    let anim_dir = workspace.path().join("data/characters/foo/animations");
    let idle_yml_before = fs::read_to_string(anim_dir.join("idle.yml"))?;

    let mut updated = sample_animation("walk");
    updated.is_loop = false;
    updated.loop_start_index = 3;
    updated.frames.push(sample_frame(0, 100));
    repo.update_animation("foo", &updated)?;

    // walk.yml が新内容になっている
    let walk_yml_after = fs::read_to_string(anim_dir.join("walk.yml"))?;
    assert!(walk_yml_after.contains("is_loop: false"));
    assert!(walk_yml_after.contains("duration: 100"));
    // idle.yml は変わっていない
    let idle_yml_after = fs::read_to_string(anim_dir.join("idle.yml"))?;
    assert_eq!(idle_yml_before, idle_yml_after);
    Ok(())
}

#[test]
fn filesystem_update_animation_preserves_files_under_subdirs() -> Result<()> {
    // Repository.update_animation でサブディレクトリ {anim}/ 配下のファイルが
    // 巻き添えで消えないことを保証する。リグレッション。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk")],
    ))?;

    let anim_dir = workspace.path().join("data/characters/foo/animations");
    let extra = anim_dir.join("walk/cache/preview.png");
    fs::create_dir_all(extra.parent().expect("extra has a parent"))?;
    fs::write(&extra, b"fake-cache")?;

    let mut updated = sample_animation("walk");
    updated.frames.push(sample_frame(0, 50));
    repo.update_animation("foo", &updated)?;

    assert!(extra.exists(), "subdir file should be preserved");
    Ok(())
}

#[test]
fn filesystem_update_animation_rejects_missing_yml() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk")],
    ))?;
    assert!(
        repo.update_animation("foo", &sample_animation("missing"))
            .is_err()
    );
    Ok(())
}

#[test]
fn filesystem_delete_animation_removes_yml_and_subdir() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("walk")],
    ))?;

    let anim_dir = workspace.path().join("data/characters/foo/animations");
    let extra = anim_dir.join("walk/cache/preview.png");
    fs::create_dir_all(extra.parent().expect("extra has a parent"))?;
    fs::write(&extra, b"fake-cache")?;

    repo.delete_animation("foo", "walk")?;

    assert!(!anim_dir.join("walk.yml").exists());
    assert!(!anim_dir.join("walk").exists());
    assert!(!extra.exists());

    // 不在 delete はエラー
    assert!(repo.delete_animation("foo", "walk").is_err());
    Ok(())
}

// ---------- SoundGroup roundtrip / Frame.sound ----------

fn sample_sound_group(name: &str, number: u32, sounds: Vec<Sound>) -> SoundGroup {
    SoundGroup {
        name: name.to_string(),
        number,
        sounds,
    }
}

fn sample_character_with_sound_groups(
    name: &str,
    hp: u32,
    sound_groups: Vec<SoundGroup>,
) -> Character {
    Character {
        name: name.to_string(),
        thumbnail_path: format!("sprite-groups/thumbnail/sprites/{name}.png"),
        hp,
        depth: DEFAULT_CHARACTER_DEPTH,
        physics: CharacterPhysics::default(),
        sprite_groups: Vec::new(),
        animations: Vec::new(),
        sound_groups,
    }
}

#[test]
fn filesystem_persists_sound_groups_with_weight() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    let pain = sample_sound_group(
        "pain",
        5000,
        vec![
            Sound {
                index: 0,
                path: "pain_001.wav".to_string(),
                volume: 1.0,
                weight: 0.0, // 省略相当 → yaml 出力でも省略される (engine で 1.0 にフォールバック)
            },
            Sound {
                index: 1,
                path: "pain_002.wav".to_string(),
                volume: 0.9,
                weight: 2.0,
            },
        ],
    );
    repo.create(&sample_character_with_sound_groups("foo", 100, vec![pain]))?;

    // ファイルが書かれている
    let pain_yml = workspace
        .path()
        .join("data/characters/foo/sound-groups/pain.yml");
    assert!(pain_yml.exists());
    let yml = fs::read_to_string(&pain_yml)?;
    // weight=0 の Sound は YAML から weight 行が省略されるが、weight=2.0 は残る。
    assert!(yml.contains("weight: 2"), "weight line missing: {yml}");

    // 読み戻すと sound_groups が正しく載っている
    let loaded = repo.get("foo")?.expect("character exists");
    assert_eq!(loaded.sound_groups.len(), 1);
    let g = &loaded.sound_groups[0];
    assert_eq!(g.name, "pain");
    assert_eq!(g.number, 5000);
    assert_eq!(g.sounds.len(), 2);
    assert_eq!(g.sounds[0].path, "pain_001.wav");
    assert!((g.sounds[1].weight - 2.0).abs() < 1e-6);

    Ok(())
}

#[test]
fn filesystem_persists_frame_sound_field() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    let mut anim = sample_animation("hit");
    let mut f = sample_frame(0, 200);
    f.sound = Some(FrameSound {
        number: 5000,
        delay_ms: 120,
    });
    anim.frames.push(f);
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![anim],
    ))?;

    let loaded = repo
        .get("foo")?
        .expect("character exists")
        .animations
        .into_iter()
        .find(|a| a.name == "hit")
        .expect("hit animation");
    assert_eq!(
        loaded.frames[0].sound,
        Some(FrameSound {
            number: 5000,
            delay_ms: 120,
        })
    );
    Ok(())
}

#[test]
fn filesystem_renames_sound_group_yml_and_subdir() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_sound_groups(
        "foo",
        10,
        vec![sample_sound_group(
            "pain",
            5000,
            vec![Sound {
                index: 0,
                path: "pain_001.wav".into(),
                volume: 1.0,
                weight: 0.0,
            }],
        )],
    ))?;

    // sound-groups/{name}/sounds/ ディレクトリも mock でつくる (rename で {new}/ に移動されるはず)
    let sounds_dir = workspace
        .path()
        .join("data/characters/foo/sound-groups/pain/sounds");
    fs::create_dir_all(&sounds_dir)?;
    fs::write(sounds_dir.join("pain_001.wav"), b"fake-wav")?;

    repo.rename_sound_group("foo", "pain", "hurt")?;

    let sg_dir = workspace.path().join("data/characters/foo/sound-groups");
    assert!(!sg_dir.join("pain.yml").exists());
    assert!(sg_dir.join("hurt.yml").exists());
    assert!(!sg_dir.join("pain").exists());
    assert!(sg_dir.join("hurt/sounds/pain_001.wav").exists());

    let yml = fs::read_to_string(sg_dir.join("hurt.yml"))?;
    assert!(yml.contains("name: hurt"), "name not rewritten: {yml}");
    Ok(())
}

#[test]
fn filesystem_deletes_sound_group_yml_and_subdir() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_sound_groups(
        "foo",
        10,
        vec![sample_sound_group("pain", 5000, vec![])],
    ))?;
    let sg_dir = workspace.path().join("data/characters/foo/sound-groups");
    fs::create_dir_all(sg_dir.join("pain/sounds"))?;
    fs::write(sg_dir.join("pain/sounds/pain_001.wav"), b"fake-wav")?;

    repo.delete_sound_group("foo", "pain")?;
    assert!(!sg_dir.join("pain.yml").exists());
    assert!(!sg_dir.join("pain").exists());

    // 不在 delete はエラー
    assert!(repo.delete_sound_group("foo", "pain").is_err());
    Ok(())
}

#[test]
fn filesystem_update_sound_group_writes_only_target_yml() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_sound_groups(
        "foo",
        10,
        vec![
            sample_sound_group(
                "pain",
                5000,
                vec![Sound {
                    index: 0,
                    path: "pain_001.wav".into(),
                    volume: 1.0,
                    weight: 0.0,
                }],
            ),
            sample_sound_group("death", 11, vec![]),
        ],
    ))?;

    // pain だけ Sound を追加して update_sound_group
    let mut updated = sample_sound_group(
        "pain",
        5000,
        vec![
            Sound {
                index: 0,
                path: "pain_001.wav".into(),
                volume: 1.0,
                weight: 0.0,
            },
            Sound {
                index: 1,
                path: "pain_002.wav".into(),
                volume: 0.8,
                weight: 3.0,
            },
        ],
    );
    updated.number = 5000;
    repo.update_sound_group("foo", &updated)?;

    let loaded = repo
        .get("foo")?
        .expect("character exists")
        .sound_groups
        .into_iter()
        .find(|g| g.name == "pain")
        .expect("pain group");
    assert_eq!(loaded.sounds.len(), 2);
    assert!((loaded.sounds[1].weight - 3.0).abs() < 1e-6);

    // 不在 update はエラー
    let mut bogus = sample_sound_group("bogus", 9999, vec![]);
    bogus.number = 9999;
    assert!(repo.update_sound_group("foo", &bogus).is_err());
    Ok(())
}

#[test]
fn filesystem_import_sound_file_copies_to_sounds_dir() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_sound_groups(
        "foo",
        10,
        vec![sample_sound_group("pain", 5000, vec![])],
    ))?;

    let source = workspace.path().join("source.wav");
    fs::write(&source, b"riff-wav-bytes")?;

    let basename = repo.import_sound_file("foo", "pain", &source)?;
    assert_eq!(basename, "source.wav");

    let copied = workspace
        .path()
        .join("data/characters/foo/sound-groups/pain/sounds/source.wav");
    assert!(copied.exists());
    assert_eq!(fs::read(&copied)?, b"riff-wav-bytes");
    Ok(())
}

#[test]
fn filesystem_update_sound_group_prunes_orphan_wav_files() -> Result<()> {
    // Save 時に yml と sounds/ 配下の wav を同期する。yml に書かれていない wav は orphan として削除。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_sound_groups(
        "foo",
        10,
        vec![sample_sound_group(
            "pain",
            5000,
            vec![Sound {
                index: 0,
                path: "a.wav".into(),
                volume: 1.0,
                weight: 0.0,
            }],
        )],
    ))?;

    // a.wav / b.wav / c.wav を sounds/ に置く
    let sounds_dir = workspace
        .path()
        .join("data/characters/foo/sound-groups/pain/sounds");
    fs::create_dir_all(&sounds_dir)?;
    fs::write(sounds_dir.join("a.wav"), b"a")?;
    fs::write(sounds_dir.join("b.wav"), b"b")?;
    fs::write(sounds_dir.join("c.wav"), b"c")?;

    // yml に a と c だけを残して update_sound_group → b.wav は orphan として削除されるはず
    let mut updated = sample_sound_group(
        "pain",
        5000,
        vec![
            Sound {
                index: 0,
                path: "a.wav".into(),
                volume: 1.0,
                weight: 0.0,
            },
            Sound {
                index: 1,
                path: "c.wav".into(),
                volume: 1.0,
                weight: 0.0,
            },
        ],
    );
    updated.number = 5000;
    repo.update_sound_group("foo", &updated)?;

    assert!(sounds_dir.join("a.wav").exists());
    assert!(
        !sounds_dir.join("b.wav").exists(),
        "b.wav は orphan なので削除されるはず"
    );
    assert!(sounds_dir.join("c.wav").exists());
    Ok(())
}

#[test]
fn filesystem_update_sound_group_handles_missing_sounds_dir() -> Result<()> {
    // sounds/ 自体が無いケースでも error にならず、yml だけ書き込まれる。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_sound_groups(
        "foo",
        10,
        vec![sample_sound_group("pain", 5000, vec![])],
    ))?;

    let mut updated = sample_sound_group("pain", 5000, vec![]);
    updated.number = 5000;
    // sounds/ ディレクトリは作っていない
    repo.update_sound_group("foo", &updated)?;
    Ok(())
}

#[test]
fn filesystem_import_sound_file_rejects_existing_target() -> Result<()> {
    // 同名既存 wav を上書きしない（pending_imports rollback で committed wav を消す事故防止）。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_sound_groups(
        "foo",
        10,
        vec![sample_sound_group("pain", 5000, vec![])],
    ))?;

    let sounds_dir = workspace
        .path()
        .join("data/characters/foo/sound-groups/pain/sounds");
    fs::create_dir_all(&sounds_dir)?;
    fs::write(sounds_dir.join("dup.wav"), b"committed")?;

    let source = workspace.path().join("dup.wav");
    fs::write(&source, b"new-content")?;

    // 同名 import は error
    assert!(repo.import_sound_file("foo", "pain", &source).is_err());
    // 既存ファイルは触られない
    assert_eq!(fs::read(sounds_dir.join("dup.wav"))?, b"committed");
    Ok(())
}

#[test]
fn filesystem_delete_sound_file_removes_only_target() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_sound_groups(
        "foo",
        10,
        vec![sample_sound_group("pain", 5000, vec![])],
    ))?;
    let dir = workspace
        .path()
        .join("data/characters/foo/sound-groups/pain/sounds");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("a.wav"), b"a")?;
    fs::write(dir.join("b.wav"), b"b")?;

    repo.delete_sound_file("foo", "pain", "a.wav")?;
    assert!(!dir.join("a.wav").exists());
    assert!(dir.join("b.wav").exists());

    // 不在 delete は no-op (sprite と同じ規約)
    repo.delete_sound_file("foo", "pain", "ghost.wav")?;
    Ok(())
}

#[test]
fn in_memory_sound_group_crud_roundtrip() -> Result<()> {
    let repo = InMemoryCharacterRepository::new();
    repo.create(&sample_character_with_sound_groups(
        "foo",
        10,
        vec![
            sample_sound_group("pain", 5000, vec![]),
            sample_sound_group("death", 11, vec![]),
        ],
    ))?;

    repo.rename_sound_group("foo", "pain", "hurt")?;
    let names: Vec<String> = repo
        .get("foo")?
        .expect("foo exists")
        .sound_groups
        .into_iter()
        .map(|g| g.name)
        .collect();
    assert!(names.contains(&"hurt".to_string()));
    assert!(!names.contains(&"pain".to_string()));

    // 重複 rename はエラー
    assert!(repo.rename_sound_group("foo", "death", "hurt").is_err());

    repo.delete_sound_group("foo", "death")?;
    assert_eq!(repo.get("foo")?.expect("foo exists").sound_groups.len(), 1);

    // 不在 delete はエラー
    assert!(repo.delete_sound_group("foo", "death").is_err());
    Ok(())
}

#[test]
fn filesystem_omits_frame_sound_when_none() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());

    let mut anim = sample_animation("idle");
    anim.frames.push(sample_frame(0, 50)); // sound: None
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![anim],
    ))?;

    let yml = fs::read_to_string(
        workspace
            .path()
            .join("data/characters/foo/animations/idle.yml"),
    )?;
    assert!(
        !yml.contains("sound:"),
        "sound: None should be omitted from YAML, got:\n{yml}"
    );
    Ok(())
}

#[test]
fn filesystem_update_animation_persists_role_and_variant() -> Result<()> {
    // Animation.role + Animation.variant が YAML に書き込まれ、再 get で復元されること。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("attack1")],
    ))?;

    let updated = sample_animation_with_role("attack1", Role::Attack, 2);
    repo.update_animation("foo", &updated)?;

    let yml = fs::read_to_string(
        workspace
            .path()
            .join("data/characters/foo/animations/attack1.yml"),
    )?;
    assert!(
        yml.contains("role: attack"),
        "yml should contain role: attack, got:\n{yml}"
    );
    assert!(
        yml.contains("variant: 2"),
        "yml should contain variant: 2, got:\n{yml}"
    );

    let got = repo.get("foo")?.expect("character should exist");
    let anim = got
        .animations
        .iter()
        .find(|a| a.name == "attack1")
        .expect("attack1 missing");
    assert_eq!(anim.role, Role::Attack);
    assert_eq!(anim.variant, 2);
    Ok(())
}

#[test]
fn filesystem_loads_animation_without_role_field_as_custom() -> Result<()> {
    // 既存 yaml (role 行なし) を読んでも Role::Custom にフォールバックして壊れないこと。
    let workspace = tempfile::tempdir()?;
    let repo = FilesystemCharacterRepository::new(workspace.path());
    repo.create(&sample_character_with_animations(
        "foo",
        10,
        vec![],
        vec![sample_animation("ai_taunt")],
    ))?;

    // disk から role: 行を取り除いて legacy yaml を再現
    let yml_path = workspace
        .path()
        .join("data/characters/foo/animations/ai_taunt.yml");
    let yml = fs::read_to_string(&yml_path)?;
    let stripped: String = yml
        .lines()
        .filter(|l| !l.starts_with("role:") && !l.starts_with("variant:"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&yml_path, stripped)?;

    let got = repo.get("foo")?.expect("character should exist");
    let anim = got
        .animations
        .iter()
        .find(|a| a.name == "ai_taunt")
        .expect("ai_taunt missing");
    assert_eq!(anim.role, Role::Custom);
    assert_eq!(anim.variant, 0);
    Ok(())
}
