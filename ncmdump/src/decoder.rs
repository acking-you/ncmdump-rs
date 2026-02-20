use std::io::{Read, Seek, SeekFrom, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::cipher::{aes128_ecb_decrypt, rc4_ksa, rc4_stream_byte};
use crate::error::{NcmError, Result};
use crate::metadata::NcmMetadata;

/// NCM file magic: "CTENFDAM"
const NCM_MAGIC: [u8; 8] = [0x43, 0x54, 0x45, 0x4E, 0x46, 0x44, 0x41, 0x4D];

/// AES key for decrypting the RC4 key data.
const CORE_KEY: [u8; 16] = [
    0x68, 0x7A, 0x48, 0x52, 0x41, 0x6D, 0x73, 0x6F, 0x35, 0x6B, 0x49, 0x6E, 0x62, 0x61, 0x78, 0x57,
];

/// AES key for decrypting the metadata.
const MODIFY_KEY: [u8; 16] = [
    0x23, 0x31, 0x34, 0x6C, 0x6A, 0x6B, 0x5F, 0x21, 0x5C, 0x5D, 0x26, 0x30, 0x55, 0x3C, 0x27, 0x28,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    Flac,
}

impl AudioFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
        }
    }
}

/// Parsed NCM file, ready for audio extraction.
pub struct NcmFile {
    pub metadata: Option<NcmMetadata>,
    pub cover_image: Option<Vec<u8>>,
    pub format: AudioFormat,
    pub key_box: [u8; 256],
    pub audio_offset: u64,
}

impl NcmFile {
    /// Parse an NCM file from a reader. After this, call `dump_audio` to extract.
    pub fn parse<R: Read + Seek>(r: &mut R) -> Result<Self> {
        // 1. Verify magic
        let mut magic = [0u8; 8];
        r.read_exact(&mut magic)?;
        if magic != NCM_MAGIC {
            return Err(NcmError::InvalidMagic);
        }

        // 2. Skip 2-byte gap
        r.seek(SeekFrom::Current(2))?;

        // 3. Read & decrypt RC4 key
        let key_len = read_u32_le(r)? as usize;
        let mut key_data = vec![0u8; key_len];
        r.read_exact(&mut key_data)?;
        for b in &mut key_data {
            *b ^= 0x64;
        }
        let key_decrypted = aes128_ecb_decrypt(&CORE_KEY, &key_data)?;
        // Strip "neteasecloudmusic" prefix (17 bytes)
        let rc4_key = &key_decrypted[17..];
        let key_box = rc4_ksa(rc4_key);

        // 4. Read & decrypt metadata
        let meta_len = read_u32_le(r)? as usize;
        let metadata = if meta_len > 0 {
            let mut meta_data = vec![0u8; meta_len];
            r.read_exact(&mut meta_data)?;
            for b in &mut meta_data {
                *b ^= 0x63;
            }
            // Strip "163 key(Don't modify):" prefix (22 bytes)
            let b64_data = &meta_data[22..];
            let decoded = BASE64.decode(b64_data)?;
            let decrypted = aes128_ecb_decrypt(&MODIFY_KEY, &decoded)?;
            // Strip "music:" prefix (6 bytes)
            Some(NcmMetadata::from_decrypted(&decrypted[6..])?)
        } else {
            None
        };

        // 5. Skip CRC + image version (5 bytes)
        r.seek(SeekFrom::Current(5))?;

        // 6. Read cover image
        let cover_frame_len = read_u32_le(r)?;
        let image_size = read_u32_le(r)?;
        let cover_image = if image_size > 0 {
            let mut img = vec![0u8; image_size as usize];
            r.read_exact(&mut img)?;
            // Skip padding
            let padding = i64::from(cover_frame_len) - i64::from(image_size);
            if padding > 0 {
                r.seek(SeekFrom::Current(padding))?;
            }
            Some(img)
        } else {
            if cover_frame_len > 0 {
                r.seek(SeekFrom::Current(i64::from(cover_frame_len)))?;
            }
            None
        };

        // 7. Record audio offset
        let audio_offset = r.stream_position()?;

        // 8. Detect format from first 3 decrypted bytes
        let mut header = [0u8; 3];
        r.read_exact(&mut header)?;
        for (i, b) in header.iter_mut().enumerate() {
            *b ^= rc4_stream_byte(&key_box, i);
        }
        let format = if header == [0x49, 0x44, 0x33] {
            AudioFormat::Mp3
        } else {
            AudioFormat::Flac
        };

        Ok(Self {
            metadata,
            cover_image,
            format,
            key_box,
            audio_offset,
        })
    }

    /// Construct from pre-parsed parts (for FFI use).
    pub fn from_parts(key_box: [u8; 256], audio_offset: u64) -> Self {
        Self {
            metadata: None,
            cover_image: None,
            format: AudioFormat::Mp3, // will be overwritten by actual data
            key_box,
            audio_offset,
        }
    }

    /// Decrypt and write the audio stream.
    pub fn dump_audio<R: Read + Seek, W: Write>(&self, r: &mut R, w: &mut W) -> Result<()> {
        r.seek(SeekFrom::Start(self.audio_offset))?;

        let mut buf = vec![0u8; 0x8000];
        let mut offset = 0usize;

        loop {
            let n = r.read(&mut buf)?;
            if n == 0 {
                break;
            }
            for (i, byte) in buf[..n].iter_mut().enumerate() {
                *byte ^= rc4_stream_byte(&self.key_box, offset + i);
            }
            w.write_all(&buf[..n])?;
            offset += n;
        }

        Ok(())
    }
}

fn read_u32_le<R: Read>(r: &mut R) -> Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}
