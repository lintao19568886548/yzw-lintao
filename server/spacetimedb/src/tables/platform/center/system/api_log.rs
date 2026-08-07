//! 中心库 API 操作审计日志表。

use spacetimedb::Timestamp;

/// 对应中心库 `api_log` 表。
#[spacetimedb::table(
    accessor = center_api_log,
    index(accessor = center_api_log_by_scope, btree(columns = [center_scope])),
    index(accessor = center_api_log_by_user, btree(columns = [center_user_id])),
    index(accessor = center_api_log_by_time, btree(columns = [request_time]))
)]
pub struct CenterApiLog {
    #[primary_key]
    #[auto_inc]
    pub log_id: u64,
    /// 中心库为全局单例空间，固定为 `0`，供私表视图安全订阅全量数据。
    pub center_scope: u8,
    pub method: String,
    pub path: Option<String>,
    pub referer_path: String,
    pub item_name: Option<String>,
    pub username: String,
    /// 关联中心用户，弥补 MySQL 原表只能依赖用户名的问题。
    pub center_user_id: u64,
    pub request_time: Timestamp,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
