//! 中心用户刷新令牌会话表。

use spacetimedb::Timestamp;

/// 对应中心库 `refresh_token` 表，仅保存令牌哈希，不保存原始令牌。
#[spacetimedb::table(
    accessor = refresh_token,
    index(accessor = refresh_token_by_user, btree(columns = [user_id])),
    index(accessor = refresh_token_by_jti, btree(columns = [jti])),
    index(accessor = refresh_token_by_expiry, btree(columns = [expires_at]))
)]
pub struct RefreshToken {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub user_id: u64,
    pub jti: String,
    pub token_hash: String,
    pub expires_at: Timestamp,
    pub revoked_at: Option<Timestamp>,
    pub replaced_by_jti: Option<String>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
