//! 招商租户收支记录表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `investment_tenant` 表。
#[spacetimedb::table(
    accessor = investment_tenant,
    index(accessor = investment_tenant_by_customer, btree(columns = [customer_id])),
    index(accessor = investment_tenant_by_transaction_time, btree(columns = [transaction_time]))
)]
pub struct InvestmentTenant {
    #[primary_key]
    #[auto_inc]
    pub tenant_id: u64,
    pub customer_id: String,
    pub bill_name: String,
    pub bill_category: String,
    /// 金额统一保存为分，避免浮点数精度损失。
    pub amount_cents: i64,
    pub transaction_type: String,
    pub transaction_time: Timestamp,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
