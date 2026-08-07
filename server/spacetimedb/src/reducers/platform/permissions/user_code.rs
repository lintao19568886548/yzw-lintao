//! 用户直接权限码授权。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_user},
        validation::required_text,
    },
    tables::*,
};

#[spacetimedb::reducer]
pub fn assign_code_to_user(ctx: &ReducerContext, user_id: u64, code: String) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_user(ctx, user_id)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let code = required_text(code, "用户权限码不能为空")?;
    if ctx
        .db
        .user_code()
        .user_code_by_pair()
        .filter((user_id, code.as_str()))
        .next()
        .is_some()
    {
        return Err("用户已经拥有该直接权限码".into());
    }
    ctx.db.user_code().insert(UserCode {
        id: 0,
        customer_id,
        user_id,
        code,
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_code_from_user(
    ctx: &ReducerContext,
    user_id: u64,
    code: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_user(ctx, user_id)?;
    let code = required_text(code, "用户权限码不能为空")?;
    let link = ctx
        .db
        .user_code()
        .user_code_by_pair()
        .filter((user_id, code.as_str()))
        .next()
        .ok_or("用户没有该直接权限码")?;
    ctx.db.user_code().id().delete(link.id);
    Ok(())
}
