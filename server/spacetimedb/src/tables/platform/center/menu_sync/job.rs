//! 菜单模板同步任务表。

use spacetimedb::Timestamp;

/// 对应中心库 `menu_template_sync_job` 表。
#[spacetimedb::table(
    accessor = menu_template_sync_job,
    index(accessor = menu_sync_job_by_scope, btree(columns = [center_scope])),
    index(accessor = menu_sync_job_by_mode_status, btree(columns = [mode, status])),
    index(accessor = menu_sync_job_by_source_customer, btree(columns = [source_customer_id])),
    index(accessor = menu_sync_job_by_target_scope, btree(columns = [target_scope])),
    index(accessor = menu_sync_job_by_target_customer, btree(columns = [target_customer_id]))
)]
pub struct MenuTemplateSyncJob {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    /// 中心库全局分区标识，固定为 `0`，供私有视图按索引读取。
    pub center_scope: u8,
    pub mode: String,
    pub source_customer_id: String,
    pub target_scope: String,
    pub target_customer_id: Option<String>,
    pub status: String,
    /// MySQL JSON 字段以规范化后的 JSON 文本保存。
    pub summary_json: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
