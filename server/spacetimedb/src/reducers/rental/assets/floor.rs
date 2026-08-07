//! 厂房楼层创建、更新与逻辑删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{
            current_customer_id, require_factory, require_factory_floor, require_park_access,
            require_rental_manager,
        },
        park_ref::has_park,
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

/// 楼层可修改字段，所有小数字段均已乘以一百。
#[derive(SpacetimeType)]
pub struct FactoryFloorInput {
    pub floor_name: String,
    pub floor_height_centi_metres: Option<i64>,
    pub load_bearing_centi_units: Option<i64>,
    pub rent_price_cents: i64,
    pub total_area_centi_square_metres: i64,
    pub description: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_factory_floor(
    ctx: &ReducerContext,
    factory_id: u64,
    input: FactoryFloorInput,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let factory = require_factory(ctx, factory_id)?;
    if has_park(factory.park_id) {
        require_park_access(ctx, factory.park_id)?;
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_floor(ctx, 0, factory_id, customer_id, input)?;
    ctx.db.factory_floor().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_factory_floor(
    ctx: &ReducerContext,
    floor_id: u64,
    input: FactoryFloorInput,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let existing = require_factory_floor(ctx, floor_id)?;
    let factory = require_factory(ctx, existing.factory_id)?;
    if has_park(factory.park_id) {
        require_park_access(ctx, factory.park_id)?;
    }
    let mut row = validated_floor(
        ctx,
        floor_id,
        existing.factory_id,
        existing.customer_id,
        input,
    )?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.factory_floor().floor_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_factory_floor(ctx: &ReducerContext, floor_id: u64) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let mut floor = require_factory_floor(ctx, floor_id)?;
    let factory = require_factory(ctx, floor.factory_id)?;
    if has_park(factory.park_id) {
        require_park_access(ctx, factory.park_id)?;
    }
    delete_floor_image_links(ctx, floor_id);
    floor.is_deleted = true;
    floor.updated_at = Some(ctx.timestamp);
    ctx.db.factory_floor().floor_id().update(floor);
    Ok(())
}

pub(super) fn delete_floor_image_links(ctx: &ReducerContext, floor_id: u64) {
    let link_ids = ctx
        .db
        .factory_floor_image()
        .floor_image_by_floor()
        .filter(floor_id)
        .map(|link| link.id)
        .collect::<Vec<_>>();
    for id in link_ids {
        ctx.db.factory_floor_image().id().delete(id);
    }
}

pub(super) fn validated_floor(
    ctx: &ReducerContext,
    floor_id: u64,
    factory_id: u64,
    customer_id: String,
    input: FactoryFloorInput,
) -> Result<FactoryFloor, String> {
    require_factory(ctx, factory_id)?;
    let floor_name = required_text(input.floor_name, "楼层名称不能为空")?;
    if input.rent_price_cents < 0
        || input.total_area_centi_square_metres < 0
        || input
            .floor_height_centi_metres
            .is_some_and(|value| value < 0)
        || input
            .load_bearing_centi_units
            .is_some_and(|value| value < 0)
    {
        return Err("楼层数值不能为负数".into());
    }
    Ok(FactoryFloor {
        floor_id,
        customer_id,
        factory_id,
        floor_name,
        floor_height_centi_metres: input.floor_height_centi_metres,
        load_bearing_centi_units: input.load_bearing_centi_units,
        rent_price_cents: input.rent_price_cents,
        total_area_centi_square_metres: input.total_area_centi_square_metres,
        description: normalize_optional_text(input.description),
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
