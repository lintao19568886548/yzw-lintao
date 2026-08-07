//! 应收账单主表。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = amount_bill,
    index(accessor = amount_bill_by_customer, btree(columns = [customer_id])),
    index(accessor = amount_bill_by_tenant, btree(columns = [tenant_id])),
    index(accessor = amount_bill_by_park, btree(columns = [park_id])),
    index(accessor = amount_bill_by_finance, btree(columns = [finance_id]))
)]
pub struct AmountBill {
    #[primary_key]
    #[auto_inc]
    pub bill_id: u64,
    pub customer_id: String,
    pub project_name: String,
    pub tenant_name: Option<String>,
    pub public_bank_account: Option<String>,
    pub private_bank_account: Option<String>,
    pub ele_fee_cents: i64,
    pub water_fee_cents: i64,
    pub receive_fee_cents: i64,
    pub factory_rent_cents: i64,
    pub management_fee_cents: i64,
    pub invoice_tax_cents: i64,
    pub total_fee_cents: i64,
    pub service_fee_cents: i64,
    pub garbage_fee_cents: i64,
    /// 电损费（分）。
    ///
    /// 原来只有 `extra_ele_rate_basis_points` 这个比例字段，却没有地方放算出来的
    /// 金额，于是电损要么被并进电费、要么根本没收。
    pub extra_ele_fee_cents: i64,
    /// 基本电费（分）。
    pub basic_ele_fee_cents: i64,
    /// 开账单时的计费容量快照，千瓦乘以一百。
    ///
    /// 合同改了容量或单价，已开出去的账单不能跟着变——账单是对外的凭据，必须
    /// 自带算式。工资和账单明细已经是这个口径，这里保持一致。
    pub basic_ele_capacity_centi_kw: Option<i64>,
    /// 开账单时的基本电费单价快照（元/千瓦，乘以一亿）。
    pub basic_ele_price_scaled: Option<i64>,
    pub receipt_amount_cents: i64,
    pub penalty_fee_cents: Option<i64>,
    pub service_rate_basis_points: Option<i64>,
    pub garbage_rate_basis_points: Option<i64>,
    pub penalty_rate_basis_points: Option<i64>,
    pub extra_ele_rate_basis_points: Option<i64>,
    pub penalty_item: Option<String>,
    pub extra_ele_item: Option<String>,
    pub ele_item: Option<String>,
    pub water_item: Option<String>,
    pub extra_project_item: Option<String>,
    pub tax_rate_json: Option<String>,
    pub remark: Option<String>,
    pub receipt_time: Option<Timestamp>,
    pub finance_id: u64,
    pub tenant_id: u64,
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
    /// 上期结转过来的欠款（分）。
    ///
    /// **单独成项，不并进租金**：账单是给租户看的凭据，本期新发生多少、上期欠了
    /// 多少必须分得开。并进租金之后，租户只看到一个变大的数字，催缴时说不清。
    ///
    /// 这一列的值必须等于所消费结转项的差额之和，由
    /// `check_carryover_consumption` 在写入前核对——账单上写着结转 ¥2000、
    /// 实际却勾了 ¥3000 的结转项，那 ¥1000 就凭空消失了。
    // 字面量必须显式标 i64：写 `0` 会被编码成 4 字节的 i32，发布时报
    // 「data too short for i64: Expected 8, given 4」，且要到 spacetime generate
    // 才暴露，cargo check 一路是绿的。
    #[default(0i64)]
    pub carryover_fee_cents: i64,
    /// 结转项的名目，如「7月欠款结转」。与 `penalty_item` 等同属名目列。
    #[default(None::<String>)]
    pub carryover_item: Option<String>,
}
