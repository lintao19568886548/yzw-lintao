//! 账期结转清单：系统算、经理确认。
//!
//! 承载 `docs/租金收缴与对账确认流程.md` §2.2–§2.5：账期截止时把所有「未确认
//! 收齐」的账单算进一个批次，**不改动任何金额**；经理逐张核对、可剔除个别，
//! 确认之后差额才真正结转。
//!
//! 为什么不全自动写库：经理如果从没点过确认，全自动结转会把整个园区的账变成
//! 假欠款，而且混在真欠款里很难发现。做成待确认之后，最坏结果是「结转没发生」
//! ——清单还挂着，随时能点。**错要错在「没做事」那一侧。**

use spacetimedb::Timestamp;

/// 批次状态：等经理核对。
pub const BATCH_STATUS_PENDING: &str = "pending";
/// 批次状态：经理已确认，差额已结转。
pub const BATCH_STATUS_CONFIRMED: &str = "confirmed";

/// 处置：结转到下一账期（默认）。
pub const DISPOSITION_CARRY: &str = "carry";
/// 处置：本次不结转，下期还会再出现。
///
/// 语义是「我再查查」，**不等于已收齐**——所以不补任何确认快照。
pub const DISPOSITION_SKIP: &str = "skip";
/// 处置：剔除并确认收齐，「这张其实收了，财务忘了录」。
///
/// 与 [`DISPOSITION_SKIP`] 分成两个动作是有意的：经理只是想挂起时，系统不该
/// 替他记一条并不成立的「已收齐」快照——那快照将来是要拿去对账的。
pub const DISPOSITION_COLLECTED: &str = "collected";

/// 一个园区一次账期结算产出的批次。
///
/// 同一园区同时只允许一个 `pending` 批次，否则同一张账单会被算进两处，
/// 确认时重复结转。
#[spacetimedb::table(
    accessor = carryover_batch,
    index(accessor = carryover_batch_by_customer, btree(columns = [customer_id])),
    index(accessor = carryover_batch_by_park, btree(columns = [park_id]))
)]
pub struct CarryoverBatch {
    #[primary_key]
    #[auto_inc]
    pub batch_id: u64,
    pub customer_id: String,
    /// 所属园区，必填非 0。结转按园区分批：一次点击影响一个园区下月的账。
    pub park_id: u64,
    /// 账期标签，如「2026-08」。**只是标签不是判据**——账单表里没有账期字段，
    /// 批次实际覆盖的是「结算那一刻所有未确认收齐的账单」。
    pub period_label: String,
    /// `pending` 或 `confirmed`，见本文件顶部常量。
    pub status: String,
    pub created_at: Timestamp,
    pub confirmed_by: Option<u64>,
    /// 确认人姓名快照：人事变动后记录仍然可读。
    pub confirmed_by_name: Option<String>,
    pub confirmed_at: Option<Timestamp>,
}

/// 批次里的一张账单。
///
/// 金额是**结算那一刻的快照**：确认发生在核对之后，中间财务可能改过实收，
/// 快照保证「经理当初看到的是哪个数」说得清（与确认记录同一原则）。
///
/// 刻意不带 `park_id`：园区范围经由所属批次判定，也因此不进园区注销守卫清单。
#[spacetimedb::table(
    accessor = carryover_item,
    index(accessor = carryover_item_by_batch, btree(columns = [batch_id])),
    index(accessor = carryover_item_by_bill, btree(columns = [bill_id]))
)]
pub struct CarryoverItem {
    #[primary_key]
    #[auto_inc]
    pub item_id: u64,
    pub customer_id: String,
    pub batch_id: u64,
    pub bill_id: u64,
    /// 结算时刻的应收（分）。
    pub receivable_cents: i64,
    /// 结算时刻的实收（分）。
    pub received_cents: i64,
    /// `carry` / `skip` / `collected`，见本文件顶部常量。默认 `carry`。
    pub disposition: String,
    /// 结转额被并入了哪张下期账单；`None` 表示尚未并入。
    ///
    /// 有了它，「这笔结转有没有被真的收进下个月」才有答案；没有它，确认完
    /// 就断线了，钱去哪儿没人说得清。
    pub consumed_by_bill_id: Option<u64>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
    // 以下为后续追加列。**必须留在结构体末尾**：SpacetimeDB 2.6.1 按列序比对
    // schema，插在中间会被判定为「Reordering table requires a manual migration」
    // 而拒绝发布。写在 created_at 之前看着更顺眼，代价是发布不了。
    /// 来源账单的租户，结算时快照。
    ///
    /// **不靠查账单表反推**：账单页的账单走分页 Procedure、不在订阅里，前端
    /// 拿不到 `amount_bill` 全量，靠 `bill_id` 反查会永远查不到。结转项本来
    /// 就已经存了应收/实收快照，租户一并存下才自足。
    #[default(0u64)]
    pub tenant_id: u64,
    /// 来源账单的项目名，结算时快照。让经理认得出这是哪一笔。
    #[default(None::<String>)]
    pub source_project_name: Option<String>,
}
