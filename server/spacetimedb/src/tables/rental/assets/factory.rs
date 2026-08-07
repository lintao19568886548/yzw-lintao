//! 厂房表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `factory` 表。
#[spacetimedb::table(
    accessor = factory,
    index(accessor = factory_by_customer, btree(columns = [customer_id])),
    index(accessor = factory_by_park, btree(columns = [park_id]))
)]
pub struct Factory {
    #[primary_key]
    #[auto_inc]
    pub factory_id: u64,
    pub customer_id: String,
    pub factory_name: String,
    /// 使用 `0` 表示尚未分配园区，使该字段可以建立索引。
    pub park_id: u64,
    /// MySQL 字段类型是 `DATE`，这里保留 `YYYY-MM-DD` 日期语义。
    pub build_date: Option<String>,
    pub description: Option<String>,
    pub is_own: bool,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
