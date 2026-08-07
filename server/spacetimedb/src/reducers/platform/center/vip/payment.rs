//! VIP 支付快照和权益生效事务。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::{canonicalize_optional_json, require_vip_payment};
use crate::{
    reducers::{
        access::{AdminContext, current_center_user_id, require_customer, require_organization},
        validation::{limited_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 创建待支付快照时保存的订单上下文。
#[derive(SpacetimeType)]
pub struct VipMembershipPaymentInput {
    pub out_trade_no: String,
    pub source_org_id: Option<u64>,
    pub source_customer_id: String,
    pub target_customer_id: Option<String>,
    pub target_city: Option<String>,
    pub target_company_short_name: Option<String>,
    pub username: Option<String>,
    pub amount_total: u64,
    pub raw_attach_json: Option<String>,
}

#[spacetimedb::reducer]
pub fn record_vip_membership_payment_pending(
    ctx: &ReducerContext,
    input: VipMembershipPaymentInput,
) -> Result<(), String> {
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let out_trade_no = required_text(input.out_trade_no, "商户订单号不能为空")?;
    validate_max_length(&out_trade_no, 64, "商户订单号不能超过 64 个字符")?;
    let source_customer_id = required_text(input.source_customer_id, "来源租户不能为空")?;
    require_customer(ctx, &source_customer_id)?;
    if let Some(source_org_id) = input.source_org_id {
        let organization = require_organization(ctx, source_org_id)?;
        if organization.source_customer_id != source_customer_id {
            return Err("来源组织不属于来源租户".into());
        }
    }
    if input.amount_total == 0 {
        return Err("VIP 支付金额必须大于 0".into());
    }
    if let Some(existing) = ctx
        .db
        .vip_membership_payment()
        .out_trade_no()
        .find(&out_trade_no)
    {
        if existing.center_user_id == center_user_id
            && existing.amount_total == input.amount_total
            && existing.source_customer_id == source_customer_id
        {
            return Ok(());
        }
        return Err("商户订单号已被其他支付快照使用".into());
    }
    ctx.db
        .vip_membership_payment()
        .insert(VipMembershipPayment {
            id: 0,
            center_scope: 0,
            out_trade_no,
            center_user_id,
            source_org_id: input.source_org_id,
            source_customer_id,
            target_customer_id: limited_optional_text(
                input.target_customer_id,
                50,
                "目标租户不能超过 50 个字符",
            )?,
            target_city: limited_optional_text(
                input.target_city,
                30,
                "目标城市不能超过 30 个字符",
            )?,
            target_company_short_name: limited_optional_text(
                input.target_company_short_name,
                50,
                "目标公司简称不能超过 50 个字符",
            )?,
            username: limited_optional_text(input.username, 50, "用户名不能超过 50 个字符")?,
            amount_total: input.amount_total,
            trade_state: "NOTPAY".into(),
            transaction_id: None,
            paid_at: None,
            applied_at: None,
            refund_checked_at: None,
            raw_attach: canonicalize_optional_json(
                input.raw_attach_json,
                "支付附加数据必须是有效 JSON",
            )?,
            created_at: Some(ctx.timestamp),
            updated_at: None,
        });
    Ok(())
}

/// 支付渠道确认成功后一次性生效会员权益。
#[spacetimedb::reducer]
pub fn apply_vip_membership_payment(
    ctx: &ReducerContext,
    out_trade_no: String,
    transaction_id: String,
    customer_id: String,
    duration_months: u32,
    end_at: Timestamp,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut payment = require_vip_payment(ctx, &out_trade_no)?;
    let transaction_id = required_text(transaction_id, "支付平台交易号不能为空")?;
    validate_max_length(&transaction_id, 64, "支付平台交易号不能超过 64 个字符")?;
    require_customer(ctx, &customer_id)?;
    if duration_months == 0 || duration_months > 120 {
        return Err("VIP 权益月份必须在 1 到 120 之间".into());
    }
    if let Some(existing) = ctx
        .db
        .vip_membership_entitlement()
        .out_trade_no()
        .find(&out_trade_no)
    {
        if existing.customer_id == customer_id
            && existing.transaction_id.as_deref() == Some(&transaction_id)
        {
            return Ok(());
        }
        return Err("支付订单已经生成其他 VIP 权益".into());
    }
    if !matches!(payment.trade_state.as_str(), "NOTPAY" | "SUCCESS") {
        return Err("当前支付状态不能生效 VIP 权益".into());
    }
    let start_at = ctx
        .db
        .vip_membership()
        .customer_id()
        .find(&customer_id)
        .filter(|row| row.status == "active" && row.expire_at > ctx.timestamp)
        .map(|row| row.expire_at)
        .unwrap_or(ctx.timestamp);
    if end_at <= start_at {
        return Err("VIP 权益结束时间必须晚于开始时间".into());
    }
    ctx.db
        .vip_membership_entitlement()
        .insert(VipMembershipEntitlement {
            id: 0,
            center_scope: 0,
            customer_id: customer_id.clone(),
            center_user_id: Some(payment.center_user_id),
            out_trade_no: out_trade_no.clone(),
            transaction_id: Some(transaction_id.clone()),
            amount_total: payment.amount_total,
            duration_months,
            start_at,
            end_at,
            status: "active".into(),
            refunded_at: None,
            refunded_out_refund_no: None,
            created_at: Some(ctx.timestamp),
            updated_at: None,
        });
    if let Some(mut membership) = ctx.db.vip_membership().customer_id().find(&customer_id) {
        membership.status = "active".into();
        membership.expire_at = end_at;
        membership.last_payer_center_user_id = Some(payment.center_user_id);
        membership.last_out_trade_no = Some(out_trade_no.clone());
        membership.last_transaction_id = Some(transaction_id.clone());
        membership.updated_at = Some(ctx.timestamp);
        ctx.db.vip_membership().id().update(membership);
    } else {
        ctx.db.vip_membership().insert(VipMembership {
            id: 0,
            center_scope: 0,
            customer_id: customer_id.clone(),
            status: "active".into(),
            expire_at: end_at,
            last_payer_center_user_id: Some(payment.center_user_id),
            last_out_trade_no: Some(out_trade_no.clone()),
            last_transaction_id: Some(transaction_id.clone()),
            created_at: Some(ctx.timestamp),
            updated_at: None,
        });
    }
    payment.target_customer_id = Some(customer_id);
    payment.trade_state = "SUCCESS".into();
    payment.transaction_id = Some(transaction_id);
    payment.paid_at = Some(ctx.timestamp);
    payment.applied_at = Some(ctx.timestamp);
    payment.updated_at = Some(ctx.timestamp);
    ctx.db.vip_membership_payment().id().update(payment);
    Ok(())
}
