//! 车辆出入登记的创建、更新与删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::validate_status;
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        park_ref::{NO_PARK, optional_park_ref},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 车辆登记可修改字段；未传登记时间时使用服务器当前时间。
#[derive(SpacetimeType)]
pub struct AccessCarInput {
    /// 车辆牌照号码；保存前会去除首尾空白并执行必填与长度校验。
    pub car_number: String,
    /// 车辆登记状态，仅允许使用 `0` 或 `1`。
    pub status: i8,
    /// 车辆登记时间；创建记录时未提供则使用服务器当前时间。
    pub register_time: Option<Timestamp>,
    /// 可选备注；空白内容会被规范化为空值。
    pub remark: Option<String>,
    /// 可选的所属园区编号；提供时必须关联到有效园区。
    pub park_id: Option<u64>,
}

#[spacetimedb::reducer]
pub fn create_access_car(ctx: &ReducerContext, input: AccessCarInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_car(ctx, 0, customer_id, ctx.timestamp, input)?;
    ctx.db.access_car().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_access_car(
    ctx: &ReducerContext,
    car_id: u64,
    input: AccessCarInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_car(ctx, car_id)?;
    let mut row = validated_car(
        ctx,
        car_id,
        existing.customer_id,
        existing.register_time,
        input,
    )?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.access_car().car_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_access_car(ctx: &ReducerContext, car_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_car(ctx, car_id)?;
    ctx.db.access_car().car_id().delete(car_id);
    Ok(())
}

fn require_car(ctx: &ReducerContext, car_id: u64) -> Result<AccessCar, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .access_car()
        .car_id()
        .find(car_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("车辆登记不存在".into())
}

fn validated_car(
    ctx: &ReducerContext,
    car_id: u64,
    customer_id: String,
    default_register_time: Timestamp,
    input: AccessCarInput,
) -> Result<AccessCar, String> {
    let park_id = optional_park_ref(ctx, input.park_id.unwrap_or(NO_PARK))?;
    let car_number = required_text(input.car_number, "车牌号码不能为空")?;
    validate_max_length(&car_number, 20, "车牌号码不能超过20个字符")?;
    let remark = normalize_optional_text(input.remark);
    if let Some(value) = &remark {
        validate_max_length(value, 200, "备注不能超过200个字符")?;
    }
    Ok(AccessCar {
        car_id,
        customer_id,
        car_number,
        status: validate_status(input.status, "车辆出入状态参数错误")?,
        register_time: input.register_time.unwrap_or(default_register_time),
        remark,
        park_id,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
