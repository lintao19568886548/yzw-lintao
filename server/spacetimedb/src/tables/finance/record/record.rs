//! 财务收支流水表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `finance` 表。
#[spacetimedb::table(
    accessor = finance,
    index(accessor = finance_by_customer, btree(columns = [customer_id])),
    index(accessor = finance_by_park, btree(columns = [park_id])),
    index(accessor = finance_by_transaction_time, btree(columns = [transaction_time]))
)]
pub struct Finance {
    #[primary_key]
    #[auto_inc]
    pub finance_id: u64,
    pub customer_id: String,
    pub bill_name: String,
    pub bill_category: String,
    pub amount_cents: i64,
    pub transaction_type: String,
    pub transaction_time: Timestamp,
    pub remark: Option<String>,
    /// 使用 `0` 表示未分配园区。
    pub park_id: u64,
    pub status: i32,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
