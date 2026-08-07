//! 招商雷达企业信号事件表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `signal_event` 表。
#[spacetimedb::table(
    accessor = signal_event,
    index(accessor = signal_event_by_customer, btree(columns = [customer_id])),
    index(accessor = signal_event_by_customer_hash, btree(columns = [customer_id, content_hash])),
    index(accessor = signal_event_by_company, btree(columns = [company_name])),
    index(accessor = signal_event_by_type, btree(columns = [event_type])),
    index(accessor = signal_event_by_status, btree(columns = [status])),
    index(accessor = signal_event_by_source_type, btree(columns = [source_type]))
)]
pub struct SignalEvent {
    #[primary_key]
    #[auto_inc]
    pub event_id: u64,
    pub customer_id: String,
    /// 显式关联企业画像；信号也允许先于画像产生。
    pub profile_id: Option<u64>,
    pub enterprise_id: Option<u64>,
    pub company_name: String,
    pub event_type: String,
    pub event_title: String,
    pub event_summary: Option<String>,
    pub event_time: Option<Timestamp>,
    pub source_type: String,
    pub source_name: String,
    pub source_url: String,
    pub confidence_score: i32,
    pub status: String,
    /// 对应尚待迁移的外部企业线索主键。
    pub related_external_lead_id: Option<u64>,
    /// 对应尚待迁移的雷达潜客主键。
    pub related_radar_lead_id: Option<u64>,
    pub content_hash: String,
    pub raw_payload_json: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
