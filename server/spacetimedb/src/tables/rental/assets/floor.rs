//! 厂房楼层表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `factory_floor` 表。
#[spacetimedb::table(
    accessor = factory_floor,
    index(accessor = factory_floor_by_customer, btree(columns = [customer_id])),
    index(accessor = factory_floor_by_factory, btree(columns = [factory_id]))
)]
pub struct FactoryFloor {
    #[primary_key]
    #[auto_inc]
    pub floor_id: u64,
    pub customer_id: String,
    pub factory_id: u64,
    pub floor_name: String,
    /// 原 `decimal(5,2)` 字段乘以一百后保存。
    pub floor_height_centi_metres: Option<i64>,
    /// 原承重 `decimal(10,2)` 字段乘以一百后保存。
    pub load_bearing_centi_units: Option<i64>,
    /// 每平方米租金，以分为单位。
    pub rent_price_cents: i64,
    pub total_area_centi_square_metres: i64,
    pub description: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
