//! 合同约定的周期性费用：电损费、服务费、垃圾费。
//!
//! 这几项本来就是合同条款——签的时候谈定比例或金额，整个租期不变。但在这张表
//! 出现之前，合同里没有任何地方能写它们，`AmountBill` 上倒是有
//! `service_fee_cents`、`garbage_fee_cents`、`service_rate_basis_points`、
//! `extra_ele_rate_basis_points` 这些字段，于是每个月开账单都要重新手填一遍。
//! 迁移过来的生产数据里那几个 `*_rate_basis_points` 至今没有任何界面能编辑，
//! 就是因为「比例该存在哪」这个问题一直没有答案。
//!
//! # 为什么是子表而不是主表上开几个字段
//!
//! 三种费用 ×（固定金额 / 比例 + 基数）= 主表要开九个字段，而且加一种费用就要
//! 再开三个、再改一遍迁移。子表一行一项，加种类只要放宽 [`FEE_KINDS`] 的校验。
//!
//! # 为什么基数只是一个字符串枚举
//!
//! 比例项的基数是「按什么算」——厂房电费合计、厂房度数合计等等。这些量全部能从
//! 当期水电明细**算出来**，不需要在合同里存任何金额：合同存「5% × 厂房电费合计」，
//! 开账单时按那个月的实际明细算。存金额就等于把某一个月的数字冻进合同里。

use spacetimedb::Timestamp;

/// 费用种类。账单侧有对应的金额字段，一一对应，不接受种类之外的自由文本——
/// 自由文本填进来没有任何地方会算它，等于白填。
pub const FEE_KIND_LOSS: &str = "loss";
pub const FEE_KIND_SERVICE: &str = "service";
pub const FEE_KIND_GARBAGE: &str = "garbage";
pub const FEE_KINDS: [&str; 3] = [FEE_KIND_LOSS, FEE_KIND_SERVICE, FEE_KIND_GARBAGE];

/// 计费方式。
pub const CHARGE_MODE_FIXED: &str = "fixed";
pub const CHARGE_MODE_RATE: &str = "rate";
pub const CHARGE_MODES: [&str; 2] = [CHARGE_MODE_FIXED, CHARGE_MODE_RATE];

/// 比例项的计费基数。
///
/// 「厂房」「宿舍」不需要在账单行上另存字段：电表挂在哪一层，台账
/// （`UtilityMeter.factory_floor_id` / `dormitory_floor_id`）已经写着了，场景
/// 是**算出来的**。老系统要求每行手选使用场景，选错就算错，而且同一块表这个月
/// 选厂房下个月选宿舍也没人拦。
pub const RATE_BASE_FACTORY_USAGE: &str = "factory_usage";
pub const RATE_BASE_FACTORY_FEE: &str = "factory_fee";
pub const RATE_BASE_FACTORY_FEE_WITH_BASIC: &str = "factory_fee_with_basic";
pub const RATE_BASE_FACTORY_DORM_FEE: &str = "factory_dorm_fee";
pub const RATE_BASE_TOTAL_ELE_FEE: &str = "total_ele_fee";
pub const RATE_BASES: [&str; 5] = [
    RATE_BASE_FACTORY_USAGE,
    RATE_BASE_FACTORY_FEE,
    RATE_BASE_FACTORY_FEE_WITH_BASIC,
    RATE_BASE_FACTORY_DORM_FEE,
    RATE_BASE_TOTAL_ELE_FEE,
];

#[spacetimedb::table(
    accessor = rental_tenant_fee,
    index(accessor = rental_tenant_fee_by_customer, btree(columns = [customer_id])),
    index(accessor = rental_tenant_fee_by_tenant, btree(columns = [rental_tenant_id])),
    index(accessor = rental_tenant_fee_by_pair, btree(columns = [rental_tenant_id, fee_kind]))
)]
pub struct RentalTenantFee {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub rental_tenant_id: u64,
    /// [`FEE_KINDS`] 之一。同一份合同同一种费用只能有一条。
    pub fee_kind: String,
    /// [`CHARGE_MODES`] 之一。
    pub charge_mode: String,
    /// 固定金额（分）。`charge_mode = fixed` 时必填，否则为空。
    pub amount_cents: Option<i64>,
    /// 比例，基点。`5%` 存 `500`。`charge_mode = rate` 时必填，否则为空。
    pub rate_basis_points: Option<i64>,
    /// [`RATE_BASES`] 之一。`charge_mode = rate` 时必填，否则为空。
    pub rate_base: Option<String>,
    pub remark: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
