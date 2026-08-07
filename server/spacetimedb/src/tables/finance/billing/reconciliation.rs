//! 账单收缴确认记录：只增不删。
//!
//! 一行是园区经理的一次确认动作——「确认已收齐」或「确认差额」。与巡检记录
//! 同构：不提供更新与删除 Reducer，确认错了补一条新的确认。
//!
//! **应收与实收都是确认时刻的快照。**只存差额的话，后续补缴改变了实收，就再也
//! 说不清经理当初确认的是哪个数（docs/租金收缴与对账确认流程.md 第四章）；存
//! 两个操作数而不是差值，与账单「自带算式」的口径一致（`AmountBill` 的容量与
//! 单价快照）。
//!
//! 刻意**不带 `park_id` 列**：园区范围经由所属账单判定（视图走 `my_amount_bills`
//! 过滤），也因此不进入园区注销守卫的子表清单——账单本身已经挡住园区删除，
//! 确认记录不可能比它的账单活得更久。

use spacetimedb::Timestamp;

/// 确认类型：已收齐。这是唯一能闭环的确认。
pub const CONFIRM_TYPE_COLLECTED: &str = "collected";
/// 确认类型：差额。只表示知悉，不闭环，确认后进入二次催交。
pub const CONFIRM_TYPE_SHORTFALL: &str = "shortfall";

#[spacetimedb::table(
    accessor = bill_collection_confirmation,
    index(accessor = confirmation_by_customer, btree(columns = [customer_id])),
    index(accessor = confirmation_by_bill, btree(columns = [bill_id]))
)]
pub struct BillCollectionConfirmation {
    #[primary_key]
    #[auto_inc]
    pub confirmation_id: u64,
    pub customer_id: String,
    pub bill_id: u64,
    /// `collected` 或 `shortfall`，见本文件顶部常量。
    pub confirm_type: String,
    /// 确认时刻的应收（分）。
    pub receivable_cents: i64,
    /// 确认时刻的实收（分）。
    pub received_cents: i64,
    /// 确认人，`system_user` 主键。
    pub confirmed_by: u64,
    /// 确认人姓名快照：人事变动后记录仍然可读。
    pub confirmed_by_name: String,
    pub remark: Option<String>,
    pub confirmed_at: Timestamp,
}
