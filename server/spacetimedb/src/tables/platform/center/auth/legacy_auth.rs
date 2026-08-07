//! 原登录服务与 SpacetimeDB 之间的私有信任配置。

use spacetimedb::Timestamp;

/// 保存原后台 Access Token 的验证密钥，不向客户端公开。
#[spacetimedb::table(accessor = legacy_auth_config)]
pub struct LegacyAuthConfig {
    #[primary_key]
    pub config_key: String,
    pub access_token_secret: String,
    pub updated_at: Timestamp,
}
