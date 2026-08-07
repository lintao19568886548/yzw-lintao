//! API 操作审计日志表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `api_log` 表。
#[spacetimedb::table(
    accessor = api_log,
    index(accessor = api_log_by_customer, btree(columns = [customer_id])),
    index(accessor = api_log_by_customer_user, btree(columns = [customer_id, user_id])),
    index(accessor = api_log_by_customer_time, btree(columns = [customer_id, request_time]))
)]
pub struct ApiLog {
    #[primary_key]
    #[auto_inc]
    pub log_id: u64,
    pub customer_id: String,
    pub method: String,
    pub path: Option<String>,
    pub referer_path: String,
    pub item_name: Option<String>,
    pub username: String,
    /// 关联当前租户业务用户，避免仅依赖可能重复的用户名。
    pub user_id: u64,
    pub request_time: Timestamp,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
