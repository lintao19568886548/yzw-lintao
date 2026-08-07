//! 中心用户和 SpacetimeDB 身份绑定。

use spacetimedb::{Identity, Timestamp};

/// 对应 `magic_center.user`，不迁移密码哈希。
#[spacetimedb::table(
    accessor = center_user,
    index(accessor = center_user_by_status, btree(columns = [status]))
)]
pub struct CenterUser {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub username: String,
    pub real_name: String,
    pub customer_type: Option<String>,
    pub status: i8,
    pub token_version: u32,
    pub phone: Option<String>,
    pub home_path: Option<String>,
    pub membership_trial_start_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

/// SpacetimeDB 可信身份只绑定中心用户，租户业务用户通过映射表解析。
#[spacetimedb::table(accessor = user_identity)]
pub struct UserIdentity {
    #[primary_key]
    pub identity: Identity,
    #[unique]
    pub center_user_id: u64,
    pub created_at: Timestamp,
}
