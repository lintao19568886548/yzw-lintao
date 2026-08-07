//! 访客出入登记的创建、更新与删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::{validate_phone_number, validate_status};
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        park_ref::{NO_PARK, optional_park_ref},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 访客登记可修改字段；未传登记时间时使用服务器当前时间。
#[derive(SpacetimeType)]
pub struct AccessVisitorInput {
    pub visitor_name: String,
    pub car_num: Option<String>,
    pub phone_number: String,
    pub status: i8,
    pub register_time: Option<Timestamp>,
    pub remark: Option<String>,
    pub park_id: Option<u64>,
}

#[spacetimedb::reducer]
pub fn create_access_visitor(
    ctx: &ReducerContext,
    input: AccessVisitorInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_visitor(ctx, 0, customer_id, ctx.timestamp, input)?;
    ctx.db.access_visitor().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_access_visitor(
    ctx: &ReducerContext,
    visitor_id: u64,
    input: AccessVisitorInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_visitor(ctx, visitor_id)?;
    let mut row = validated_visitor(
        ctx,
        visitor_id,
        existing.customer_id,
        existing.register_time,
        input,
    )?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.access_visitor().visitor_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_access_visitor(ctx: &ReducerContext, visitor_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_visitor(ctx, visitor_id)?;
    ctx.db.access_visitor().visitor_id().delete(visitor_id);
    Ok(())
}

fn require_visitor(ctx: &ReducerContext, visitor_id: u64) -> Result<AccessVisitor, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .access_visitor()
        .visitor_id()
        .find(visitor_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("访客登记不存在".into())
}

fn validated_visitor(
    ctx: &ReducerContext,
    visitor_id: u64,
    customer_id: String,
    default_register_time: Timestamp,
    input: AccessVisitorInput,
) -> Result<AccessVisitor, String> {
    let park_id = optional_park_ref(ctx, input.park_id.unwrap_or(NO_PARK))?;
    let visitor_name = required_text(input.visitor_name, "姓名不能为空")?;
    validate_max_length(&visitor_name, 50, "姓名不能超过50个字符")?;
    let remark = normalize_optional_text(input.remark);
    if let Some(value) = &remark {
        validate_max_length(value, 100, "备注不能超过100个字符")?;
    }
    Ok(AccessVisitor {
        visitor_id,
        customer_id,
        visitor_name,
        car_num: normalize_optional_text(input.car_num),
        phone_number: validate_phone_number(input.phone_number)?,
        status: validate_status(input.status, "访问状态参数错误")?,
        register_time: input.register_time.unwrap_or(default_register_time),
        remark,
        park_id,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
