//! 厂房维护记录的创建、更新与删除。

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
pub struct FactoryMaintenanceInput {
    pub maintenance_item: String,
    pub maintenance_status: String,
    pub person_in_charge: String,
    pub start_time: Timestamp,
    pub end_time: Option<Timestamp>,
    pub remark: Option<String>,
    pub factory_id: u64,
    pub park_id: u64,
}

#[spacetimedb::reducer]
pub fn create_factory_maintenance(
    ctx: &ReducerContext,
    input: FactoryMaintenanceInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_row(ctx, 0, customer_id, input)?;
    ctx.db.factory_maintenance().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_factory_maintenance(
    ctx: &ReducerContext,
    id: u64,
    input: FactoryMaintenanceInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_record(ctx, id)?;
    let mut row = validated_row(ctx, id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db
        .factory_maintenance()
        .factory_maintenance_id()
        .update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_factory_maintenance(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_record(ctx, id)?;
    ctx.db
        .factory_maintenance()
        .factory_maintenance_id()
        .delete(id);
    Ok(())
}

fn require_record(ctx: &ReducerContext, id: u64) -> Result<FactoryMaintenance, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .factory_maintenance()
        .factory_maintenance_id()
        .find(id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("厂房维护记录不存在".into())
}

fn validated_row(
    ctx: &ReducerContext,
    factory_maintenance_id: u64,
    customer_id: String,
    input: FactoryMaintenanceInput,
) -> Result<FactoryMaintenance, String> {
    validate_location(ctx, input.park_id, Some(input.factory_id))?;
    if input
        .end_time
        .is_some_and(|end_time| end_time < input.start_time)
    {
        return Err("维护结束时间不能早于开始时间".into());
    }
    let maintenance_item = required_text(input.maintenance_item, "维护项目不能为空")?;
    let maintenance_status = required_text(input.maintenance_status, "维护状态不能为空")?;
    let person_in_charge = required_text(input.person_in_charge, "负责人不能为空")?;
    validate_max_length(&maintenance_item, 100, "维护项目不能超过100个字符")?;
    validate_max_length(&maintenance_status, 20, "维护状态不能超过20个字符")?;
    validate_max_length(&person_in_charge, 50, "负责人不能超过50个字符")?;
    Ok(FactoryMaintenance {
        factory_maintenance_id,
        customer_id,
        maintenance_item,
        maintenance_status,
        person_in_charge,
        start_time: input.start_time,
        end_time: input.end_time,
        remark: limited_optional(input.remark, 100, "备注不能超过100个字符")?,
        factory_id: input.factory_id,
        park_id: input.park_id,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
