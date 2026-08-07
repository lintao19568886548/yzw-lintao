//! 招商雷达评分规则表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `lead_score_rule` 表。
#[spacetimedb::table(
    accessor = lead_score_rule,
    index(accessor = lead_score_rule_by_customer, btree(columns = [customer_id])),
    index(accessor = lead_score_rule_by_customer_code, btree(columns = [customer_id, rule_code])),
    index(accessor = lead_score_rule_by_event_type, btree(columns = [event_type]))
)]
pub struct LeadScoreRule {
    #[primary_key]
    #[auto_inc]
    pub rule_id: u64,
    pub customer_id: String,
    pub rule_code: String,
    pub rule_name: String,
    pub event_type: Option<String>,
    /// 对应旧表的 `keyword_json`，改为强类型列表供确定性匹配。
    pub keywords: Vec<String>,
    pub score_delta: i32,
    pub enabled: bool,
    pub rule_description: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
