//! 用户的创建、身份绑定与删除逻辑。

use spacetimedb::{Identity, ReducerContext, Table};

use crate::reducers::{
    access::{
        AdminContext, current_customer_id, current_user_id, require_center_user, require_user,
    },
    hr::relations::cleanup::delete_user_relations,
    validation::{normalize_optional_text, normalize_status, required_text},
};
use crate::tables::*;

#[spacetimedb::reducer]
pub fn create_user(
    ctx: &ReducerContext,
    username: String,
    real_name: String,
    phone: Option<String>,
    status: i8,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let username = required_text(username, "账号不能为空")?;
    let real_name = required_text(real_name, "姓名不能为空")?;
    if ctx
        .db
        .system_user()
        .business_user_by_customer_username()
        .filter((customer_id.as_str(), username.as_str()))
        .next()
        .is_some()
    {
        return Err("账号已存在".into());
    }
    ctx.db.system_user().insert(SystemUser {
        id: 0,
        username,
        customer_id,
        real_name,
        home_path: Some("/analytics".into()),
        phone: normalize_optional_text(phone),
        customer_type: None,
        status: normalize_status(status),
        token_version: 1,
        created_at: ctx.timestamp,
        updated_at: None,
        avatar_url: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn bind_user_identity(
    ctx: &ReducerContext,
    center_user_id: u64,
    identity: Identity,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_center_user(ctx, center_user_id)?;
    if ctx.db.user_identity().identity().find(identity).is_some() {
        return Err("该身份已绑定用户".into());
    }
    if ctx
        .db
        .user_identity()
        .center_user_id()
        .find(center_user_id)
        .is_some()
    {
        return Err("该中心用户已绑定身份".into());
    }
    ctx.db.user_identity().insert(UserIdentity {
        identity,
        center_user_id,
        created_at: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_user(ctx: &ReducerContext, user_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_user(ctx, user_id)?;
    if current_user_id(ctx) == Some(user_id) {
        return Err("不能删除当前登录用户".into());
    }
    // SpacetimeDB reducer 具有事务性，用户及其关系要么全部删除，要么全部回滚。
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    delete_user_relations(ctx, user_id, &customer_id);
    ctx.db.system_user().id().delete(user_id);
    Ok(())
}

/// 清理同一中心账号在历史租户中遗留的重复业务账号。
///
/// 只有当前租户的系统管理员可以执行；两个业务账号必须使用相同用户名、
/// 归属于同一个中心用户，并且被保留账号必须属于当前租户。
#[spacetimedb::reducer]
pub fn delete_cross_tenant_duplicate_user(
    ctx: &ReducerContext,
    retained_user_id: u64,
    duplicate_user_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    if retained_user_id == duplicate_user_id {
        return Err("保留账号和待删除账号不能相同".into());
    }
    let retained = require_user(ctx, retained_user_id)?;
    let duplicate = ctx
        .db
        .system_user()
        .id()
        .find(duplicate_user_id)
        .ok_or("待清理账号不存在")?;
    if duplicate.customer_id == retained.customer_id {
        return Err("同一租户内的账号请使用常规用户管理功能处理".into());
    }
    if duplicate.username != retained.username {
        return Err("两个账号的用户名不一致，拒绝清理".into());
    }

    let retained_mapping = ctx
        .db
        .user_tenant_mapping()
        .mapping_by_customer_user()
        .filter((retained.customer_id.as_str(), retained.id))
        .next()
        .ok_or("保留账号缺少中心账号映射")?;
    let duplicate_mapping = ctx
        .db
        .user_tenant_mapping()
        .mapping_by_customer_user()
        .filter((duplicate.customer_id.as_str(), duplicate.id))
        .next()
        .ok_or("待清理账号缺少中心账号映射")?;
    if retained_mapping.center_user_id != duplicate_mapping.center_user_id {
        return Err("两个业务账号不属于同一中心账号，拒绝清理".into());
    }

    let duplicate_customer_id = duplicate.customer_id.clone();
    delete_user_relations(ctx, duplicate.id, &duplicate_customer_id);
    ctx.db.system_user().id().delete(duplicate.id);
    Ok(())
}
