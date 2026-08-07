//! 角色菜单授权。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::shared::access::{AdminContext, require_menu, require_role},
    tables::*,
};

#[spacetimedb::reducer]
pub fn assign_menu_to_role(ctx: &ReducerContext, role_id: u64, menu_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_role(ctx, role_id)?;
    require_menu(ctx, menu_id)?;
    if let Some(mut link) = ctx
        .db
        .role_menu()
        .role_menu_by_pair()
        .filter((role_id, menu_id))
        .next()
    {
        if !link.is_deleted {
            return Err("角色已经拥有该菜单".into());
        }
        // 与 MySQL 保持一致：重新授权时恢复软删除记录。
        link.is_deleted = false;
        link.updated_at = Some(ctx.timestamp);
        ctx.db.role_menu().id().update(link);
        return Ok(());
    }
    ctx.db.role_menu().insert(RoleMenu {
        id: 0,
        role_id,
        menu_id,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

/// 使用原 MySQL 的软删除语义撤销角色菜单关系。
#[spacetimedb::reducer]
pub fn remove_menu_from_role(
    ctx: &ReducerContext,
    role_id: u64,
    menu_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_role(ctx, role_id)?;
    require_menu(ctx, menu_id)?;
    let mut link = ctx
        .db
        .role_menu()
        .role_menu_by_pair()
        .filter((role_id, menu_id))
        .next()
        .ok_or("角色尚未拥有该菜单")?;
    if link.is_deleted {
        return Err("角色菜单关系已经撤销".into());
    }
    link.is_deleted = true;
    link.updated_at = Some(ctx.timestamp);
    ctx.db.role_menu().id().update(link);
    Ok(())
}
