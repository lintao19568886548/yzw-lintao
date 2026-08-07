//! 升降机检查记录表定义（**已弃置**）。
//!
//! 本表把设备信息和巡检信息混装在一行里，已被 `elevator_asset`（资产）+
//! `elevator_inspection`（巡检）取代，见 `docs/电梯台账与扫码巡检.md`。
//! 生产库中本表为空；因 SpacetimeDB 不允许删除已发布的表（ARCHITECTURE.md
//! §2.6.1），表定义保留为空壳，不再有任何 Reducer 或 View 读写它。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = elevator,
    index(accessor = elevator_by_customer, btree(columns = [customer_id])),
    index(accessor = elevator_by_factory, btree(columns = [factory_id])),
    index(accessor = elevator_by_park, btree(columns = [park_id]))
)]
pub struct Elevator {
    #[primary_key]
    #[auto_inc]
    pub elevator_id: u64,
    pub customer_id: String,
    pub name: Option<String>,
    pub status: Option<String>,
    pub size: Option<String>,
    /// MySQL `decimal(10,2)` 乘以一百保存。
    pub load_capacity_centi_units: Option<i64>,
    pub production_date: Option<Timestamp>,
    pub checker: String,
    pub check_time: Timestamp,
    pub remark: Option<String>,
    pub factory_id: u64,
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
