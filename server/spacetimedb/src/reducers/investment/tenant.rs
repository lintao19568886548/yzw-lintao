//! 招商租户收支记录的创建、更新与删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::require_investment_tenant;
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 招商租户收支记录可修改字段。
#[derive(SpacetimeType)]
pub struct InvestmentTenantInput {
    pub bill_name: String,
    pub bill_category: String,
    pub amount_cents: i64,
    pub transaction_type: String,
    pub transaction_time: Timestamp,
}

#[spacetimedb::reducer]
pub fn create_investment_tenant(
    ctx: &ReducerContext,
    input: InvestmentTenantInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_row(ctx, 0, customer_id, input)?;
    ctx.db.investment_tenant().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_investment_tenant(
    ctx: &ReducerContext,
    tenant_id: u64,
    input: InvestmentTenantInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_investment_tenant(ctx, tenant_id)?;
    let mut row = validated_row(ctx, tenant_id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.investment_tenant().tenant_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_investment_tenant(ctx: &ReducerContext, tenant_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_investment_tenant(ctx, tenant_id)?;
    ctx.db.investment_tenant().tenant_id().delete(tenant_id);
    Ok(())
}

fn validated_row(
    ctx: &ReducerContext,
    tenant_id: u64,
    customer_id: String,
    input: InvestmentTenantInput,
) -> Result<InvestmentTenant, String> {
    let bill_name = required_text(input.bill_name, "账目名称不能为空")?;
    let bill_category = required_text(input.bill_category, "账目分类不能为空")?;
    let transaction_type = required_text(input.transaction_type, "收支类型不能为空")?;
    validate_max_length(&bill_name, 100, "账目名称不能超过100个字符")?;
    validate_max_length(&bill_category, 50, "账目分类不能超过50个字符")?;
    validate_max_length(&transaction_type, 20, "收支类型不能超过20个字符")?;
    if input.amount_cents < 0 {
        return Err("收支金额不能为负数".into());
    }
    Ok(InvestmentTenant {
        tenant_id,
        customer_id,
        bill_name,
        bill_category,
        amount_cents: input.amount_cents,
        transaction_type,
        transaction_time: input.transaction_time,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
