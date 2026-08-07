//! 财务流水创建、更新与逻辑删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_finance},
        park_ref::{NO_PARK, optional_park_ref},
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

use super::images::{delete_finance_image_links, replace_finance_images};

/// 财务流水可修改字段。
#[derive(SpacetimeType)]
pub struct FinanceInput {
    pub bill_name: String,
    pub bill_category: String,
    pub amount_cents: i64,
    pub transaction_type: String,
    pub transaction_time: Timestamp,
    pub remark: Option<String>,
    pub park_id: Option<u64>,
    pub status: i32,
}

#[spacetimedb::reducer]
pub fn create_finance(ctx: &ReducerContext, input: FinanceInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_finance(ctx, 0, customer_id, input)?;
    ctx.db.finance().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn create_finance_with_images(
    ctx: &ReducerContext,
    input: FinanceInput,
    image_urls: Vec<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_finance(ctx, 0, customer_id.clone(), input)?;
    let row = ctx.db.finance().insert(row);
    replace_finance_images(ctx, row.finance_id, customer_id, image_urls)
}

#[spacetimedb::reducer]
pub fn update_finance(
    ctx: &ReducerContext,
    finance_id: u64,
    input: FinanceInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_finance(ctx, finance_id)?;
    let mut row = validated_finance(ctx, finance_id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.finance().finance_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_finance_with_images(
    ctx: &ReducerContext,
    finance_id: u64,
    input: FinanceInput,
    image_urls: Vec<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_finance(ctx, finance_id)?;
    let customer_id = existing.customer_id.clone();
    let mut row = validated_finance(ctx, finance_id, customer_id.clone(), input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.finance().finance_id().update(row);
    replace_finance_images(ctx, finance_id, customer_id, image_urls)
}

#[spacetimedb::reducer]
pub fn delete_finance(ctx: &ReducerContext, finance_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut finance = require_finance(ctx, finance_id)?;
    delete_finance_image_links(ctx, finance_id);
    finance.is_deleted = true;
    finance.updated_at = Some(ctx.timestamp);
    ctx.db.finance().finance_id().update(finance);
    Ok(())
}

fn validated_finance(
    ctx: &ReducerContext,
    finance_id: u64,
    customer_id: String,
    input: FinanceInput,
) -> Result<Finance, String> {
    let bill_name = required_text(input.bill_name, "账目名称不能为空")?;
    let bill_category = required_text(input.bill_category, "账目分类不能为空")?;
    let transaction_type = required_text(input.transaction_type, "收支类型不能为空")?;
    if input.amount_cents < 0 {
        return Err("财务金额不能为负数".into());
    }
    let park_id = optional_park_ref(ctx, input.park_id.unwrap_or(NO_PARK))?;
    Ok(Finance {
        finance_id,
        customer_id,
        bill_name,
        bill_category,
        amount_cents: input.amount_cents,
        transaction_type,
        transaction_time: input.transaction_time,
        remark: normalize_optional_text(input.remark),
        park_id,
        status: input.status,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
