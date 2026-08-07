//! 招商雷达爬虫数据源表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `crawler_source` 表。
#[spacetimedb::table(
    accessor = crawler_source,
    index(accessor = crawler_source_by_customer, btree(columns = [customer_id])),
    index(accessor = crawler_source_by_customer_code, btree(columns = [customer_id, source_code])),
    index(accessor = crawler_source_by_type, btree(columns = [source_type]))
)]
pub struct CrawlerSource {
    #[primary_key]
    #[auto_inc]
    pub source_id: u64,
    pub customer_id: String,
    pub source_code: String,
    pub source_name: String,
    pub source_type: String,
    pub base_url: String,
    pub robots_url: Option<String>,
    pub enabled: bool,
    pub crawl_interval_minutes: u32,
    pub rate_limit_per_minute: u32,
    /// 原 MySQL JSON 字段改为强类型列表。
    pub allowed_paths: Vec<String>,
    pub blocked_paths: Vec<String>,
    pub keyword_include: Vec<String>,
    pub keyword_exclude: Vec<String>,
    pub region_scope: Vec<String>,
    pub last_crawled_at: Option<Timestamp>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
