//! 外部企业线索共用查找与枚举校验。

use spacetimedb::ReducerContext;

use crate::{reducers::shared::access::current_customer_id, tables::*};

const VALID_LEAD_STATUSES: [&str; 7] = [
    "ASSIGNED",
    "FOLLOWING",
    "INVALID",
    "NEW",
    "PENDING_REVIEW",
    "VISITED",
    "WON",
];

pub(crate) fn require_company_lead(
    ctx: &ReducerContext,
    lead_id: u64,
) -> Result<CompanyLead, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .company_lead()
        .lead_id()
        .find(lead_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("外部企业线索不存在".into())
}

pub(super) fn validate_lead_status(status: &str) -> Result<(), String> {
    VALID_LEAD_STATUSES
        .contains(&status)
        .then_some(())
        .ok_or("外部企业线索状态无效".into())
}
