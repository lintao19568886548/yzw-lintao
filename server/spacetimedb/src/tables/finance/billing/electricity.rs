//! 电费账单明细表。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = ele_bill,
    index(accessor = ele_bill_by_customer, btree(columns = [customer_id])),
    index(accessor = ele_bill_by_bill, btree(columns = [bill_id])),
    index(accessor = ele_bill_by_meter, btree(columns = [meter_id]))
)]
pub struct EleBill {
    #[primary_key]
    #[auto_inc]
    pub ele_id: u64,
    pub customer_id: String,
    pub bill_id: u64,
    /// 这一行抄的是哪块表。`0` 表示没有关联到台账——历史数据和手工补录的
    /// 行没有表可指，用 0 而不是 `Option<u64>` 是为了让这一列可以建索引。
    ///
    /// 有了它，读数才能可靠地对上表：`meter_name` 是自由文本，迁移过来的
    /// 生产数据里 262 行是空的、91 行把「尖峰平谷」塞进了表名。
    pub meter_id: u64,
    /// 分时时段：`tip` / `peak` / `flat` / `valley`，非分时行为 `None`。
    ///
    /// 存英文标识而不是「尖峰平谷」：展示文案在客户端映射，改叫法不用动数据。
    /// 一块分时表一个账期出四行，靠这一列区分，而不是靠表名后缀。
    pub tou_tier: Option<String>,
    pub meter_name: String,
    pub previous_reading_centi: i64,
    pub current_reading_centi: i64,
    pub monthly_usage_centi: i64,
    pub multiplier_centi: i64,
    pub total_usage_centi: i64,
    /// 单价乘以一亿，保留 MySQL `decimal(10,8)` 精度。
    pub unit_price_scaled: i64,
    pub amount_cents: i64,
    pub remark: Option<String>,
    pub receipt_time: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
