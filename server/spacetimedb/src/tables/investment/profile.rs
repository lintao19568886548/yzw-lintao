//! 招商雷达企业画像表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `enterprise_profile` 表。
#[spacetimedb::table(
    accessor = enterprise_profile,
    index(accessor = enterprise_profile_by_customer, btree(columns = [customer_id])),
    index(accessor = enterprise_profile_by_customer_company, btree(columns = [customer_id, company_name])),
    index(accessor = enterprise_profile_by_enterprise, btree(columns = [enterprise_id])),
    index(accessor = enterprise_profile_by_industry, btree(columns = [industry_name])),
    index(accessor = enterprise_profile_by_city, btree(columns = [region_city]))
)]
pub struct EnterpriseProfile {
    #[primary_key]
    #[auto_inc]
    pub profile_id: u64,
    pub customer_id: String,
    pub enterprise_id: Option<u64>,
    pub company_name: String,
    pub unified_social_credit_code: Option<String>,
    pub industry_name: Option<String>,
    /// 保留原系统的 JSON 数组格式，便于后续数据导入。
    pub industry_tags_json: Option<String>,
    pub region_province: Option<String>,
    pub region_city: Option<String>,
    pub region_district: Option<String>,
    /// 注册资本统一保存为分，避免浮点数精度损失。
    pub registered_capital_cents: Option<i64>,
    pub employee_scale: Option<String>,
    pub business_scope: Option<String>,
    pub address: Option<String>,
    pub last_signal_time: Option<Timestamp>,
    pub signal_count: u64,
    pub latest_intent_type: Option<String>,
    pub profile_completeness: i32,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
