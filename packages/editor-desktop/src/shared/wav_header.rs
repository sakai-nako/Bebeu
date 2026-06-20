//! WAV ファイルのヘッダ情報（サンプリングレート / チャンネル数 / 再生時間）を読み取る軽量パーサ。
//!
//! `hound` 等の依存追加を避けるため自前実装。RIFF/WAVE の "fmt " と "data" チャンクだけを
//! 探して必要最小限のフィールドを取り出す。LIST / JUNK 等の未知チャンクはサイズを読んで
//! スキップする。

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Result, anyhow, bail};

/// WAV のメタデータ。`duration_secs` は data チャンクサイズ ÷ byte_rate で算出する。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WavInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub duration_secs: f32,
}

impl WavInfo {
    /// "44100 Hz · 16 bit · Stereo · 1.23s" のような 1 行ラベルを生成する。
    /// 順序は Hz → bit → channel → duration（ユーザ要望）。
    #[must_use]
    pub fn label(&self) -> String {
        let ch = match self.channels {
            1 => "Mono".to_string(),
            2 => "Stereo".to_string(),
            n => format!("{n}ch"),
        };
        format!(
            "{} Hz · {} bit · {ch} · {:.2}s",
            self.sample_rate, self.bits_per_sample, self.duration_secs
        )
    }
}

/// 指定 path から WAV ヘッダを読む。
pub fn read_wav_info(path: &Path) -> Result<WavInfo> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    parse_wav_info(&mut reader)
}

/// `Read + Seek` から WAV ヘッダを解析する。テストでは `Cursor<Vec<u8>>` を渡せる。
pub fn parse_wav_info<R: Read + Seek>(reader: &mut R) -> Result<WavInfo> {
    let mut riff = [0u8; 12];
    reader.read_exact(&mut riff)?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        bail!("not a RIFF/WAVE file");
    }

    let mut sample_rate: u32 = 0;
    let mut channels: u16 = 0;
    let mut bits_per_sample: u16 = 0;
    let mut byte_rate: u32 = 0;
    let mut data_size: Option<u32> = None;
    let mut fmt_seen = false;

    loop {
        let mut chunk_header = [0u8; 8];
        match reader.read_exact(&mut chunk_header) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]);
        let chunk_id = &chunk_header[0..4];

        match chunk_id {
            b"fmt " => {
                if chunk_size < 16 {
                    bail!("fmt chunk too small ({chunk_size} bytes)");
                }
                let mut fmt = vec![0u8; chunk_size as usize];
                reader.read_exact(&mut fmt)?;
                channels = u16::from_le_bytes([fmt[2], fmt[3]]);
                sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
                byte_rate = u32::from_le_bytes([fmt[8], fmt[9], fmt[10], fmt[11]]);
                bits_per_sample = u16::from_le_bytes([fmt[14], fmt[15]]);
                fmt_seen = true;
                // RIFF チャンクは 2 バイト境界。奇数サイズなら 1 バイト padding を読み飛ばす。
                if chunk_size % 2 == 1 {
                    let mut pad = [0u8; 1];
                    let _ = reader.read_exact(&mut pad);
                }
            }
            b"data" => {
                data_size = Some(chunk_size);
                break;
            }
            _ => {
                let aligned = i64::from(chunk_size) + i64::from(chunk_size % 2);
                reader.seek(SeekFrom::Current(aligned))?;
            }
        }
    }

    if !fmt_seen {
        bail!("fmt chunk not found");
    }
    if byte_rate == 0 {
        return Err(anyhow!("byte_rate is zero in fmt chunk"));
    }

    // u32 → f32 のキャストは精度落ちが起き得るが、UI 表示用なので許容（最大誤差 ~ULP）。
    #[allow(clippy::cast_precision_loss)]
    let duration_secs = data_size.map_or(0.0, |s| s as f32 / byte_rate as f32);

    Ok(WavInfo {
        sample_rate,
        channels,
        bits_per_sample,
        duration_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// テスト用の最小 WAV を組み立てる（PCM, fmt + data のみ）。
    fn build_wav(sample_rate: u32, channels: u16, bits: u16, num_samples: u32) -> Vec<u8> {
        let block_align = channels * bits / 8;
        let byte_rate = sample_rate * u32::from(block_align);
        let data_size = num_samples * u32::from(block_align);
        let fmt_size: u32 = 16;
        let total_size = 4 + (8 + fmt_size) + (8 + data_size);

        let mut out = Vec::with_capacity(total_size as usize + 8);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&total_size.to_le_bytes());
        out.extend_from_slice(b"WAVE");

        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&fmt_size.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());

        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_size.to_le_bytes());
        out.resize(out.len() + data_size as usize, 0);
        out
    }

    #[test]
    fn parses_minimal_pcm_wav() {
        // 44100 Hz Stereo 16bit、44100 samples = 1.0 秒
        let bytes = build_wav(44100, 2, 16, 44100);
        let info = parse_wav_info(&mut Cursor::new(bytes)).expect("parse");
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.bits_per_sample, 16);
        assert!((info.duration_secs - 1.0).abs() < 0.001);
    }

    #[test]
    fn parses_mono_8bit() {
        let bytes = build_wav(8000, 1, 8, 4000);
        let info = parse_wav_info(&mut Cursor::new(bytes)).expect("parse");
        assert_eq!(info.channels, 1);
        assert_eq!(info.bits_per_sample, 8);
        assert!((info.duration_secs - 0.5).abs() < 0.001);
    }

    #[test]
    fn skips_unknown_chunk_before_data() {
        // fmt の後に "JUNK" チャンクを挟んでも data に到達できる
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&0u32.to_le_bytes()); // size 後で未使用
        bytes.extend_from_slice(b"WAVE");

        // fmt
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes()); // channels
        bytes.extend_from_slice(&48000u32.to_le_bytes()); // sample_rate
        bytes.extend_from_slice(&192_000u32.to_le_bytes()); // byte_rate (48000*2*2)
        bytes.extend_from_slice(&4u16.to_le_bytes()); // block_align
        bytes.extend_from_slice(&16u16.to_le_bytes()); // bits

        // JUNK 5 バイト + 1 バイト padding
        bytes.extend_from_slice(b"JUNK");
        bytes.extend_from_slice(&5u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 5]);
        bytes.push(0); // padding

        // data 192_000 バイト = 0.5 秒
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&96_000u32.to_le_bytes());
        bytes.resize(bytes.len() + 96_000, 0);

        let info = parse_wav_info(&mut Cursor::new(bytes)).expect("parse");
        assert_eq!(info.sample_rate, 48000);
        assert_eq!(info.channels, 2);
        assert!((info.duration_secs - 0.5).abs() < 0.001);
    }

    #[test]
    fn rejects_non_riff() {
        let bytes = b"NOPE........".to_vec();
        assert!(parse_wav_info(&mut Cursor::new(bytes)).is_err());
    }

    #[test]
    fn label_formats_known_values() {
        let info = WavInfo {
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: 16,
            duration_secs: 1.234,
        };
        assert_eq!(info.label(), "44100 Hz · 16 bit · Stereo · 1.23s");
    }
}
