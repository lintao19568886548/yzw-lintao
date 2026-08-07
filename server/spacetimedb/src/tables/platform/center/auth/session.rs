//! SpacetimeDB 身份与业务账号之间的登录会话。

use spacetimedb::{Identity, Timestamp};

/// 一个设备身份同时只登录一个账号，同一账号可以拥有多个设备会话。
#[spacetimedb::table(
    accessor = user_session,
    index(accessor = user_session_by_center_user, btree(columns = [center_user_id])),
    index(accessor = user_session_by_expiry, btree(columns = [expires_at]))
)]
pub struct UserSession {
    #[primary_key]
    pub identity: Identity,
    pub center_user_id: u64,
    pub expires_at: Timestamp,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
