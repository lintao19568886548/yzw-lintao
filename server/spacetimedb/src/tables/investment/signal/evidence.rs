//! 企业信号证据表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `signal_evidence` 表。
#[spacetimedb::table(
    accessor = signal_evidence,
    index(accessor = signal_evidence_by_customer, btree(columns = [customer_id])),
    index(accessor = signal_evidence_by_event, btree(columns = [event_id])),
    index(accessor = signal_evidence_by_event_hash, btree(columns = [event_id, content_hash])),
    index(accessor = signal_evidence_by_hash, btree(columns = [content_hash]))
)]
pub struct SignalEvidence {
    #[primary_key]
    #[auto_inc]
    pub evidence_id: u64,
    pub customer_id: String,
    pub event_id: u64,
    pub evidence_type: String,
    pub source_title: Option<String>,
    pub source_link: String,
    pub raw_text: Option<String>,
    pub matched_keywords_json: Option<String>,
    pub matched_sentences_json: Option<String>,
    pub score_delta: i32,
    pub content_hash: String,
    pub published_at: Option<Timestamp>,
    pub crawled_at: Option<Timestamp>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
