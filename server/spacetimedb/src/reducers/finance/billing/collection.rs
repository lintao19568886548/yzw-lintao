//! 账单催缴发送结果记录。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_amount_bill},
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

/// 短信网关调用完成后的审计输入。
#[derive(SpacetimeType)]
pub struct CollectionSmsResultInput {
    pub collection_type: String,
    pub template_id: Option<String>,
    pub phone_number: Option<String>,
    pub tenant_name: Option<String>,
    pub project_name: Option<String>,
    pub remaining_amount_cents: Option<i64>,
    pub success: bool,
    pub error: Option<String>,
    pub provider_result_json: Option<String>,
}

#[spacetimedb::reducer]
pub fn record_collection_sms_result(
    ctx: &ReducerContext,
    bill_id: u64,
    input: CollectionSmsResultInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_amount_bill(ctx, bill_id)?;
    let collection_type = required_text(input.collection_type, "催缴类型不能为空")?;
    if input
        .remaining_amount_cents
        .is_some_and(|remaining| remaining < 0)
    {
        return Err("催缴剩余金额不能为负数".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .amount_bill_collection_sms_log()
        .insert(AmountBillCollectionSmsLog {
            log_id: 0,
            customer_id,
            bill_id,
            collection_type,
            template_id: normalize_optional_text(input.template_id),
            phone_number: normalize_optional_text(input.phone_number),
            tenant_name: normalize_optional_text(input.tenant_name),
            project_name: normalize_optional_text(input.project_name),
            remaining_amount_cents: input.remaining_amount_cents,
            success: input.success,
            error: normalize_optional_text(input.error),
            provider_result_json: normalize_optional_text(input.provider_result_json),
            sent_at: ctx.timestamp,
            created_at: ctx.timestamp,
        });
    Ok(())
}
