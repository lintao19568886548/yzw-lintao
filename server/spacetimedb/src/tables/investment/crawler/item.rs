//! 爬虫任务抓取项表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `crawler_task_item` 表。
#[spacetimedb::table(
    accessor = crawler_task_item,
    index(accessor = crawler_task_item_by_customer, btree(columns = [customer_id])),
    index(accessor = crawler_task_item_by_source, btree(columns = [source_id])),
    index(accessor = crawler_task_item_by_source_hash, btree(columns = [source_id, url_hash])),
    index(accessor = crawler_task_item_by_task, btree(columns = [last_task_id])),
    index(accessor = crawler_task_item_by_status, btree(columns = [status]))
)]
pub struct CrawlerTaskItem {
    #[primary_key]
    #[auto_inc]
    pub item_id: u64,
    pub customer_id: String,
    pub source_id: u64,
    /// 使用 `0` 表示尚未被任务领取。
    pub last_task_id: u64,
    pub source_ref_type: Option<String>,
    pub source_ref_id: Option<u64>,
    pub source_url: String,
    pub url_hash: String,
    pub status: String,
    pub retry_count: u32,
    pub max_retry_count: u32,
    pub next_retry_at: Option<Timestamp>,
    pub last_http_status: Option<i32>,
    pub last_error: Option<String>,
    pub skip_reason: Option<String>,
    pub published_at: Option<Timestamp>,
    pub last_started_at: Option<Timestamp>,
    pub last_finished_at: Option<Timestamp>,
    pub last_success_at: Option<Timestamp>,
    pub response_hash: Option<String>,
    pub parsed_payload_json: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
