//! 消防设施资产台账表定义。
//!
//! 一行是一个真实设施（灭火器、消防栓、消防出口、应急照明等各自成行）。
//! 与旧 `firefighting` 表（一行"检查点"混装三类设施状态，已弃置）不同，
//! 设施本体在这里长期存在，巡检事件在 `firefighting_inspection` 只增不删地
//! 累积。完整设计见 `docs/消防设施台账与扫码巡检.md`。
//!
//! 位置规则：消防设施位于厂房楼层或宿舍楼层中，两个楼层外键**恰好一个非 0**
//! （沿用 0 哨兵约定，无"园区公共区域"口子——园区没有室外消防设施）；
//! `park_id` 由服务端沿楼层反查推导写入，不接受客户端直传。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = firefighting_asset,
    index(accessor = firefighting_asset_by_customer, btree(columns = [customer_id])),
    index(accessor = firefighting_asset_by_park, btree(columns = [park_id])),
    index(accessor = firefighting_asset_by_factory_floor, btree(columns = [factory_floor_id])),
    index(accessor = firefighting_asset_by_dormitory_floor, btree(columns = [dormitory_floor_id]))
)]
pub struct FirefightingAsset {
    #[primary_key]
    #[auto_inc]
    pub asset_id: u64,
    pub customer_id: String,
    /// 沿楼层反查推导写入（厂房楼层 → 厂房 → 园区，或宿舍楼层 → 宿舍楼 → 园区）。
    pub park_id: u64,
    /// 所在厂房楼层；`0` = 不在厂房。
    pub factory_floor_id: u64,
    /// 所在宿舍楼层；`0` = 不在宿舍。与上一字段恰好一个非 0。
    pub dormitory_floor_id: u64,
    /// 设施类型：灭火器 / 消防栓 / 消防出口 / 应急照明 / 其他。
    pub facility_type: String,
    /// 编号或名称，必填（如「3F 东侧灭火器 2 号」）。
    pub facility_name: String,
    /// 楼层内位置描述，必填（如「东侧楼梯口」）。
    pub location: String,
    /// 型号规格（如「MFZ/ABC4 干粉」）。
    pub specifications: Option<String>,
    /// 有效期至 `YYYY-MM-DD`；设施类型为灭火器时必填，到期提醒依赖它。
    pub expiry_on: Option<String>,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
