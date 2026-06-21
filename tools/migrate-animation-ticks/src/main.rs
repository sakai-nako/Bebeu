//! Animation YAML の `duration: <ms>` を `ticks: <60Hz tick 数>` に書き換える 1 回切りの
//! migration tool。Phase 2 で schema が ms → 60Hz tick に変わったための対応。
//!
//! 変換式: `ticks = round(ms * 60 / 1000)`、最低 1 tick。
//!
//! 対象 ディレクトリ:
//! - `runtime/data/characters/*/animations/*.yml`
//! - `sample-projects/*/data/characters/*/animations/*.yml`
//!
//! Idempotent: `duration:` を含まないファイルは skip するので、何度走らせても安全。
//!
//! - 行頭 (indent + `duration:` + 数値) だけマッチさせるので、コメント `# duration:`
//!   や文字列 (`"duration: ..."`) は触らない (実際 animation YAML には存在しない想定)。
//! - 余計な依存 (regex 等) は入れず、行ベースの簡単な parser で済ませる。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const MIGRATION_TARGETS: &[&str] = &[
    "runtime/data/characters",
    "sample-projects/minimal/data/characters",
];

fn main() -> Result<()> {
    let mut migrated = 0u32;
    let mut skipped = 0u32;
    let mut details: Vec<String> = Vec::new();

    for root in MIGRATION_TARGETS {
        let root = Path::new(root);
        if !root.exists() {
            eprintln!("[skip] root not found: {}", root.display());
            continue;
        }
        for yml in find_animation_yamls(root)? {
            let outcome =
                migrate_file(&yml).with_context(|| format!("migrate {}", yml.display()))?;
            match outcome {
                MigrationOutcome::Migrated(n) => {
                    migrated += 1;
                    details.push(format!("[ok] {} ({} frame replaced)", yml.display(), n));
                }
                MigrationOutcome::NoChange => skipped += 1,
            }
        }
    }

    for d in &details {
        println!("{d}");
    }
    println!("---");
    println!("migrated: {migrated} file(s)");
    println!("skipped:  {skipped} file(s) (already ticks or no duration field)");
    Ok(())
}

/// `<root>/<character>/animations/*.yml` を全部集める。
fn find_animation_yamls(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for char_dir in fs::read_dir(root).with_context(|| format!("read_dir {}", root.display()))? {
        let char_dir = char_dir?.path();
        if !char_dir.is_dir() {
            continue;
        }
        let anim_dir = char_dir.join("animations");
        if !anim_dir.is_dir() {
            continue;
        }
        for entry in
            fs::read_dir(&anim_dir).with_context(|| format!("read_dir {}", anim_dir.display()))?
        {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yml") {
                out.push(path);
            }
        }
    }
    Ok(out)
}

enum MigrationOutcome {
    Migrated(u32),
    NoChange,
}

fn migrate_file(path: &Path) -> Result<MigrationOutcome> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Some((new_content, n)) = migrate_text(&content) else {
        return Ok(MigrationOutcome::NoChange);
    };
    fs::write(path, new_content).with_context(|| format!("write {}", path.display()))?;
    Ok(MigrationOutcome::Migrated(n))
}

/// 行ベースで `<indent>duration: <number>` を `<indent>ticks: <converted>` に置換。
/// 1 回でも置換が起きたら `Some((新 content, 置換数))` を返す。
fn migrate_text(content: &str) -> Option<(String, u32)> {
    let mut count = 0u32;
    let pieces: Vec<String> = content
        .split('\n')
        .map(|line| {
            try_rewrite_line(line).map_or_else(
                || line.to_string(),
                |rewritten| {
                    count += 1;
                    rewritten
                },
            )
        })
        .collect();
    if count == 0 {
        return None;
    }
    Some((pieces.join("\n"), count))
}

/// 1 行を「indent + `duration:` + 数値」とみなせるか試して、できれば書き換えた行を返す。
fn try_rewrite_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("duration:")?;
    let val_str = rest.trim();
    let ms: u64 = val_str.parse().ok()?;
    let ticks = ms_to_ticks(ms);
    let indent = &line[..line.len() - trimmed.len()];
    Some(format!("{indent}ticks: {ticks}"))
}

/// ms → 60Hz tick 数。round-to-nearest、最低 1 tick。
fn ms_to_ticks(ms: u64) -> u64 {
    // ticks = round(ms * 60 / 1000) = (ms * 60 + 500) / 1000、最低 1
    ((ms * 60 + 500) / 1000).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ms_to_ticks_classic_values() {
        // 120ms / 16.67ms = 7.2 → round 7
        assert_eq!(ms_to_ticks(120), 7);
        // 150ms / 16.67ms = 9.0 → round 9
        assert_eq!(ms_to_ticks(150), 9);
        // 100ms / 16.67ms = 6.0 → round 6
        assert_eq!(ms_to_ticks(100), 6);
        // 50ms / 16.67ms = 3.0 → round 3
        assert_eq!(ms_to_ticks(50), 3);
        // 1ms → round 0 だが clamp to 1
        assert_eq!(ms_to_ticks(1), 1);
        // 0ms → clamp to 1
        assert_eq!(ms_to_ticks(0), 1);
        // 16ms → round 1
        assert_eq!(ms_to_ticks(16), 1);
        // 17ms → round 1 (1.02)
        assert_eq!(ms_to_ticks(17), 1);
        // 25ms → round 2 (1.5)
        assert_eq!(ms_to_ticks(25), 2);
    }

    #[test]
    fn try_rewrite_line_simple_two_space_indent() {
        let line = "  duration: 120";
        assert_eq!(try_rewrite_line(line).as_deref(), Some("  ticks: 7"));
    }

    #[test]
    fn try_rewrite_line_preserves_leading_dash_indent() {
        // YAML の list item: "- index: 0\n  duration: 120" の "  duration: 120" 部分
        let line = "    duration: 50";
        assert_eq!(try_rewrite_line(line).as_deref(), Some("    ticks: 3"));
    }

    #[test]
    fn try_rewrite_line_ignores_unrelated_fields() {
        assert!(try_rewrite_line("  delay_ms: 100").is_none());
        assert!(try_rewrite_line("name: walk").is_none());
        assert!(try_rewrite_line("- index: 0").is_none());
    }

    #[test]
    fn try_rewrite_line_ignores_comments() {
        // 先頭が `#` で始まる行は trim_start 後も `# duration:` のまま (prefix は "duration:" ではない)。
        assert!(try_rewrite_line("# duration: 120").is_none());
        assert!(try_rewrite_line("  # duration: 120").is_none());
    }

    #[test]
    fn try_rewrite_line_ignores_non_integer_value() {
        assert!(try_rewrite_line("  duration: not-a-number").is_none());
    }

    #[test]
    fn migrate_text_returns_none_when_no_duration() {
        let input = "name: walk\nframes:\n- index: 0\n  ticks: 7\n";
        assert!(migrate_text(input).is_none());
    }

    #[test]
    fn migrate_text_rewrites_multiple_frames() {
        let input =
            "name: walk\nframes:\n- index: 0\n  duration: 120\n- index: 1\n  duration: 50\n";
        let (out, n) = migrate_text(input).expect("should migrate");
        assert_eq!(n, 2);
        assert!(out.contains("ticks: 7"));
        assert!(out.contains("ticks: 3"));
        assert!(!out.contains("duration:"));
    }

    #[test]
    fn migrate_text_preserves_trailing_newline() {
        let input = "  duration: 120\n";
        let (out, _) = migrate_text(input).expect("should migrate");
        assert_eq!(out, "  ticks: 7\n");
    }

    #[test]
    fn migrate_text_preserves_no_trailing_newline() {
        let input = "  duration: 120";
        let (out, _) = migrate_text(input).expect("should migrate");
        assert_eq!(out, "  ticks: 7");
    }
}
