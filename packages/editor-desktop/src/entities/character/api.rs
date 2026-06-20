//! Character の Repository (trait + 2 実装) と関連する型 (`ImportOutcome`)。
//!
//! ファイル分割:
//! - `repository`: trait 定義と `ImportOutcome`
//! - `in_memory`: テスト用 fake (`InMemoryCharacterRepository`)
//! - `filesystem`: 本番用 (`FilesystemCharacterRepository`)
//! - `tests`: 両 impl が trait 契約を満たすかの contract test と回帰テスト

mod filesystem;
mod in_memory;
mod repository;

#[cfg(test)]
mod tests;

pub use filesystem::FilesystemCharacterRepository;
pub use in_memory::InMemoryCharacterRepository;
pub use repository::{CharacterRepository, ImportOutcome};
