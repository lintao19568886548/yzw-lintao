//! 招商雷达外部企业线索表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `company_lead` 表。
#[spacetimedb::table(
    accessor = company_lead,
    index(accessor = company_lead_by_customer, btree(columns = [customer_id])),
    index(accessor = company_lead_by_customer_dedupe, btree(columns = [customer_id, dedupe_key])),
    index(accessor = company_lead_by_company, btree(columns = [company_name])),
    index(accessor = company_lead_by_status, btree(columns = [status])),
    index(accessor = company_lead_by_source_name, btree(columns = [source_name]))
)]
pub struct CompanyLead {
    #[primary_key]
    #[auto_inc]
    pub lead_id: u64,
    pub customer_id: String,
    /// 对应尚待迁移的爬虫来源主键。
    pub source_id: Option<u64>,
    pub source_name: String,
    pub source_url: String,
    pub source_title: Option<String>,
    pub source_type: String,
    pub company_name: String,
    pub lead_title: String,
    pub summary: Option<String>,
    pub demand_type: String,
    pub confidence_score: i32,
    pub confidence_level: String,
    pub industry_name: Option<String>,
    pub region_province: Option<String>,
    pub region_city: Option<String>,
    pub region_district: Option<String>,
    /// 保留原系统的 JSON 数组格式。
    pub hit_keywords_json: Option<String>,
    /// 始终由有效证据数量计算，客户端不能直接指定。
    pub evidence_count: u64,
    /// 评分规则累计分，限制在 0 到 100。
    pub rule_score: i32,
    pub priority_level: String,
    pub status: String,
    pub owner_user_id: Option<u64>,
    pub invalid_reason: Option<String>,
    pub remark: Option<String>,
    pub dedupe_key: String,
    /// 对应尚待迁移的雷达潜客主键。
    pub converted_radar_lead_id: Option<u64>,
    pub converted_at: Option<Timestamp>,
    pub first_seen_at: Option<Timestamp>,
    pub last_seen_at: Option<Timestamp>,
    pub crawled_at: Option<Timestamp>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
