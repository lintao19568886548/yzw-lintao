//! 厂房创建、更新与逻辑删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{
            current_customer_id, require_factory, require_park_access,
            require_park_access_unless_archived, require_rental_manager,
        },
        park_ref::{has_park, optional_park_ref, NO_PARK},
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

use super::floor_reducer::{FactoryFloorInput, delete_floor_image_links, validated_floor};

/// 厂房可修改字段。
#[derive(SpacetimeType)]
pub struct FactoryInput {
    pub factory_name: String,
    pub park_id: Option<u64>,
    pub build_date: Option<String>,
    pub description: Option<String>,
    pub is_own: bool,
}

#[spacetimedb::reducer]
pub fn create_factory(ctx: &ReducerContext, input: FactoryInput) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_factory(ctx, 0, customer_id, input)?;
    ctx.db.factory().insert(row);
    Ok(())
}

/// 新增厂房时同时写入楼层，保证资产父子关系在一个事务内完成。
#[spacetimedb::reducer]
pub fn create_factory_with_floors(
    ctx: &ReducerContext,
    input: FactoryInput,
    floors: Vec<FactoryFloorInput>,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    if floors.len() > 100 {
        return Err("单个厂房一次最多新增 100 个楼层".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let factory = validated_factory(ctx, 0, customer_id.clone(), input)?;
    let factory = ctx.db.factory().insert(factory);

    // Reducer 具有事务性，任意楼层校验失败时厂房主记录也会自动回滚。
    for floor_input in floors {
        let floor = validated_floor(ctx, 0, factory.factory_id, customer_id.clone(), floor_input)?;
        ctx.db.factory_floor().insert(floor);
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_factory(
    ctx: &ReducerContext,
    factory_id: u64,
    input: FactoryInput,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let existing = require_factory(ctx, factory_id)?;
    if has_park(existing.park_id) {
        require_park_access(ctx, existing.park_id)?;
    }
    let mut row = validated_factory(ctx, factory_id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.factory().factory_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_factory(ctx: &ReducerContext, factory_id: u64) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let factory = require_factory(ctx, factory_id)?;
    if has_park(factory.park_id) {
        // 园区已注销时放行：否则孤立厂房永远清不掉。
        require_park_access_unless_archived(ctx, factory.park_id)?;
    }
    soft_delete_factory_tree(ctx, factory);
    Ok(())
}

/// 注销一栋厂房及其全部楼层。
///
/// 单独抽出来是为了让「删园区」能复用同一段逻辑：厂房下面挂着楼层、楼层下面
/// 挂着图片关联，每个入口各写一遍迟早会漏——园区删除原先就漏了整棵树，删掉
/// 园区之后厂房还活着，界面上变成一栋找不到园区的孤立厂房。
pub(crate) fn soft_delete_factory_tree(ctx: &ReducerContext, mut factory: Factory) {
    let factory_id = factory.factory_id;
    factory.is_deleted = true;
    factory.updated_at = Some(ctx.timestamp);
    ctx.db.factory().factory_id().update(factory);

    // 楼层随厂房一同逻辑删除，避免后续业务读取到孤立资产。
    let floors = ctx
        .db
        .factory_floor()
        .factory_floor_by_factory()
        .filter(factory_id)
        .filter(|floor| !floor.is_deleted)
        .collect::<Vec<_>>();
    for mut floor in floors {
        delete_floor_image_links(ctx, floor.floor_id);
        floor.is_deleted = true;
        floor.updated_at = Some(ctx.timestamp);
        ctx.db.factory_floor().floor_id().update(floor);
    }
}

fn validated_factory(
    ctx: &ReducerContext,
    factory_id: u64,
    customer_id: String,
    input: FactoryInput,
) -> Result<Factory, String> {
    let factory_name = required_text(input.factory_name, "厂房名称不能为空")?;
    let park_id = optional_park_ref(ctx, input.park_id.unwrap_or(NO_PARK))?;
    if park_id != NO_PARK {
        require_park_access(ctx, park_id)?;
    }
    Ok(Factory {
        factory_id,
        customer_id,
        factory_name,
        park_id,
        build_date: normalize_optional_text(input.build_date),
        description: normalize_optional_text(input.description),
        is_own: input.is_own,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
