//! VIP 会员事务共用的记录和状态校验。

use spacetimedb::ReducerContext;

use crate::tables::*;

pub(super) fn require_vip_payment(
    ctx: &ReducerContext,
    out_trade_no: &String,
) -> Result<VipMembershipPayment, String> {
    ctx.db
        .vip_membership_payment()
        .out_trade_no()
        .find(out_trade_no)
        .ok_or("VIP 支付订单不存在".into())
}

pub(super) fn require_vip_refund(
    ctx: &ReducerContext,
    out_refund_no: &String,
) -> Result<VipMembershipRefund, String> {
    ctx.db
        .vip_membership_refund()
        .out_refund_no()
        .find(out_refund_no)
        .ok_or("VIP 退款记录不存在".into())
}

pub(super) fn canonicalize_optional_json(
    value: Option<String>,
    message: &'static str,
) -> Result<Option<String>, String> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let parsed: serde_json::Value = serde_json::from_str(value.trim()).map_err(|_| message)?;
    serde_json::to_string(&parsed)
        .map(Some)
        .map_err(|_| "JSON 数据无法序列化".into())
}
