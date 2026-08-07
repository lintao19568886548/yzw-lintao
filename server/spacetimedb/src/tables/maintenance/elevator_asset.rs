//! 电梯资产台账表定义。
//!
//! 一行是一台真实电梯。与旧 `elevator` 表（资产与巡检混装一行，已弃置）不同，
//! 设备本体在这里长期存在，巡检事件在 `elevator_inspection` 表里只增不删地
//! 累积。完整设计见 `docs/电梯台账与扫码巡检.md`。
//!
//! 与变压器的关键差异：**电梯必须属于某个厂房**（`factory_id` 必填非 0，
//! 没有「园区公共区域」哨兵），`park_id` 由服务端从厂房推导写入，不接受
//! 客户端直传——从结构上排除电梯与园区归属矛盾。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = elevator_asset,
    index(accessor = elevator_asset_by_customer, btree(columns = [customer_id])),
    index(accessor = elevator_asset_by_park, btree(columns = [park_id])),
    index(accessor = elevator_asset_by_factory, btree(columns = [factory_id]))
)]
pub struct ElevatorAsset {
    #[primary_key]
    #[auto_inc]
    pub asset_id: u64,
    pub customer_id: String,
    /// 从厂房推导写入（`factory.park_id`）。
    pub park_id: u64,
    /// 所在厂房，必填非 0——一个厂房可以有若干台电梯。
    pub factory_id: u64,
    /// 设备名称，必填（如「A 座 1 号客梯」）。
    pub elevator_name: String,
    /// 厂房内位置描述，必填（如「东侧大厅」）。
    pub location: String,
    /// 轿厢尺寸（长 × 宽 × 高）。
    pub size: Option<String>,
    /// 额定承重，单位千克（kg），乘一百的整数存储（全库定点数约定）。
    pub load_capacity_centi_kg: u64,
    /// 生产日期 `YYYY-MM-DD`，可选。
    pub production_date: Option<String>,
    /// 维保单位、联系电话等先记在备注里。
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
