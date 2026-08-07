//! 短信认证使用的密码学和供应商协议实现。

mod crypto;
mod provider;

pub(crate) use crypto::{generate_verification_code, hash_verification_code};
pub(crate) use provider::{send_personal_message, send_template_message, send_verification_code};
