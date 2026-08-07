//! 爬虫任务日志表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `crawler_task_log` 表。
#[spacetimedb::table(
    accessor = crawler_task_log,
    index(accessor = crawler_task_log_by_customer, btree(columns = [customer_id])),
    index(accessor = crawler_task_log_by_task, btree(columns = [task_id])),
    index(accessor = crawler_task_log_by_stage, btree(columns = [stage]))
)]
pub struct CrawlerTaskLog {
    #[primary_key]
    #[auto_inc]
    pub log_id: u64,
    pub customer_id: String,
    pub task_id: u64,
    pub level: String,
    pub stage: String,
    pub message: String,
    pub detail_json: Option<String>,
    pub created_at: Timestamp,
}
