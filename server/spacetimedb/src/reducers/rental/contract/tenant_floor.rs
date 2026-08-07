//! 合同与楼层归属关系的维护。
//!
//! 语义是「整体替换」：一次调用给出这份合同租用的全部楼层，服务端把旧的
//! 关联行删掉、按新列表重建。相比逐条增删，调用方（表单）不需要自己算
//! 差集，也不会出现改到一半只写进去一部分的中间状态。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{
            require_dormitory, require_factory, require_factory_floor, require_rental_tenant,
            AdminContext,
        },
        park_ref::has_park,
    },
    tables::*,
};

/// 合同在某一层占用的面积。
#[derive(SpacetimeType)]
pub struct RentalTenantFloorInput {
    pub floor_id: u64,
    pub area_centi_square_metres: i64,
}

/// 整体替换某份合同的楼层归属。传空列表即解除全部关联。
///
/// 合同表单走的是 `create_rental_tenant` / `update_rental_tenant` 里内嵌的
/// 同一套逻辑；这个独立入口留给历史数据补录——不改合同其他字段，只补楼层。
#[spacetimedb::reducer]
pub fn set_rental_tenant_floors(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    floors: Vec<RentalTenantFloorInput>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let tenant = require_rental_tenant(ctx, rental_tenant_id)?;
    replace_rental_tenant_floors(ctx, rental_tenant_id, &tenant.customer_id, floors)
}

/// 校验并整体重写某份合同的楼层归属。
///
/// 先把整批校验完再落库，校验失败时一行都不写——避免出现"改到一半"的
/// 中间状态（比如三层里只成功写进去两层）。
pub(super) fn replace_rental_tenant_floors(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    customer_id: &str,
    floors: Vec<RentalTenantFloorInput>,
) -> Result<(), String> {
    // 合同所在园区。楼层必须落在同一个园区里，否则会出现「园区 14 的合同
    // 租了园区 21 的楼层」这种记录——表单的楼层列表只列本园区，跨园区的
    // 关联在界面上根本渲染不出来，用户只看到底部计数说选了一层、却一个
    // 复选框都没勾上。生产库里真出过一条。
    let tenant_park_id = ctx
        .db
        .rental_tenant()
        .rental_tenant_id()
        .find(rental_tenant_id)
        .map(|row| row.park_id)
        .unwrap_or(0);

    let mut seen = std::collections::BTreeSet::new();
    for entry in &floors {
        let floor = require_factory_floor(ctx, entry.floor_id)?;
        check_same_park(
            tenant_park_id,
            require_factory(ctx, floor.factory_id)?.park_id,
            &floor.floor_name,
        )?;
        check_floor_area(
            entry.area_centi_square_metres,
            floor.total_area_centi_square_metres,
            &floor.floor_name,
        )?;
        if !seen.insert(entry.floor_id) {
            return Err(format!("楼层「{}」重复出现", floor.floor_name));
        }
    }

    delete_rental_tenant_floor_links(ctx, rental_tenant_id);
    for entry in floors {
        ctx.db.rental_tenant_floor().insert(RentalTenantFloor {
            id: 0,
            customer_id: customer_id.to_string(),
            rental_tenant_id,
            floor_id: entry.floor_id,
            area_centi_square_metres: entry.area_centi_square_metres,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(())
}

/// 合同被删除时一并清掉它的楼层归属，避免留下指向已删合同的关联行。
pub(super) fn delete_rental_tenant_floor_links(ctx: &ReducerContext, rental_tenant_id: u64) {
    for link in ctx
        .db
        .rental_tenant_floor()
        .rental_tenant_floor_by_tenant()
        .filter(rental_tenant_id)
        .collect::<Vec<_>>()
    {
        ctx.db.rental_tenant_floor().id().delete(link.id);
    }
}

/// 楼层与合同必须在同一个园区。
///
/// 合同还没选园区（`park_id` 为 0 哨兵）时不拦——历史数据里确实有这种
/// 合同，拦下来只会让它们连楼层都补录不了。
#[pure_function::pure]
fn check_same_park(tenant_park_id: u64, floor_park_id: u64, floor_name: &str) -> Result<(), String> {
    if has_park(tenant_park_id) && tenant_park_id != floor_park_id {
        return Err(format!("楼层「{floor_name}」不属于本合同所在园区"));
    }
    Ok(())
}

/// 单层占用面积的边界校验。
///
/// 抽成纯函数是为了能覆盖边界：整层出租（相等）必须放行，超出一点点
/// 必须拦住——这条守卫一旦写反，楼层已用面积就会超过总面积，出租率
/// 直接算出大于 100% 的结果。
#[pure_function::pure]
fn check_floor_area(area: i64, total: i64, floor_name: &str) -> Result<(), String> {
    if area < 0 {
        return Err("楼层占用面积不能为负数".into());
    }
    if area > total {
        return Err(format!("楼层「{floor_name}」占用面积不能超过该层总面积"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 整层出租放行超出总面积拦截() {
        assert!(check_floor_area(1_000, 1_000, "1层").is_ok());
        assert!(check_floor_area(400, 1_000, "1层").is_ok());
        assert!(check_floor_area(1_001, 1_000, "1层").is_err());
    }

    #[test]
    fn 跨园区的楼层被拦截() {
        assert!(check_same_park(14, 14, "1层").is_ok());
        assert!(check_same_park(14, 21, "1层").is_err());
    }

    #[test]
    fn 未选园区的合同不拦楼层归属() {
        // 历史合同里确实有没填园区的，拦下来它们连楼层都补录不了。
        assert!(check_same_park(0, 21, "1层").is_ok());
    }

    #[test]
    fn 负面积被拦截() {
        assert!(check_floor_area(-1, 1_000, "1层").is_err());
    }

    #[test]
    fn 未维护总面积的楼层只允许零占用() {
        // 楼层台账没填总面积时 total = 0，此时挂上任何正面积都是错的。
        assert!(check_floor_area(0, 0, "1层").is_ok());
        assert!(check_floor_area(1, 0, "1层").is_err());
    }
}

/// 合同在某一层宿舍占用的房间数。
#[derive(SpacetimeType)]
pub struct RentalTenantDormitoryFloorInput {
    pub dormitory_floor_id: u64,
    pub room_count: i32,
}

/// 整体替换某份合同的宿舍楼层归属。语义与厂房楼层那套一致。
#[spacetimedb::reducer]
pub fn set_rental_tenant_dormitory_floors(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    floors: Vec<RentalTenantDormitoryFloorInput>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let tenant = require_rental_tenant(ctx, rental_tenant_id)?;
    replace_rental_tenant_dormitory_floors(ctx, rental_tenant_id, &tenant.customer_id, floors)
}

pub(super) fn replace_rental_tenant_dormitory_floors(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    customer_id: &str,
    floors: Vec<RentalTenantDormitoryFloorInput>,
) -> Result<(), String> {
    let tenant_park_id = ctx
        .db
        .rental_tenant()
        .rental_tenant_id()
        .find(rental_tenant_id)
        .map(|row| row.park_id)
        .unwrap_or(0);

    let mut seen = std::collections::BTreeSet::new();
    for entry in &floors {
        let Some(floor) = ctx
            .db
            .dormitory_floor()
            .dormitory_floor_id()
            .find(entry.dormitory_floor_id)
            .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        else {
            return Err("宿舍楼层不存在".into());
        };
        check_same_park(
            tenant_park_id,
            require_dormitory(ctx, floor.dormitory_id)?.park_id,
            &format!("宿舍第 {} 层", floor.floor_no),
        )?;
        if entry.room_count < 0 {
            return Err("占用房间数不能为负数".into());
        }
        // 该层被别的合同占掉的间数，不能和本合同一起超过总房间数。
        let others: i32 = ctx
            .db
            .rental_tenant_dormitory_floor()
            .rental_tenant_dormitory_floor_by_floor()
            .filter(entry.dormitory_floor_id)
            .filter(|link| link.rental_tenant_id != rental_tenant_id)
            .filter(|link| {
                ctx.db
                    .rental_tenant()
                    .rental_tenant_id()
                    .find(link.rental_tenant_id)
                    .is_some_and(|row| !row.is_deleted)
            })
            .map(|link| link.room_count)
            .sum();
        if others + entry.room_count > floor.room_count {
            return Err(format!(
                "宿舍第 {} 层共 {} 间，已被其他合同占用 {} 间，本合同最多再占 {} 间",
                floor.floor_no,
                floor.room_count,
                others,
                floor.room_count - others
            ));
        }
        if !seen.insert(entry.dormitory_floor_id) {
            return Err(format!("宿舍第 {} 层重复出现", floor.floor_no));
        }
    }

    delete_rental_tenant_dormitory_floor_links(ctx, rental_tenant_id);
    for entry in floors {
        ctx.db
            .rental_tenant_dormitory_floor()
            .insert(RentalTenantDormitoryFloor {
                id: 0,
                customer_id: customer_id.to_string(),
                rental_tenant_id,
                dormitory_floor_id: entry.dormitory_floor_id,
                room_count: entry.room_count,
                created_at: ctx.timestamp,
                updated_at: None,
            });
    }
    Ok(())
}

pub(super) fn delete_rental_tenant_dormitory_floor_links(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
) {
    for link in ctx
        .db
        .rental_tenant_dormitory_floor()
        .rental_tenant_dormitory_floor_by_tenant()
        .filter(rental_tenant_id)
        .collect::<Vec<_>>()
    {
        ctx.db.rental_tenant_dormitory_floor().id().delete(link.id);
    }
}
