//! 爬虫执行任务表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `crawler_task` 表。
#[spacetimedb::table(
    accessor = crawler_task,
    index(accessor = crawler_task_by_customer, btree(columns = [customer_id])),
    index(accessor = crawler_task_by_source, btree(columns = [source_id])),
    index(accessor = crawler_task_by_status, btree(columns = [status])),
    index(accessor = crawler_task_by_created_at, btree(columns = [created_at]))
)]
pub struct CrawlerTask {
    #[primary_key]
    #[auto_inc]
    pub task_id: u64,
    pub customer_id: String,
    pub source_id: u64,
    pub task_type: String,
    pub status: String,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
    pub crawl_started_at: Option<Timestamp>,
    pub crawl_ended_at: Option<Timestamp>,
    pub fetched_count: u64,
    pub created_lead_count: u64,
    pub updated_lead_count: u64,
    pub skipped_count: u64,
    pub error_message: Option<String>,
    pub retry_count: u32,
    pub max_retry_count: u32,
    pub next_retry_at: Option<Timestamp>,
    pub skip_reason: Option<String>,
    pub request_config_json: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
