// rust_i18n の catalog 展開 (`packages/editor-desktop/i18n/{ja,en}.yml`)。
// このマクロはクレートルートで呼ぶ必要がある。詳細は `shared/i18n.rs` と ADR-0042。
rust_i18n::i18n!("i18n", fallback = "ja");

mod app;
pub use app::entrypoint;

pub(crate) mod entities;
pub(crate) mod features;
pub(crate) mod pages;
pub(crate) mod shared;
pub(crate) mod widgets;
