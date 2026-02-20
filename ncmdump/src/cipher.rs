use aes::Aes128;
use ecb::cipher::{BlockDecryptMut, KeyInit, block_padding::Pkcs7};

use crate::error::{NcmError, Result};

type Aes128EcbDec = ecb::Decryptor<Aes128>;

/// AES-128-ECB decrypt with PKCS#7 unpadding.
pub fn aes128_ecb_decrypt(key: &[u8; 16], data: &[u8]) -> Result<Vec<u8>> {
    let mut buf = data.to_vec();
    Aes128EcbDec::new(key.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map(<[u8]>::to_vec)
        .map_err(|e| NcmError::Decrypt(e.to_string()))
}

/// Standard RC4 Key Scheduling Algorithm. Returns the permuted S-box.
#[allow(clippy::cast_possible_truncation)]
pub fn rc4_ksa(key: &[u8]) -> [u8; 256] {
    let mut sbox = [0u8; 256];
    for (i, slot) in sbox.iter_mut().enumerate() {
        *slot = i as u8;
    }

    let key_len = key.len();
    let mut last_byte: u8 = 0;
    let mut key_offset = 0usize;

    for i in 0..256 {
        let swap = sbox[i];
        let c = swap.wrapping_add(last_byte).wrapping_add(key[key_offset]);
        key_offset += 1;
        if key_offset >= key_len {
            key_offset = 0;
        }
        sbox[i] = sbox[c as usize];
        sbox[c as usize] = swap;
        last_byte = c;
    }

    sbox
}

/// Modified RC4 stream byte at the given offset. The `key_box` is never mutated.
#[inline]
pub fn rc4_stream_byte(key_box: &[u8; 256], offset: usize) -> u8 {
    let j = (offset + 1) & 0xff;
    let jv = key_box[j] as usize;
    key_box[(jv + key_box[(jv + j) & 0xff] as usize) & 0xff]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rc4_ksa_deterministic() {
        let key = b"hello";
        let box1 = rc4_ksa(key);
        let box2 = rc4_ksa(key);
        assert_eq!(box1, box2);
    }

    #[test]
    fn test_rc4_stream_byte_deterministic() {
        let key = b"testkey";
        let sbox = rc4_ksa(key);
        let b1 = rc4_stream_byte(&sbox, 0);
        let b2 = rc4_stream_byte(&sbox, 0);
        assert_eq!(b1, b2);
    }

    #[test]
    fn test_aes128_ecb_roundtrip() {
        let key: [u8; 16] = *b"0123456789abcdef";
        let plaintext = b"hello world!!!!!"; // exactly 16 bytes
        let encrypted = {
            use aes::Aes128;
            use ecb::cipher::{BlockEncryptMut, KeyInit, block_padding::Pkcs7};
            type Aes128EcbEnc = ecb::Encryptor<Aes128>;
            // encrypt_padded_mut needs a buffer with room for padding
            let mut buf = [0u8; 32]; // 16 bytes data + up to 16 bytes padding
            buf[..16].copy_from_slice(plaintext);
            let ct = Aes128EcbEnc::new((&key).into())
                .encrypt_padded_mut::<Pkcs7>(&mut buf, 16)
                .unwrap();
            ct.to_vec()
        };
        let decrypted = aes128_ecb_decrypt(&key, &encrypted).unwrap();
        assert_eq!(&decrypted, plaintext);
    }
}
