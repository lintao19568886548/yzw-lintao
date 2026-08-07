//! 水费账单明细表。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = water_bill,
    index(accessor = water_bill_by_customer, btree(columns = [customer_id])),
    index(accessor = water_bill_by_bill, btree(columns = [bill_id])),
    index(accessor = water_bill_by_meter, btree(columns = [meter_id]))
)]
pub struct WaterBill {
    #[primary_key]
    #[auto_inc]
    pub water_id: u64,
    pub customer_id: String,
    pub bill_id: u64,
    /// 这一行抄的是哪块表。`0` 表示没有关联到台账，含义同 `EleBill.meter_id`。
    ///
    /// 水表没有分时，所以这里没有对应的 `tou_tier`。
    pub meter_id: u64,
    pub meter_name: String,
    pub previous_reading_centi: i64,
    pub current_reading_centi: i64,
    pub monthly_usage_centi: i64,
    pub multiplier_centi: i64,
    pub total_usage_centi: i64,
    pub unit_price_scaled: i64,
    pub amount_cents: i64,
    pub remark: Option<String>,
    pub receipt_time: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
