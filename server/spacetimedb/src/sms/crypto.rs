//! 使用 Module 私有密钥弥补 Procedure 随机源可预测的问题。

use hmac::{Hmac, Mac};
use sha2::Sha256;
use spacetimedb::Identity;

/// 将公开时间、调用身份和随机值放入 HMAC，最终安全性来自私有 Pepper。
pub(crate) fn generate_verification_code(
    pepper: &str,
    timestamp_micros: i64,
    identity: Identity,
    nonce: u64,
) -> String {
    let mut signer =
        Hmac::<Sha256>::new_from_slice(pepper.as_bytes()).expect("HMAC 接受任意长度的验证码密钥");
    signer.update(&timestamp_micros.to_be_bytes());
    signer.update(&identity.to_byte_array());
    signer.update(&nonce.to_be_bytes());
    let digest = signer.finalize().into_bytes();
    let number = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 1_000_000;
    format!("{number:06}")
}

/// 私有表只保存验证码 HMAC 摘要，不保存短信中的明文数字。
pub(crate) fn hash_verification_code(pepper: &str, phone_number: &str, code: &str) -> String {
    let mut signer =
        Hmac::<Sha256>::new_from_slice(pepper.as_bytes()).expect("HMAC 接受任意长度的验证码密钥");
    signer.update(phone_number.as_bytes());
    signer.update(b":");
    signer.update(code.as_bytes());
    signer
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 验证码固定为六位数字且受私有密钥影响() {
        let identity = Identity::ZERO;
        let first = generate_verification_code("密钥一", 100, identity, 7);
        let second = generate_verification_code("密钥二", 100, identity, 7);
        assert_eq!(first.len(), 6);
        assert!(first.bytes().all(|byte| byte.is_ascii_digit()));
        assert_ne!(first, second);
    }
}
