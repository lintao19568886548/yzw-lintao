//! 中心用户、租户业务用户映射和当前租户切换。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_center_user_id, require_center_user, require_customer},
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

#[spacetimedb::reducer]
pub fn create_center_user(
    ctx: &ReducerContext,
    username: String,
    real_name: String,
    phone: Option<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let username = required_text(username, "中心账号不能为空")?;
    let real_name = required_text(real_name, "姓名不能为空")?;
    if ctx.db.center_user().username().find(&username).is_some() {
        return Err("中心账号已存在".into());
    }
    ctx.db.center_user().insert(CenterUser {
        id: 0,
        username,
        real_name,
        customer_type: None,
        status: 1,
        token_version: 1,
        phone: normalize_optional_text(phone),
        home_path: Some("/analytics".into()),
        membership_trial_start_at: None,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

/// 在目标租户创建业务用户，并在同一事务中建立中心用户映射。
#[spacetimedb::reducer]
pub fn provision_tenant_user(
    ctx: &ReducerContext,
    center_user_id: u64,
    customer_id: String,
    username: String,
    real_name: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_center_user(ctx, center_user_id)?;
    let customer = require_customer(ctx, &customer_id)?;
    let username = required_text(username, "租户账号不能为空")?;
    let real_name = required_text(real_name, "姓名不能为空")?;
    if ctx
        .db
        .user_tenant_mapping()
        .mapping_by_center_customer()
        .filter((center_user_id, customer_id.as_str()))
        .next()
        .is_some()
    {
        return Err("中心用户已经加入该租户".into());
    }
    if ctx
        .db
        .system_user()
        .business_user_by_customer_username()
        .filter((customer_id.as_str(), username.as_str()))
        .next()
        .is_some()
    {
        return Err("目标租户存在同名账号".into());
    }
    let business_user = ctx.db.system_user().insert(SystemUser {
        id: 0,
        username,
        customer_id: customer_id.clone(),
        real_name,
        home_path: Some("/analytics".into()),
        phone: None,
        customer_type: Some(customer_id.clone()),
        status: 1,
        token_version: 1,
        created_at: ctx.timestamp,
        updated_at: None,
        avatar_url: None,
    });
    ctx.db.user_tenant_mapping().insert(UserTenantMapping {
        id: 0,
        center_user_id,
        customer_id,
        customer_user_id: business_user.id,
        db_name: customer.db_name,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

/// 建立中心用户到某租户业务用户的双向唯一映射。
#[spacetimedb::reducer]
pub fn map_user_to_tenant(
    ctx: &ReducerContext,
    center_user_id: u64,
    customer_id: String,
    customer_user_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_center_user(ctx, center_user_id)?;
    let customer = require_customer(ctx, &customer_id)?;
    let business_user = ctx
        .db
        .system_user()
        .id()
        .find(customer_user_id)
        .filter(|user| user.customer_id == customer_id)
        .ok_or("租户业务用户不存在")?;

    if let Some(existing) = ctx
        .db
        .user_tenant_mapping()
        .mapping_by_center_customer()
        .filter((center_user_id, customer_id.as_str()))
        .next()
    {
        if existing.customer_user_id == business_user.id {
            return Ok(());
        }
        return Err("中心用户在该租户已映射其他业务用户".into());
    }
    if let Some(existing) = ctx
        .db
        .user_tenant_mapping()
        .mapping_by_customer_user()
        .filter((customer_id.as_str(), customer_user_id))
        .next()
        && existing.center_user_id != center_user_id
    {
        return Err("租户业务用户已绑定其他中心用户".into());
    }
    ctx.db.user_tenant_mapping().insert(UserTenantMapping {
        id: 0,
        center_user_id,
        customer_id,
        customer_user_id,
        db_name: customer.db_name,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

/// 切换中心用户的当前租户；只有已建立映射的租户才能切换。
#[spacetimedb::reducer]
pub fn switch_current_tenant(ctx: &ReducerContext, customer_id: String) -> Result<(), String> {
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    require_customer(ctx, &customer_id)?;
    if ctx
        .db
        .user_tenant_mapping()
        .mapping_by_center_customer()
        .filter((center_user_id, customer_id.as_str()))
        .next()
        .is_none()
    {
        return Err("当前用户尚未加入该租户".into());
    }
    let mut center_user = require_center_user(ctx, center_user_id)?;
    center_user.customer_type = Some(customer_id);
    center_user.updated_at = Some(ctx.timestamp);
    ctx.db.center_user().id().update(center_user);
    Ok(())
}
