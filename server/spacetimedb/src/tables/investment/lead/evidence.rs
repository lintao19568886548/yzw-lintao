//! 外部企业线索证据表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `lead_evidence` 表。
#[spacetimedb::table(
    accessor = lead_evidence,
    index(accessor = lead_evidence_by_customer, btree(columns = [customer_id])),
    index(accessor = lead_evidence_by_lead, btree(columns = [lead_id])),
    index(accessor = lead_evidence_by_lead_hash, btree(columns = [lead_id, content_hash])),
    index(accessor = lead_evidence_by_type, btree(columns = [evidence_type]))
)]
pub struct LeadEvidence {
    #[primary_key]
    #[auto_inc]
    pub evidence_id: u64,
    pub customer_id: String,
    pub lead_id: u64,
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
