//! 租赁合同与水电表的用表关系，以及合同约定的水电单价。
//!
//! 水电定价本来就是合同条款——一户一价，签的时候谈定，整个租期不变。
//! 但在这张表出现之前，合同表里没有任何水电单价字段，单价只能在每次
//! 开账单时重新手填一遍（账单工作表的 `unit_price` 每次从空字符串开始）。
//! 同一份合同连续开十二个月，单价就要被手敲十二遍，敲错一次就是一次
//! 收费纠纷。
//!
//! 分时电价占了四个字段而不是一个，是因为一块分时表一天里真的有四个价。
//! 原来这四段被塞进 `EleBill.meter_name` 当表计名用（生产数据里尖 22 条、
//! 峰 23 条、平 23 条、谷 23 条），正是因为没有别的地方能放。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = rental_tenant_meter,
    index(accessor = rental_tenant_meter_by_customer, btree(columns = [customer_id])),
    index(accessor = rental_tenant_meter_by_tenant, btree(columns = [rental_tenant_id])),
    index(accessor = rental_tenant_meter_by_meter, btree(columns = [meter_id])),
    index(accessor = rental_tenant_meter_by_pair, btree(columns = [rental_tenant_id, meter_id]))
)]
pub struct RentalTenantMeter {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub rental_tenant_id: u64,
    pub meter_id: u64,
    /// 单一单价乘以一亿，与 `EleBill.unit_price_scaled` 同精度。
    ///
    /// 水表和不分时的电表用它；分时表这一项为空，改用下面四段。
    pub unit_price_scaled: Option<i64>,
    /// 尖段电价（乘以一亿）。
    ///
    /// 四段电价要么全部填写、要么全部为空——只填一半的合同没有办法结算，
    /// 服务端在写入前就会拦住。
    pub price_tip_scaled: Option<i64>,
    /// 峰段电价（乘以一亿）。
    pub price_peak_scaled: Option<i64>,
    /// 平段电价（乘以一亿）。
    pub price_flat_scaled: Option<i64>,
    /// 谷段电价（乘以一亿）。
    pub price_valley_scaled: Option<i64>,
    pub remark: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
