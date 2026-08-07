//! 定位打卡记录事务逻辑。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::{current_enabled_user, validate_coordinate};
use crate::{
    reducers::shared::access::{current_customer_id, require_admin},
    tables::*,
};

/// 定位打卡记录的可编辑参数。
#[derive(SpacetimeType)]
pub struct LocalizationInput {
    pub punch_time: Timestamp,
    pub status: i32,
    pub longitude_e15: i64,
    pub latitude_e15: i64,
}

fn require_localization(
    ctx: &ReducerContext,
    localization_id: u64,
) -> Result<Localization, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .localization()
        .localization_id()
        .find(localization_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("定位打卡记录不存在".into())
}

fn require_owner_or_admin(
    ctx: &ReducerContext,
    localization: &Localization,
    user_id: u64,
) -> Result<(), String> {
    if localization.user_id != user_id && require_admin(ctx).is_err() {
        return Err("没有权限修改他人的定位打卡记录".into());
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn create_localization(ctx: &ReducerContext, input: LocalizationInput) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    validate_coordinate(input.longitude_e15, input.latitude_e15)?;
    ctx.db.localization().insert(Localization {
        localization_id: 0,
        customer_id: user.customer_id,
        punch_time: input.punch_time,
        user_name: user.real_name,
        status: input.status,
        longitude_e15: input.longitude_e15,
        latitude_e15: input.latitude_e15,
        user_id: user.id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_localization(
    ctx: &ReducerContext,
    localization_id: u64,
    input: LocalizationInput,
) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    validate_coordinate(input.longitude_e15, input.latitude_e15)?;
    let mut row = require_localization(ctx, localization_id)?;
    require_owner_or_admin(ctx, &row, user.id)?;
    row.punch_time = input.punch_time;
    row.status = input.status;
    row.longitude_e15 = input.longitude_e15;
    row.latitude_e15 = input.latitude_e15;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.localization().localization_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_localization(ctx: &ReducerContext, localization_id: u64) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    let row = require_localization(ctx, localization_id)?;
    require_owner_or_admin(ctx, &row, user.id)?;
    ctx.db
        .localization()
        .localization_id()
        .delete(localization_id);
    Ok(())
}
