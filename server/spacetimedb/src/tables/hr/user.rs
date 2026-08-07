//! 租户业务用户表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `user` 表。
/// 密码不会进入订阅模型；登录身份统一绑定中心用户。
#[spacetimedb::table(
    accessor = system_user,
    index(accessor = business_user_by_customer, btree(columns = [customer_id])),
    index(accessor = business_user_by_customer_username, btree(columns = [customer_id, username]))
)]
pub struct SystemUser {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub username: String,
    /// 原先由物理数据库隔离的租户标识。
    pub customer_id: String,
    pub real_name: String,
    pub home_path: Option<String>,
    pub phone: Option<String>,
    pub customer_type: Option<String>,
    pub status: i8,
    pub token_version: u32,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
    /// 头像 URL（R2 公网地址），个人中心自助上传。
    ///
    /// 表尾追加列：按 §2.6.1 实测规则，带 `#[default(...)]` 标注的追加列属
    /// 安全迁移，存量行填入默认值；`Option` 的默认值必须用 turbofish 标注类型。
    #[default(None::<String>)]
    pub avatar_url: Option<String>,
}
