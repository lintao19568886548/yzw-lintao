//! 招商跟进记录表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `investment` 表。
#[spacetimedb::table(
    accessor = investment,
    index(accessor = investment_by_customer, btree(columns = [customer_id])),
    index(accessor = investment_by_park, btree(columns = [park_id])),
    index(accessor = investment_by_phone, btree(columns = [phone_number]))
)]
pub struct Investment {
    #[primary_key]
    #[auto_inc]
    pub investment_id: u64,
    pub customer_id: String,
    pub agent_name: Option<String>,
    pub tenant_name: Option<String>,
    pub intent_level: String,
    pub intent_area: Option<i64>,
    pub progress: String,
    pub phone_number: Option<String>,
    pub meeting_time: Timestamp,
    pub remark: Option<String>,
    /// 使用 `0` 表示尚未分配园区。
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
