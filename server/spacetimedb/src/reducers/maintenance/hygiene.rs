//! 卫生检查记录维护。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::{limited_optional, validate_location};
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct HygieneCheckInput {
    pub check_items: String,
    pub checker: String,
    pub check_date: Timestamp,
    pub check_result: String,
    pub remark: Option<String>,
    pub factory_id: u64,
    pub park_id: u64,
}

#[spacetimedb::reducer]
pub fn create_hygiene_check(ctx: &ReducerContext, input: HygieneCheckInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .hygiene_check()
        .insert(validated_row(ctx, 0, customer_id, input)?);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_hygiene_check(
    ctx: &ReducerContext,
    id: u64,
    input: HygieneCheckInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_record(ctx, id)?;
    let mut row = validated_row(ctx, id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.hygiene_check().hygiene_check_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_hygiene_check(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_record(ctx, id)?;
    ctx.db.hygiene_check().hygiene_check_id().delete(id);
    Ok(())
}

fn require_record(ctx: &ReducerContext, id: u64) -> Result<HygieneCheck, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .hygiene_check()
        .hygiene_check_id()
        .find(id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("卫生检查记录不存在".into())
}

fn validated_row(
    ctx: &ReducerContext,
    hygiene_check_id: u64,
    customer_id: String,
    input: HygieneCheckInput,
) -> Result<HygieneCheck, String> {
    validate_location(ctx, input.park_id, Some(input.factory_id))?;
    let check_items = required_text(input.check_items, "检查项目不能为空")?;
    let checker = required_text(input.checker, "检查人不能为空")?;
    let check_result = required_text(input.check_result, "检查结果不能为空")?;
    validate_max_length(&check_items, 100, "检查项目不能超过100个字符")?;
    validate_max_length(&checker, 10, "检查人不能超过10个字符")?;
    validate_max_length(&check_result, 100, "检查结果不能超过100个字符")?;
    Ok(HygieneCheck {
        hygiene_check_id,
        customer_id,
        check_items,
        checker,
        check_date: input.check_date,
        check_result,
        remark: limited_optional(input.remark, 100, "备注不能超过100个字符")?,
        factory_id: input.factory_id,
        park_id: input.park_id,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
