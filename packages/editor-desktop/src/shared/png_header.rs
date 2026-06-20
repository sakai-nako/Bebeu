//! PNG ファイルから幅/高さだけを取り出す軽量パーサ。
//!
//! `image` / `png` crate 等の依存追加を避けるための自前実装。PNG signature と IHDR chunk
//! の最初の 8 バイト (width, height) しか読まないので速い。
//!
//! PNG file format: <https://www.w3.org/TR/png/>
//! - bytes 0..8: signature `89 50 4E 47 0D 0A 1A 0A`
//! - bytes 8..12: IHDR chunk length (= 13)
//! - bytes 12..16: IHDR chunk type (`"IHDR"`)
//! - bytes 16..20: width (u32 BE)
//! - bytes 20..24: height (u32 BE)

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Result, bail};

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// PNG ファイルから (width, height) を読む。signature が合わない / 短すぎる場合はエラー。
pub fn read_png_dimensions(path: &Path) -> Result<[u32; 2]> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 24];
    file.read_exact(&mut header)?;
    parse_png_dimensions(&header)
}

/// 24 バイトの header から dimensions を取り出す。テスト用にも使える。
pub fn parse_png_dimensions(header: &[u8]) -> Result<[u32; 2]> {
    if header.len() < 24 {
        bail!("PNG header too short: {} bytes", header.len());
    }
    if header[0..8] != PNG_SIGNATURE {
        bail!("not a PNG file (signature mismatch)");
    }
    if &header[12..16] != b"IHDR" {
        bail!("expected IHDR chunk, found {:?}", &header[12..16]);
    }
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);
    Ok([width, height])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1x1 透明 PNG の最小バイト列を組み立てる (signature + IHDR header)。
    fn build_png_header(w: u32, h: u32) -> Vec<u8> {
        let mut buf = PNG_SIGNATURE.to_vec();
        buf.extend_from_slice(&13u32.to_be_bytes()); // IHDR length
        buf.extend_from_slice(b"IHDR");
        buf.extend_from_slice(&w.to_be_bytes());
        buf.extend_from_slice(&h.to_be_bytes());
        buf
    }

    #[test]
    fn parses_valid_png_header() {
        let header = build_png_header(42, 97);
        let dims = parse_png_dimensions(&header).expect("valid PNG header");
        assert_eq!(dims, [42, 97]);
    }

    #[test]
    fn parses_large_png_header() {
        let header = build_png_header(1920, 1080);
        let dims = parse_png_dimensions(&header).expect("valid PNG header");
        assert_eq!(dims, [1920, 1080]);
    }

    #[test]
    fn rejects_short_header() {
        let header = vec![0u8; 10];
        let err = parse_png_dimensions(&header).expect_err("parse should fail");
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn rejects_non_png_signature() {
        let mut header = vec![0u8; 24];
        header[0..4].copy_from_slice(b"\x00\x00\x00\x01"); // wrong signature
        let err = parse_png_dimensions(&header).expect_err("parse should fail");
        assert!(err.to_string().contains("signature"));
    }

    #[test]
    fn rejects_non_ihdr_chunk() {
        let mut header = build_png_header(1, 1);
        header[12..16].copy_from_slice(b"sBIT"); // wrong chunk type
        let err = parse_png_dimensions(&header).expect_err("parse should fail");
        assert!(err.to_string().contains("IHDR"));
    }
}
