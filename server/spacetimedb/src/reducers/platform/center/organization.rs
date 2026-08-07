//! 组织空间、成员关系和组织租户映射。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::{
            current_center_user_id, current_customer_id, require_center_user, require_customer,
            require_organization,
        },
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

#[spacetimedb::reducer]
pub fn create_organization(
    ctx: &ReducerContext,
    city: String,
    company_short_name: String,
) -> Result<(), String> {
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let source_customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let city = required_text(city, "所在城市不能为空")?;
    let company_short_name = required_text(company_short_name, "组织简称不能为空")?;
    if city.chars().count() > 30 || company_short_name.chars().count() > 50 {
        return Err("城市或组织简称超过长度限制".into());
    }
    let has_active_source = ctx
        .db
        .organization_member()
        .member_by_center_user()
        .filter(center_user_id)
        .filter(|member| member.status == "active")
        .filter_map(|member| ctx.db.organization().id().find(member.organization_id))
        .any(|organization| {
            organization.status == "active" && organization.source_customer_id == source_customer_id
        });
    if has_active_source {
        return Err("当前用户在该来源租户已拥有组织空间".into());
    }
    let organization = ctx.db.organization().insert(Organization {
        id: 0,
        name: format!("{city}{company_short_name}"),
        city: Some(city),
        company_short_name: Some(company_short_name),
        source_customer_id: source_customer_id.clone(),
        status: "active".into(),
        created_by_center_user_id: Some(center_user_id),
        legacy: false,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    ctx.db.organization_member().insert(OrganizationMember {
        id: 0,
        organization_id: organization.id,
        center_user_id,
        source_customer_id,
        source_user_id: None,
        member_role: "owner".into(),
        status: "active".into(),
        joined_at: Some(ctx.timestamp),
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn add_organization_member(
    ctx: &ReducerContext,
    organization_id: u64,
    center_user_id: u64,
    member_role: String,
) -> Result<(), String> {
    require_organization_owner(ctx, organization_id)?;
    require_center_user(ctx, center_user_id)?;
    let organization = require_organization(ctx, organization_id)?;
    let member_role = normalize_optional_text(Some(member_role)).unwrap_or_else(|| "member".into());
    if member_role != "owner" && member_role != "member" {
        return Err("组织成员角色无效".into());
    }
    if ctx
        .db
        .organization_member()
        .member_by_pair()
        .filter((organization_id, center_user_id))
        .next()
        .is_some()
    {
        return Err("用户已经是该组织成员".into());
    }
    ctx.db.organization_member().insert(OrganizationMember {
        id: 0,
        organization_id,
        center_user_id,
        source_customer_id: organization.source_customer_id,
        source_user_id: None,
        member_role,
        status: "active".into(),
        joined_at: Some(ctx.timestamp),
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn map_organization_to_tenant(
    ctx: &ReducerContext,
    organization_id: u64,
    target_customer_id: String,
) -> Result<(), String> {
    require_organization_owner(ctx, organization_id)?;
    let customer = require_customer(ctx, &target_customer_id)?;
    if ctx
        .db
        .organization_tenant_mapping()
        .target_customer_id()
        .find(&target_customer_id)
        .is_some()
    {
        return Err("目标租户已经绑定组织".into());
    }
    ctx.db
        .organization_tenant_mapping()
        .insert(OrganizationTenantMapping {
            id: 0,
            organization_id,
            target_customer_id,
            target_db_name: customer.db_name,
            tenant_provisioning_job_id: None,
            status: "active".into(),
            legacy: false,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    Ok(())
}

pub(crate) fn require_organization_owner(
    ctx: &ReducerContext,
    organization_id: u64,
) -> Result<(), String> {
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    require_organization(ctx, organization_id)?;
    ctx.db
        .organization_member()
        .member_by_pair()
        .filter((organization_id, center_user_id))
        .next()
        .filter(|member| member.status == "active" && member.member_role == "owner")
        .map(|_| ())
        .ok_or("需要组织所有者权限".into())
}
