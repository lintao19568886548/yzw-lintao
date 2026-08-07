//! 企业线索评分明细表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `lead_score_breakdown` 表。
#[spacetimedb::table(
    accessor = lead_score_breakdown,
    index(accessor = lead_score_breakdown_by_customer, btree(columns = [customer_id])),
    index(accessor = lead_score_breakdown_by_lead, btree(columns = [lead_id])),
    index(accessor = lead_score_breakdown_by_rule, btree(columns = [rule_id]))
)]
pub struct LeadScoreBreakdown {
    #[primary_key]
    #[auto_inc]
    pub breakdown_id: u64,
    pub customer_id: String,
    /// 线上缺失原设计的雷达潜客表，因此明确关联现有企业线索。
    pub lead_id: u64,
    pub event_id: Option<u64>,
    pub rule_id: u64,
    /// 保存规则快照，确保规则修改后历史明细仍可解释。
    pub rule_code: String,
    pub rule_name: String,
    pub score_delta: i32,
    pub reason: String,
    pub created_at: Timestamp,
}
