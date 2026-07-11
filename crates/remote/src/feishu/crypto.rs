use aes::Aes256;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use cbc::Decryptor;
use cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

type Aes256CbcDec = Decryptor<Aes256>;

#[derive(Debug, Error)]
pub enum FeishuCryptoError {
    #[error("invalid base64 ciphertext")]
    Base64,
    #[error("ciphertext too short")]
    TooShort,
    #[error("AES decrypt failed")]
    Decrypt,
    #[error("decrypted payload is not valid UTF-8")]
    Utf8,
}

/// Feishu Encrypt Key event decryption (AES-256-CBC).
/// Key = SHA256(encrypt_key); IV = first 16 bytes of base64-decoded ciphertext.
pub fn decrypt_event(encrypt_key: &str, encrypt: &str) -> Result<String, FeishuCryptoError> {
    let key = Sha256::digest(encrypt_key.as_bytes());
    let raw = BASE64
        .decode(encrypt.trim())
        .map_err(|_| FeishuCryptoError::Base64)?;
    if raw.len() < 17 {
        return Err(FeishuCryptoError::TooShort);
    }
    let (iv, ciphertext) = raw.split_at(16);
    let mut buf = ciphertext.to_vec();
    let decrypted = Aes256CbcDec::new_from_slices(&key, iv)
        .map_err(|_| FeishuCryptoError::Decrypt)?
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| FeishuCryptoError::Decrypt)?;
    String::from_utf8(decrypted.to_vec()).map_err(|_| FeishuCryptoError::Utf8)
}

/// Feishu signature verification when Encrypt Key is configured.
/// `sha256(timestamp + nonce + encrypt_key + body) == X-Lark-Signature`
pub fn verify_signature(
    encrypt_key: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
    signature: &str,
) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(timestamp.as_bytes());
    hasher.update(nonce.as_bytes());
    hasher.update(encrypt_key.as_bytes());
    hasher.update(body);
    let expected = hex::encode(hasher.finalize());
    let provided = signature.trim();
    if expected.len() != provided.len() {
        return false;
    }
    expected.as_bytes().ct_eq(provided.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_stable() {
        let ok = verify_signature("key", "123", "nonce", b"{}", "deadbeef");
        assert!(!ok);
        let mut hasher = Sha256::new();
        hasher.update(b"123");
        hasher.update(b"nonce");
        hasher.update(b"key");
        hasher.update(b"{}");
        let expected = hex::encode(hasher.finalize());
        assert!(verify_signature("key", "123", "nonce", b"{}", &expected));
    }
}
