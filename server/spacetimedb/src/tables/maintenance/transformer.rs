//! 变压器检查表定义（**已弃置**）。
//!
//! 本表把设备信息和巡检信息混装在一行里，已被 `transformer_asset`（资产）+
//! `transformer_inspection`（巡检）取代，见 `docs/变压器台账与扫码巡检.md`。
//! 生产库中本表为空；因 SpacetimeDB 不允许删除已发布的表（ARCHITECTURE.md
//! §2.6.1），表定义保留为空壳，不再有任何 Reducer 或 View 读写它。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = transformer,
    index(accessor = transformer_by_customer, btree(columns = [customer_id])),
    index(accessor = transformer_by_park, btree(columns = [park_id])),
    index(accessor = transformer_by_check_time, btree(columns = [check_time]))
)]
pub struct Transformer {
    #[primary_key]
    #[auto_inc]
    pub transformer_id: u64,
    pub customer_id: String,
    pub transformer_name: Option<String>,
    pub address: Option<String>,
    pub contact: Option<String>,
    pub checker: Option<String>,
    pub status: String,
    pub specifications: String,
    pub check_time: Timestamp,
    pub remark: Option<String>,
    pub factory_id: Option<u64>,
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
