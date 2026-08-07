//! 角色权限码授权。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::shared::access::{AdminContext, current_customer_id, require_code, require_role},
    tables::*,
};

#[spacetimedb::reducer]
pub fn assign_code_to_role(ctx: &ReducerContext, role_id: u64, code_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_role(ctx, role_id)?;
    require_code(ctx, code_id)?;
    if ctx
        .db
        .role_code()
        .role_code_by_pair()
        .filter((role_id, code_id))
        .next()
        .is_some()
    {
        return Err("角色权限码关联已经存在".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.role_code().insert(RoleCode {
        id: 0,
        customer_id,
        role_id,
        code_id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_code_from_role(
    ctx: &ReducerContext,
    role_id: u64,
    code_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_role(ctx, role_id)?;
    let link = ctx
        .db
        .role_code()
        .role_code_by_pair()
        .filter((role_id, code_id))
        .next()
        .ok_or("角色没有该权限码")?;
    ctx.db.role_code().id().delete(link.id);
    Ok(())
}
