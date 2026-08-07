//! 用户和角色的园区授权。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::shared::access::{AdminContext, require_park, require_role, require_user},
    tables::*,
};

#[spacetimedb::reducer]
pub fn assign_park_to_user(ctx: &ReducerContext, user_id: u64, park_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_user(ctx, user_id)?;
    require_park(ctx, park_id)?;
    upsert_user_park(ctx, user_id, park_id)
}

#[spacetimedb::reducer]
pub fn assign_park_to_role(ctx: &ReducerContext, role_id: u64, park_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_role(ctx, role_id)?;
    require_park(ctx, park_id)?;
    if let Some(mut link) = ctx
        .db
        .role_park()
        .role_park_by_pair()
        .filter((role_id, park_id))
        .next()
    {
        if !link.is_deleted {
            return Err("角色已经关联该园区".into());
        }
        link.is_deleted = false;
        link.updated_at = Some(ctx.timestamp);
        ctx.db.role_park().id().update(link);
        return Ok(());
    }
    ctx.db.role_park().insert(RolePark {
        id: 0,
        role_id,
        park_id,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

fn upsert_user_park(ctx: &ReducerContext, user_id: u64, park_id: u64) -> Result<(), String> {
    if let Some(mut link) = ctx
        .db
        .user_park()
        .user_park_by_pair()
        .filter((user_id, park_id))
        .next()
    {
        if !link.is_deleted {
            return Err("用户已经关联该园区".into());
        }
        link.is_deleted = false;
        link.updated_at = Some(ctx.timestamp);
        ctx.db.user_park().id().update(link);
        return Ok(());
    }
    ctx.db.user_park().insert(UserPark {
        id: 0,
        user_id,
        park_id,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}
