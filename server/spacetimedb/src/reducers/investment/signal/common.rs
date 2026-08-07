//! 企业信号共用查找与字段校验。

use spacetimedb::ReducerContext;

use crate::{reducers::shared::access::current_customer_id, tables::*};

pub(super) const VALID_SIGNAL_STATUSES: [&str; 4] = ["CONVERTED", "IGNORED", "NEW", "REVIEWED"];

pub(super) fn require_signal_event(
    ctx: &ReducerContext,
    event_id: u64,
) -> Result<SignalEvent, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .signal_event()
        .event_id()
        .find(event_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("企业信号事件不存在".into())
}

pub(super) fn validate_signal_status(status: &str) -> Result<(), String> {
    VALID_SIGNAL_STATUSES
        .contains(&status)
        .then_some(())
        .ok_or("企业信号状态无效".into())
}
