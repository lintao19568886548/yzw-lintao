//! VIP 退款申请和权益撤销事务。

use spacetimedb::{ReducerContext, Table};

use super::common::{canonicalize_optional_json, require_vip_payment, require_vip_refund};
use crate::{
    reducers::{
        access::{AdminContext, current_center_user_id},
        validation::{limited_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

#[spacetimedb::reducer]
pub fn request_vip_membership_refund(
    ctx: &ReducerContext,
    out_refund_no: String,
    out_trade_no: String,
    refund_amount: u64,
    reason: Option<String>,
    channel: Option<String>,
) -> Result<(), String> {
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let payment = require_vip_payment(ctx, &out_trade_no)?;
    if payment.center_user_id != center_user_id {
        return Err("只能申请本人支付订单的退款".into());
    }
    if payment.trade_state != "SUCCESS" {
        return Err("仅支付成功的订单可以退款".into());
    }
    let entitlement = ctx
        .db
        .vip_membership_entitlement()
        .out_trade_no()
        .find(&out_trade_no)
        .filter(|row| row.status == "active")
        .ok_or("支付订单没有可退款的有效权益")?;
    if refund_amount == 0 || refund_amount > payment.amount_total {
        return Err("退款金额必须大于 0 且不能超过支付金额".into());
    }
    let out_refund_no = required_text(out_refund_no, "商户退款单号不能为空")?;
    validate_max_length(&out_refund_no, 64, "商户退款单号不能超过 64 个字符")?;
    if ctx
        .db
        .vip_membership_refund()
        .out_refund_no()
        .find(&out_refund_no)
        .is_some()
    {
        return Err("商户退款单号已存在".into());
    }
    ctx.db.vip_membership_refund().insert(VipMembershipRefund {
        id: 0,
        center_scope: 0,
        out_refund_no,
        refund_id: None,
        out_trade_no,
        transaction_id: payment.transaction_id,
        center_user_id,
        customer_id: entitlement.customer_id,
        amount_total: payment.amount_total,
        refund_amount,
        status: "PENDING".into(),
        reason: limited_optional_text(reason, 255, "退款原因不能超过 255 个字符")?,
        channel: limited_optional_text(channel, 32, "退款渠道不能超过 32 个字符")?
            .unwrap_or_else(|| "system".into()),
        notify_event_id: None,
        provider_raw: None,
        requested_at: Some(ctx.timestamp),
        success_at: None,
        last_checked_at: None,
        next_check_at: None,
        created_at: Some(ctx.timestamp),
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn complete_vip_membership_refund(
    ctx: &ReducerContext,
    out_refund_no: String,
    refund_id: Option<String>,
    notify_event_id: Option<String>,
    provider_raw_json: Option<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut refund = require_vip_refund(ctx, &out_refund_no)?;
    if refund.status == "SUCCESS" {
        return Ok(());
    }
    if !matches!(refund.status.as_str(), "PENDING" | "PROCESSING") {
        return Err("当前退款状态不能标记成功".into());
    }
    let notify_event_id =
        limited_optional_text(notify_event_id, 64, "退款通知事件标识不能超过 64 个字符")?;
    if let Some(event_id) = notify_event_id.as_deref()
        && ctx
            .db
            .vip_membership_refund()
            .iter()
            .any(|row| row.id != refund.id && row.notify_event_id.as_deref() == Some(event_id))
    {
        return Err("退款通知事件已经处理".into());
    }
    let mut entitlement = ctx
        .db
        .vip_membership_entitlement()
        .out_trade_no()
        .find(&refund.out_trade_no)
        .ok_or("退款订单关联的 VIP 权益不存在")?;
    entitlement.status = "refunded".into();
    entitlement.refunded_at = Some(ctx.timestamp);
    entitlement.refunded_out_refund_no = Some(out_refund_no.clone());
    entitlement.updated_at = Some(ctx.timestamp);
    ctx.db.vip_membership_entitlement().id().update(entitlement);

    let remaining_end = ctx
        .db
        .vip_membership_entitlement()
        .vip_entitlement_by_customer()
        .filter(refund.customer_id.as_str())
        .filter(|row| row.status == "active" && row.end_at > ctx.timestamp)
        .map(|row| row.end_at)
        .max();
    if let Some(mut membership) = ctx
        .db
        .vip_membership()
        .customer_id()
        .find(&refund.customer_id)
    {
        if let Some(end_at) = remaining_end {
            membership.status = "active".into();
            membership.expire_at = end_at;
        } else {
            membership.status = "refunded".into();
            membership.expire_at = ctx.timestamp;
        }
        membership.updated_at = Some(ctx.timestamp);
        ctx.db.vip_membership().id().update(membership);
    }
    let mut payment = require_vip_payment(ctx, &refund.out_trade_no)?;
    payment.trade_state = "REFUND".into();
    payment.refund_checked_at = Some(ctx.timestamp);
    payment.updated_at = Some(ctx.timestamp);
    ctx.db.vip_membership_payment().id().update(payment);

    refund.status = "SUCCESS".into();
    refund.refund_id = limited_optional_text(refund_id, 64, "退款平台标识不能超过 64 个字符")?;
    refund.notify_event_id = notify_event_id;
    refund.provider_raw =
        canonicalize_optional_json(provider_raw_json, "退款渠道原始数据必须是有效 JSON")?;
    refund.success_at = Some(ctx.timestamp);
    refund.last_checked_at = Some(ctx.timestamp);
    refund.next_check_at = None;
    refund.updated_at = Some(ctx.timestamp);
    ctx.db.vip_membership_refund().id().update(refund);
    Ok(())
}

#[spacetimedb::reducer]
pub fn fail_vip_membership_refund(
    ctx: &ReducerContext,
    out_refund_no: String,
    provider_raw_json: Option<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut refund = require_vip_refund(ctx, &out_refund_no)?;
    if refund.status == "SUCCESS" {
        return Err("成功退款不能回退为失败".into());
    }
    refund.status = "FAILED".into();
    refund.provider_raw =
        canonicalize_optional_json(provider_raw_json, "退款渠道原始数据必须是有效 JSON")?;
    refund.last_checked_at = Some(ctx.timestamp);
    refund.next_check_at = None;
    refund.updated_at = Some(ctx.timestamp);
    ctx.db.vip_membership_refund().id().update(refund);
    Ok(())
}
