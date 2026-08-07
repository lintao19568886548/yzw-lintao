//! 宿舍创建、更新与逻辑删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{
            current_customer_id, require_dormitory, require_park, require_park_access,
            require_park_access_unless_archived, require_rental_manager,
        },
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

/// 宿舍楼可修改字段。
///
/// 层高、挂牌租金、房间数都下沉到 `DormitoryFloor` 逐层维护；占用情况由
/// 合同关联算出，都不在这里填。
#[derive(SpacetimeType)]
pub struct DormitoryInput {
    pub park_id: u64,
    pub dormitory_name: String,
    pub remark: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_dormitory(ctx: &ReducerContext, input: DormitoryInput) -> Result<(), String> {
    require_rental_manager(ctx)?;
    require_park_access(ctx, input.park_id)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_dormitory(ctx, 0, customer_id, input)?;
    ctx.db.dormitory_building().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_dormitory(
    ctx: &ReducerContext,
    dormitory_id: u64,
    input: DormitoryInput,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let existing = require_dormitory(ctx, dormitory_id)?;
    require_park_access(ctx, existing.park_id)?;
    // 迁移到别的园区时，目标园区也必须落在数据范围内。
    require_park_access(ctx, input.park_id)?;
    let mut row = validated_dormitory(ctx, dormitory_id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.dormitory_building().dormitory_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_dormitory(ctx: &ReducerContext, dormitory_id: u64) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let dormitory = require_dormitory(ctx, dormitory_id)?;
    // 园区已注销时放行：否则孤立宿舍永远清不掉。
    require_park_access_unless_archived(ctx, dormitory.park_id)?;
    soft_delete_dormitory_tree(ctx, dormitory);
    Ok(())
}

/// 注销一栋宿舍及其全部楼层。
///
/// 楼层的级联原先是漏的：宿舍注销之后楼层还活着，宿舍面积按「房间数 × 单间
/// 面积」求和时照旧被算进园区，界面上却找不到它属于哪栋楼。跟厂房那棵树同一
/// 个形状，也同样被「删园区」复用。
pub(crate) fn soft_delete_dormitory_tree(ctx: &ReducerContext, mut dormitory: Dormitory) {
    let dormitory_id = dormitory.dormitory_id;
    let link_ids = ctx
        .db
        .dormitory_image()
        .dormitory_image_by_dormitory()
        .filter(dormitory_id)
        .map(|link| link.id)
        .collect::<Vec<_>>();
    for id in link_ids {
        ctx.db.dormitory_image().id().delete(id);
    }

    let floors = ctx
        .db
        .dormitory_floor()
        .dormitory_floor_by_dormitory()
        .filter(dormitory_id)
        .filter(|floor| !floor.is_deleted)
        .collect::<Vec<_>>();
    for mut floor in floors {
        floor.is_deleted = true;
        floor.updated_at = Some(ctx.timestamp);
        ctx.db
            .dormitory_floor()
            .dormitory_floor_id()
            .update(floor);
    }

    dormitory.is_deleted = true;
    dormitory.updated_at = Some(ctx.timestamp);
    ctx.db.dormitory_building().dormitory_id().update(dormitory);
}

pub(super) fn validated_dormitory(
    ctx: &ReducerContext,
    dormitory_id: u64,
    customer_id: String,
    input: DormitoryInput,
) -> Result<Dormitory, String> {
    require_park(ctx, input.park_id)?;
    let dormitory_name = required_text(input.dormitory_name, "宿舍名称不能为空")?;
    Ok(Dormitory {
        dormitory_id,
        customer_id,
        park_id: input.park_id,
        dormitory_name,
        remark: normalize_optional_text(input.remark),
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
