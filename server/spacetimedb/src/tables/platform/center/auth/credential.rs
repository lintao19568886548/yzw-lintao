//! 中心账号的私有认证凭据。

use spacetimedb::Timestamp;

/// 密码哈希与公开用户资料分开保存，本表不会直接订阅到客户端。
#[spacetimedb::table(accessor = user_credential)]
pub struct UserCredential {
    #[primary_key]
    pub center_user_id: u64,
    pub password_hash: String,
    pub password_updated_at: Timestamp,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
