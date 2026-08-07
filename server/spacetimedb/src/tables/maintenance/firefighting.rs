//! 消防设施检查表定义（**已弃置**）。
//!
//! 本表一行是一个"检查点"，把灭火器、消防栓、安全通道三类设施的状态混装在
//! 一行里，已被 `firefighting_asset`（一行一个设施）+ `firefighting_inspection`
//! （巡检）取代，见 `docs/消防设施台账与扫码巡检.md`。生产库中本表为空；因
//! SpacetimeDB 不允许删除已发布的表（ARCHITECTURE.md §2.6.1），表定义保留为
//! 空壳，不再有任何 Reducer 或 View 读写它。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = firefighting,
    index(accessor = firefighting_by_customer, btree(columns = [customer_id])),
    index(accessor = firefighting_by_park, btree(columns = [park_id])),
    index(accessor = firefighting_by_check_time, btree(columns = [check_time]))
)]
pub struct Firefighting {
    #[primary_key]
    #[auto_inc]
    pub firefighting_id: u64,
    pub customer_id: String,
    pub address: Option<String>,
    pub firefighting_name: Option<String>,
    pub extinguisher: String,
    pub hydrant: String,
    pub fire_exit: String,
    pub checker: String,
    pub check_time: Timestamp,
    pub remark: Option<String>,
    pub factory_id: Option<u64>,
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
