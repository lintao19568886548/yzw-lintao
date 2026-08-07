//! 园区和楼层的图片绑定逻辑。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::shared::access::{
        AdminContext, current_customer_id, require_dormitory, require_factory_floor, require_image,
        require_park, require_rental_tenant, require_salary,
    },
    tables::*,
};

#[spacetimedb::reducer]
pub fn bind_image_to_park(ctx: &ReducerContext, park_id: u64, img_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_park(ctx, park_id)?;
    require_image(ctx, img_id)?;
    if ctx
        .db
        .park_image()
        .park_image_by_pair()
        .filter((park_id, img_id))
        .next()
        .is_some()
    {
        return Err("园区已经绑定该图片".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.park_image().insert(ParkImage {
        id: 0,
        customer_id,
        park_id,
        img_id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn unbind_image_from_park(
    ctx: &ReducerContext,
    park_id: u64,
    img_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_park(ctx, park_id)?;
    let link = ctx
        .db
        .park_image()
        .park_image_by_pair()
        .filter((park_id, img_id))
        .next()
        .ok_or("园区没有绑定该图片")?;
    ctx.db.park_image().id().delete(link.id);
    Ok(())
}

#[spacetimedb::reducer]
pub fn bind_image_to_factory_floor(
    ctx: &ReducerContext,
    floor_id: u64,
    img_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_factory_floor(ctx, floor_id)?;
    require_image(ctx, img_id)?;
    if ctx
        .db
        .factory_floor_image()
        .floor_image_by_pair()
        .filter((floor_id, img_id))
        .next()
        .is_some()
    {
        return Err("楼层已经绑定该图片".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.factory_floor_image().insert(FactoryFloorImage {
        id: 0,
        customer_id,
        floor_id,
        img_id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn unbind_image_from_factory_floor(
    ctx: &ReducerContext,
    floor_id: u64,
    img_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_factory_floor(ctx, floor_id)?;
    let link = ctx
        .db
        .factory_floor_image()
        .floor_image_by_pair()
        .filter((floor_id, img_id))
        .next()
        .ok_or("楼层没有绑定该图片")?;
    ctx.db.factory_floor_image().id().delete(link.id);
    Ok(())
}

#[spacetimedb::reducer]
pub fn bind_image_to_dormitory(
    ctx: &ReducerContext,
    dormitory_id: u64,
    img_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_dormitory(ctx, dormitory_id)?;
    require_image(ctx, img_id)?;
    if ctx
        .db
        .dormitory_image()
        .dormitory_image_by_pair()
        .filter((dormitory_id, img_id))
        .next()
        .is_some()
    {
        return Err("宿舍已经绑定该图片".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.dormitory_image().insert(DormitoryImage {
        id: 0,
        customer_id,
        dormitory_id,
        img_id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn unbind_image_from_dormitory(
    ctx: &ReducerContext,
    dormitory_id: u64,
    img_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_dormitory(ctx, dormitory_id)?;
    let link = ctx
        .db
        .dormitory_image()
        .dormitory_image_by_pair()
        .filter((dormitory_id, img_id))
        .next()
        .ok_or("宿舍没有绑定该图片")?;
    ctx.db.dormitory_image().id().delete(link.id);
    Ok(())
}

#[spacetimedb::reducer]
pub fn bind_image_to_rental_tenant(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    img_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_rental_tenant(ctx, rental_tenant_id)?;
    require_image(ctx, img_id)?;
    if ctx
        .db
        .tenant_image()
        .tenant_image_by_pair()
        .filter((rental_tenant_id, img_id))
        .next()
        .is_some()
    {
        return Err("租赁客户已经绑定该图片".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.tenant_image().insert(TenantImage {
        id: 0,
        customer_id,
        rental_tenant_id,
        img_id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn unbind_image_from_rental_tenant(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    img_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_rental_tenant(ctx, rental_tenant_id)?;
    let link = ctx
        .db
        .tenant_image()
        .tenant_image_by_pair()
        .filter((rental_tenant_id, img_id))
        .next()
        .ok_or("租赁客户没有绑定该图片")?;
    ctx.db.tenant_image().id().delete(link.id);
    Ok(())
}

#[spacetimedb::reducer]
pub fn bind_image_to_salary(
    ctx: &ReducerContext,
    salary_id: u64,
    img_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_salary(ctx, salary_id)?;
    require_image(ctx, img_id)?;
    if ctx
        .db
        .salary_image()
        .salary_image_by_pair()
        .filter((salary_id, img_id))
        .next()
        .is_some()
    {
        return Err("工资记录已经绑定该图片".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.salary_image().insert(SalaryImage {
        id: 0,
        customer_id,
        salary_id,
        img_id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn unbind_image_from_salary(
    ctx: &ReducerContext,
    salary_id: u64,
    img_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_salary(ctx, salary_id)?;
    let link = ctx
        .db
        .salary_image()
        .salary_image_by_pair()
        .filter((salary_id, img_id))
        .next()
        .ok_or("工资记录没有绑定该图片")?;
    ctx.db.salary_image().id().delete(link.id);
    Ok(())
}
