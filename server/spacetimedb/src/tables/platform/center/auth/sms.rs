//! 短信供应商配置和一次性登录验证码。

use spacetimedb::{Identity, Timestamp};

/// 仅 Module 和数据库管理员可读取的短信供应商配置。
#[spacetimedb::table(accessor = sms_provider_config)]
pub struct SmsProviderConfig {
    #[primary_key]
    pub config_key: String,
    pub api_host: String,
    pub app_id: String,
    pub merchant_id: String,
    pub version: String,
    pub sign_type: String,
    pub secret_key: String,
    pub template_id: String,
    pub message_type: String,
    /// 用于生成和散列验证码的 Module 私有密钥。
    pub code_pepper: String,
    pub code_ttl_seconds: u32,
    pub resend_interval_seconds: u32,
    pub max_verify_attempts: u32,
    pub updated_at: Timestamp,
}

/// 每个手机号只保留最后一次发送的验证码摘要。
#[spacetimedb::table(
    accessor = sms_login_challenge,
    index(accessor = sms_challenge_by_identity, btree(columns = [requested_by]))
)]
pub struct SmsLoginChallenge {
    #[primary_key]
    pub phone_number: String,
    pub requested_by: Identity,
    pub code_hash: String,
    /// `true` 表示短信 HTTP 请求仍在执行。
    pub sending: bool,
    pub attempts: u32,
    pub sent_at: Timestamp,
    pub expires_at: Timestamp,
}
