//! 菜单模板同步目标日志表。

use spacetimedb::Timestamp;

/// 对应中心库 `menu_template_sync_log` 表。
#[spacetimedb::table(
    accessor = menu_template_sync_log,
    index(accessor = menu_sync_log_by_job, btree(columns = [job_id])),
    index(accessor = menu_sync_log_by_target_customer, btree(columns = [target_customer_id])),
    index(accessor = menu_sync_log_by_status, btree(columns = [status]))
)]
pub struct MenuTemplateSyncLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub job_id: u64,
    pub target_customer_id: String,
    pub target_db_name: Option<String>,
    pub status: String,
    /// MySQL JSON 字段以规范化后的 JSON 文本保存。
    pub summary_json: Option<String>,
    /// 每个目标租户的差异明细，使用规范化后的 JSON 文本保存。
    pub details_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
