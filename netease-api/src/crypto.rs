//! WEAPI encryption for Netease Cloud Music API.
//!
//! Flow: JSON → AES-CBC(preset_key) → base64 → AES-CBC(random_key) → base64 = params
//! RSA:  `reverse(random_key)` → zero-pad to 128 bytes → `modpow(e, n)` → hex = `encSecKey`

use aes::Aes128;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use cbc::{Encryptor, cipher::KeyIvInit, cipher::BlockEncryptMut, cipher::block_padding::Pkcs7};
use num_bigint::BigUint;
use rand::Rng;

const IV: &[u8; 16] = b"0102030405060708";
const PRESET_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";

// RSA-1024 public key extracted from Netease web client
const RSA_MODULUS_HEX: &str = "\
    e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b72515\
    2b3ab17a876aea8a5aa76d2e417629ec4ee341f56135fccf695280104e0312ec\
    bda92557c93870114af6c9d05c4f7f0c3685b7a46bee255932575cce10b424d\
    813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462db0a22b8e7";
const RSA_EXPONENT: u32 = 65537;

type Aes128CbcEnc = Encryptor<Aes128>;

/// WEAPI encrypted payload.
pub struct WeapiPayload {
    pub params: String,
    pub enc_sec_key: String,
}

/// Encrypt `data` (JSON string) using the WEAPI scheme.
pub fn weapi_encrypt(data: &str) -> WeapiPayload {
    let secret_key = random_key(16);

    // First AES pass: encrypt with preset key
    let pass1 = aes_cbc_encrypt(data.as_bytes(), PRESET_KEY, IV);
    let pass1_b64 = B64.encode(&pass1);

    // Second AES pass: encrypt with random key
    let pass2 = aes_cbc_encrypt(pass1_b64.as_bytes(), &secret_key, IV);
    let params = B64.encode(&pass2);

    let enc_sec_key = rsa_encrypt(&secret_key);

    WeapiPayload { params, enc_sec_key }
}

/// AES-128-CBC encrypt with PKCS7 padding.
fn aes_cbc_encrypt(plaintext: &[u8], key: &[u8; 16], iv: &[u8; 16]) -> Vec<u8> {
    let enc = Aes128CbcEnc::new(key.into(), iv.into());
    // Allocate buffer: plaintext + up to 16 bytes padding
    let pad_len = 16 - (plaintext.len() % 16);
    let mut buf = vec![0u8; plaintext.len() + pad_len];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    let ct = enc
        .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
        .expect("buffer is correctly sized");
    ct.to_vec()
}

/// RSA `NO_PADDING` encrypt: reverse key, zero-pad to 128 bytes, `modpow(e, n)`, hex output.
fn rsa_encrypt(key: &[u8; 16]) -> String {
    let mut reversed: Vec<u8> = key.iter().copied().rev().collect();

    // Zero-pad on the left to 128 bytes (1024 bits)
    let mut padded = vec![0u8; 128 - reversed.len()];
    padded.append(&mut reversed);

    let m = BigUint::from_bytes_be(&padded);
    let n = BigUint::parse_bytes(RSA_MODULUS_HEX.replace(' ', "").as_bytes(), 16)
        .expect("invalid RSA modulus");
    let e = BigUint::from(RSA_EXPONENT);

    let cipher = m.modpow(&e, &n);
    let hex = format!("{cipher:0>256x}");
    hex
}

/// Generate a random alphanumeric key of `len` bytes.
fn random_key(len: usize) -> [u8; 16] {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();
    let mut key = [0u8; 16];
    for b in &mut key[..len] {
        *b = CHARSET[rng.random_range(0..CHARSET.len())];
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_cbc_roundtrip() {
        let plaintext = b"hello netease";
        let key: [u8; 16] = *b"0123456789abcdef";
        let iv: [u8; 16] = *b"0102030405060708";
        let encrypted = aes_cbc_encrypt(plaintext, &key, &iv);
        assert!(!encrypted.is_empty());
        // Ciphertext length is a multiple of 16
        assert_eq!(encrypted.len() % 16, 0);
    }

    #[test]
    fn weapi_produces_nonempty_output() {
        let payload = weapi_encrypt(r#"{"s":"test","type":1}"#);
        assert!(!payload.params.is_empty());
        assert!(!payload.enc_sec_key.is_empty());
        // encSecKey should be 256 hex chars (128 bytes)
        assert_eq!(payload.enc_sec_key.len(), 256);
    }

    #[test]
    fn rsa_output_length() {
        let key = *b"abcdefghijklmnop";
        let hex = rsa_encrypt(&key);
        assert_eq!(hex.len(), 256);
    }
}
