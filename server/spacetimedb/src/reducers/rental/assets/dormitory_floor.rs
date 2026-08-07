//! 宿舍楼层的增删改，以及楼层占用情况的推导。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{current_customer_id, require_dormitory, require_park_access, require_rental_manager},
        validation::normalize_optional_text,
    },
    tables::*,
};

/// 宿舍楼层可修改字段，小数字段均乘以一百后传入。
#[derive(SpacetimeType)]
pub struct DormitoryFloorInput {
    pub dormitory_id: u64,
    pub floor_no: i32,
    pub room_count: i32,
    pub room_area_centi_square_metres: Option<i64>,
    pub floor_height_centi_metres: Option<i64>,
    /// 对外报价，不是成交价。
    pub rent_price_cents: Option<i64>,
    pub remark: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_dormitory_floor(
    ctx: &ReducerContext,
    input: DormitoryFloorInput,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let dormitory = require_dormitory(ctx, input.dormitory_id)?;
    require_park_access(ctx, dormitory.park_id)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_dormitory_floor(ctx, 0, customer_id, input)?;
    ctx.db.dormitory_floor().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_dormitory_floor(
    ctx: &ReducerContext,
    dormitory_floor_id: u64,
    input: DormitoryFloorInput,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let existing = require_dormitory_floor(ctx, dormitory_floor_id)?;
    let dormitory = require_dormitory(ctx, existing.dormitory_id)?;
    require_park_access(ctx, dormitory.park_id)?;
    // 缩减房间数时不能少于已被合同占用的间数，否则占用率会算出超过 100%。
    let occupied = occupied_rooms(ctx, dormitory_floor_id);
    if input.room_count < occupied {
        return Err(format!(
            "该层已有合同占用 {occupied} 间，房间数不能少于这个数量"
        ));
    }
    let mut row = validated_dormitory_floor(
        ctx,
        dormitory_floor_id,
        existing.customer_id.clone(),
        input,
    )?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db
        .dormitory_floor()
        .dormitory_floor_id()
        .update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_dormitory_floor(
    ctx: &ReducerContext,
    dormitory_floor_id: u64,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let mut floor = require_dormitory_floor(ctx, dormitory_floor_id)?;
    let dormitory = require_dormitory(ctx, floor.dormitory_id)?;
    require_park_access(ctx, dormitory.park_id)?;
    let occupied = occupied_rooms(ctx, dormitory_floor_id);
    if occupied > 0 {
        return Err(format!(
            "该层还有 {occupied} 间被合同占用，请先解除合同关联再删除"
        ));
    }
    floor.is_deleted = true;
    floor.updated_at = Some(ctx.timestamp);
    ctx.db
        .dormitory_floor()
        .dormitory_floor_id()
        .update(floor);
    Ok(())
}

/// 这一层被有效合同占用了多少间。
///
/// 这就是原来那个人工填写的「已用房间数」的替代品：房间总数固定，
/// 占用数从合同关联算出来，两者相减即为可租。
pub(crate) fn occupied_rooms(ctx: &ReducerContext, dormitory_floor_id: u64) -> i32 {
    ctx.db
        .rental_tenant_dormitory_floor()
        .rental_tenant_dormitory_floor_by_floor()
        .filter(dormitory_floor_id)
        .filter(|link| {
            ctx.db
                .rental_tenant()
                .rental_tenant_id()
                .find(link.rental_tenant_id)
                .is_some_and(|tenant| !tenant.is_deleted)
        })
        .map(|link| link.room_count)
        .sum()
}

fn require_dormitory_floor(
    ctx: &ReducerContext,
    dormitory_floor_id: u64,
) -> Result<DormitoryFloor, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .dormitory_floor()
        .dormitory_floor_id()
        .find(dormitory_floor_id)
        .filter(|floor| floor.customer_id == customer_id && !floor.is_deleted)
        .ok_or("宿舍楼层不存在".into())
}

fn validated_dormitory_floor(
    ctx: &ReducerContext,
    dormitory_floor_id: u64,
    customer_id: String,
    input: DormitoryFloorInput,
) -> Result<DormitoryFloor, String> {
    if input.floor_no <= 0 {
        return Err("楼层号必须大于零".into());
    }
    if input.room_count < 0 {
        return Err("房间数不能为负数".into());
    }
    if input
        .room_area_centi_square_metres
        .is_some_and(|value| value < 0)
        || input.floor_height_centi_metres.is_some_and(|value| value < 0)
        || input.rent_price_cents.is_some_and(|value| value < 0)
    {
        return Err("面积、层高和挂牌租金不能为负数".into());
    }
    Ok(DormitoryFloor {
        dormitory_floor_id,
        customer_id,
        dormitory_id: input.dormitory_id,
        floor_no: input.floor_no,
        room_count: input.room_count,
        room_area_centi_square_metres: input.room_area_centi_square_metres,
        floor_height_centi_metres: input.floor_height_centi_metres,
        rent_price_cents: input.rent_price_cents,
        remark: normalize_optional_text(input.remark),
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
