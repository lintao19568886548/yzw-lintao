//! 水电表台账。
//!
//! 建这张表之前，「这块表是哪块表」只存在于 `EleBill.meter_name` /
//! `WaterBill.meter_name` 的自由文本里。生产数据说明了这条路走不通：
//! 1200 多条明细里有 262 条表计名是空的，另有 91 条把分时电价的时段
//! （尖/峰/平/谷）填进了表计名字段——因为分时电价在系统里根本没有
//! 别的地方可放。剩下的值也全是「公共用电」「车间用电」这类用途分类，
//! 不是表的身份。
//!
//! 表跟楼层一样是**资产**：它物理安装在某个位置，不随租户更替而消失。
//! 「谁在用这块表、按什么价结算」是另一件事，在 `RentalTenantMeter`。
//! 两者拆开之后，空置楼层依然看得到自己有哪几块表，新签合同可以从
//! 列表里勾选而不是重新敲一遍表号。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = utility_meter,
    index(accessor = utility_meter_by_customer, btree(columns = [customer_id])),
    index(accessor = utility_meter_by_park, btree(columns = [park_id])),
    index(accessor = utility_meter_by_factory_floor, btree(columns = [factory_floor_id])),
    index(accessor = utility_meter_by_dormitory_floor, btree(columns = [dormitory_floor_id]))
)]
pub struct UtilityMeter {
    #[primary_key]
    #[auto_inc]
    pub meter_id: u64,
    pub customer_id: String,
    /// 表所属园区。安装位置可以为空（公共区域的表），园区不能为空。
    pub park_id: u64,
    /// 表号／资产编号，园区内唯一。
    pub meter_code: String,
    /// true = 电表，false = 水表。
    pub is_electric: bool,
    /// 装在哪个厂房楼层。0 表示不在厂房。
    ///
    /// 用 0 哨兵而不是 `Option<u64>`，是为了能直接走 btree 索引按楼层反查
    /// ——「这一层有哪几块表」是资产详情页每次渲染都要问的问题。
    pub factory_floor_id: u64,
    /// 装在哪个宿舍楼层。0 表示不在宿舍。
    ///
    /// 与 `factory_floor_id` 至多有一个非零；两个都为 0 表示这是园区公共
    /// 区域的表（生产数据里 299 条「公共用电」就是这一类）。
    pub dormitory_floor_id: u64,
    /// 互感器倍率乘以一百。抄表读数乘以它才是实际用量。
    ///
    /// 存在表上而不是每张账单重填：倍率是表的物理属性，装上去就不变。
    pub multiplier_centi: i64,
    /// 这块表是否支持分时计量（尖／峰／平／谷）。
    ///
    /// 只有分时表才谈得上四段电价；水表和普通电表按单一单价结算。
    pub is_time_of_use: bool,
    /// 智能水电表平台里的设备号，用于把外部回读的读数落到这块表上。
    pub external_device_id: Option<String>,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
