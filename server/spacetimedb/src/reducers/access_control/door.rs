//! 门禁设备的创建、更新与删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::common::validate_status;
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        park_ref::{NO_PARK, optional_park_ref},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 门禁设备可修改字段。
#[derive(SpacetimeType)]
pub struct AccessDoorInput {
    pub device_code: String,
    pub device_name: String,
    pub location: String,
    pub status: i8,
    pub park_id: Option<u64>,
}

#[spacetimedb::reducer]
pub fn create_access_door(ctx: &ReducerContext, input: AccessDoorInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_door(ctx, 0, customer_id, input)?;
    ctx.db.access_door().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_access_door(
    ctx: &ReducerContext,
    device_id: u64,
    input: AccessDoorInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_door(ctx, device_id)?;
    let mut row = validated_door(ctx, device_id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.access_door().device_id().update(row);
    Ok(())
}

/// 仅切换门禁开关状态，对应原项目的门禁状态接口。
#[spacetimedb::reducer]
pub fn set_access_door_status(
    ctx: &ReducerContext,
    device_id: u64,
    status: i8,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut row = require_door(ctx, device_id)?;
    row.status = validate_status(status, "门禁状态参数错误")?;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.access_door().device_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_access_door(ctx: &ReducerContext, device_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_door(ctx, device_id)?;
    ctx.db.access_door().device_id().delete(device_id);
    Ok(())
}

fn require_door(ctx: &ReducerContext, device_id: u64) -> Result<AccessDoor, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .access_door()
        .device_id()
        .find(device_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("门禁设备不存在".into())
}

fn validated_door(
    ctx: &ReducerContext,
    device_id: u64,
    customer_id: String,
    input: AccessDoorInput,
) -> Result<AccessDoor, String> {
    let park_id = optional_park_ref(ctx, input.park_id.unwrap_or(NO_PARK))?;
    let device_code = required_text(input.device_code, "设备编码不能为空")?;
    validate_max_length(&device_code, 30, "设备编码不能超过30个字符")?;
    let device_name = required_text(input.device_name, "设备名称不能为空")?;
    validate_max_length(&device_name, 50, "设备名称不能超过50个字符")?;
    let location = required_text(input.location, "设备位置不能为空")?;
    validate_max_length(&location, 100, "设备位置不能超过100个字符")?;
    let duplicate = ctx
        .db
        .access_door()
        .access_door_by_customer_code()
        .filter((customer_id.as_str(), device_code.as_str()))
        .any(|row| row.device_id != device_id);
    if duplicate {
        return Err("设备编码已存在".into());
    }
    Ok(AccessDoor {
        device_id,
        customer_id,
        device_code,
        device_name,
        location,
        status: validate_status(input.status, "门禁状态参数错误")?,
        park_id,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
