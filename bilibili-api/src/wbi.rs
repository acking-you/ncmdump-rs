//! WBI signature for Bilibili API requests.
//!
//! Ported from `biu/electron/ipc/api/wbi.ts`.
//! The WBI scheme signs query parameters with a time-based HMAC-like hash
//! derived from rotating `img_key`/`sub_key` pairs fetched from the nav API.

use md5::{Digest, Md5};

/// Character shuffle table for mixin key derivation.
/// Stable since 2023; maps indices of the concatenated `img_key`+`sub_key`.
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

/// Derive the mixin key from `img_key` + `sub_key` by shuffling characters.
pub fn get_mixin_key(img_key: &str, sub_key: &str) -> String {
    let orig: Vec<char> = format!("{img_key}{sub_key}").chars().collect();
    MIXIN_KEY_ENC_TAB
        .iter()
        .filter_map(|&i| orig.get(i).copied())
        .take(32)
        .collect()
}

/// Characters to strip from parameter values before signing.
const CHR_FILTER: &[char] = &['!', '\'', '(', ')', '*'];

/// Sign a set of query parameters with WBI.
///
/// Returns the signed parameters as a sorted query string with `wts` and `w_rid` appended.
pub fn sign_params(
    params: &[(String, String)],
    img_key: &str,
    sub_key: &str,
) -> Vec<(String, String)> {
    let mixin_key = get_mixin_key(img_key, sub_key);
    let wts = chrono::Utc::now().timestamp().to_string();

    // Collect all params + wts, sort by key.
    let mut all: Vec<(String, String)> = params.to_vec();
    all.push(("wts".into(), wts));
    all.sort_by(|a, b| a.0.cmp(&b.0));

    // Filter special chars from values, build query string for hashing.
    let filtered: Vec<(String, String)> = all
        .iter()
        .map(|(k, v)| {
            let clean: String = v.chars().filter(|c| !CHR_FILTER.contains(c)).collect();
            (k.clone(), clean)
        })
        .collect();

    let query: String = filtered
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    // MD5(query + mixin_key) → w_rid
    let mut hasher = Md5::new();
    hasher.update(format!("{query}{mixin_key}"));
    let w_rid = format!("{:x}", hasher.finalize());

    filtered
        .into_iter()
        .chain(std::iter::once(("w_rid".into(), w_rid)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixin_key_length() {
        let key = get_mixin_key(
            "abcdefghijklmnopqrstuvwxyz012345",
            "6789abcdefghijklmnopqrstuvwxyz01",
        );
        assert_eq!(key.len(), 32);
    }
}
