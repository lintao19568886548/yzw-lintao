//! 租赁客户及合同摘要表。

use spacetimedb::Timestamp;

/// 独立租户主档。合同、账单和工资仍保留自己的历史快照，主档只描述业务主体。
#[spacetimedb::table(
    accessor = tenant_profile,
    index(accessor = tenant_profile_by_customer, btree(columns = [customer_id])),
    index(accessor = tenant_profile_by_phone, btree(columns = [phone_number])),
    index(accessor = tenant_profile_by_status, btree(columns = [status]))
)]
pub struct TenantProfile {
    #[primary_key]
    #[auto_inc]
    pub tenant_profile_id: u64,
    pub customer_id: String,
    pub tenant_name: String,
    /// `enterprise` 或 `individual`。
    pub tenant_type: String,
    pub unified_social_credit_code: Option<String>,
    pub legal_representative: Option<String>,
    pub contact_name: String,
    pub phone_number: String,
    pub email: Option<String>,
    pub address: Option<String>,
    pub source: Option<String>,
    /// 1 为合作中，0 为已停用。
    pub status: i8,
    /// `normal`、`watch` 或 `high`。
    pub risk_level: String,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

/// 对应 MySQL 业务库中的 `rental_tenant` 表。
#[spacetimedb::table(
    accessor = rental_tenant,
    index(accessor = rental_tenant_by_customer, btree(columns = [customer_id])),
    index(accessor = rental_tenant_by_park, btree(columns = [park_id])),
    index(accessor = rental_tenant_by_transaction_type, btree(columns = [transaction_type]))
)]
pub struct RentalTenant {
    #[primary_key]
    #[auto_inc]
    pub rental_tenant_id: u64,
    pub customer_id: String,
    pub tenant_name: String,
    pub phone_number: String,
    pub transaction_type: bool,
    pub status: Option<String>,
    pub contract_start: Option<Timestamp>,
    pub contract_end: Option<Timestamp>,
    pub rental_amount_cents: Option<i64>,
    pub increase_date: Option<Timestamp>,
    /// 百分比乘以一百，例如 `5.25%` 保存为 `525`。
    pub increase_rate_basis_points: Option<i64>,
    pub increase_data: Option<String>,
    pub penalty_rate_basis_points: Option<i64>,
    /// 基本电费的计费容量，千瓦乘以一百。
    ///
    /// 大工业用电按变压器容量收一笔与用量无关的固定费用，容量和单价都是签合同
    /// 时谈定的。这两项为空表示这份合同不收基本电费。
    pub basic_ele_capacity_centi_kw: Option<i64>,
    /// 基本电费单价（元/千瓦），乘以一亿，与其他单价同精度。
    pub basic_ele_price_scaled: Option<i64>,
    pub area_centi_square_metres: Option<i64>,
    pub remark: Option<String>,
    /// 使用 `0` 表示未分配园区。
    pub park_id: u64,
    pub send_message_at: Option<Timestamp>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
