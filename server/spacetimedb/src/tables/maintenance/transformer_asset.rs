//! 变压器资产台账表定义。
//!
//! 一行是一台真实设备。与旧 `transformer` 表（资产与巡检混装一行，已弃置）不同，
//! 设备本体在这里长期存在，巡检事件在 `transformer_inspection` 表里只增不删地
//! 累积。完整设计见 `docs/变压器台账与扫码巡检.md`。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = transformer_asset,
    index(accessor = transformer_asset_by_customer, btree(columns = [customer_id])),
    index(accessor = transformer_asset_by_park, btree(columns = [park_id])),
    index(accessor = transformer_asset_by_factory, btree(columns = [factory_id]))
)]
pub struct TransformerAsset {
    #[primary_key]
    #[auto_inc]
    pub asset_id: u64,
    pub customer_id: String,
    /// 所属园区，必填非 0——变压器必须属于某个园区。
    pub park_id: u64,
    /// 所在厂房；`0` 表示园区公共区域（配电房、室外等），沿用园区外键的 0 哨兵约定。
    pub factory_id: u64,
    /// 设备名称，必填——台账里的设备必须叫得出名字（如「1 号变压器」）。
    pub transformer_name: String,
    /// 位置描述，必填（如「3 号厂房西侧配电房」）——回答「哪台在哪」。
    pub location: String,
    /// 规格型号（如 `SCB13-1250kVA`）。
    pub specifications: String,
    /// 容量，单位千瓦（kW），乘一百的整数存储（全库定点数约定）。
    pub capacity_centi_kw: u64,
    /// 投运日期 `YYYY-MM-DD`，可选。
    pub commissioned_on: Option<String>,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
