//! 用户角色授权。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::shared::access::{AdminContext, require_role, require_user},
    tables::*,
};

#[spacetimedb::reducer]
pub fn assign_role_to_user(ctx: &ReducerContext, user_id: u64, role_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_user(ctx, user_id)?;
    require_role(ctx, role_id)?;
    // MySQL 没有组合唯一约束，因此在 reducer 中显式防止重复关系。
    if ctx
        .db
        .user_role()
        .user_role_by_pair()
        .filter((user_id, role_id))
        .next()
        .is_some()
    {
        return Err("用户已经拥有该角色".into());
    }
    ctx.db.user_role().insert(UserRole {
        id: 0,
        user_id,
        role_id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}
