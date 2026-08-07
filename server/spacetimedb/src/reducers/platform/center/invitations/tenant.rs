//! 租户邀请码创建、加入和关系同步逻辑。

use spacetimedb::{ReducerContext, Table};

use super::common::{InvitationInput, ensure_usable, normalize_code, validated_input};
use crate::{
    reducers::{
        access::{
            current_center_user_id, require_center_user, require_customer, require_organization,
        },
        platform::center::organization::require_organization_owner,
    },
    tables::*,
};

fn validate_target_roles(
    ctx: &ReducerContext,
    customer_id: &str,
    role_ids: &[u64],
) -> Result<(), String> {
    for role_id in role_ids {
        ctx.db
            .role()
            .role_id()
            .find(*role_id)
            .filter(|role| role.customer_id == customer_id && role.status == 1)
            .ok_or_else(|| format!("目标租户角色不存在或已停用: {role_id}"))?;
    }
    Ok(())
}

fn target_organization(
    ctx: &ReducerContext,
    customer_id: &String,
) -> Result<(OrganizationTenantMapping, Organization), String> {
    let mapping = ctx
        .db
        .organization_tenant_mapping()
        .target_customer_id()
        .find(customer_id)
        .filter(|mapping| mapping.status == "active")
        .ok_or("目标租户尚未建立有效组织映射")?;
    let organization = require_organization(ctx, mapping.organization_id)?;
    Ok((mapping, organization))
}

#[spacetimedb::reducer]
pub fn create_tenant_invitation(
    ctx: &ReducerContext,
    customer_id: String,
    input: InvitationInput,
) -> Result<(), String> {
    let customer = require_customer(ctx, &customer_id)?;
    if customer.customer_id == "public" {
        return Err("公共租户不能创建加入邀请码".into());
    }
    let (_, organization) = target_organization(ctx, &customer_id)?;
    require_organization_owner(ctx, organization.id)?;
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let input = validated_input(ctx, input, false)?;
    validate_target_roles(ctx, &customer_id, &input.role_ids)?;
    if ctx
        .db
        .tenant_invitation()
        .tenant_invitation_by_code()
        .filter(input.code.as_str())
        .next()
        .is_some()
    {
        return Err("租户邀请码已经存在".into());
    }
    ctx.db.tenant_invitation().insert(TenantInvitation {
        id: 0,
        code: input.code,
        customer_id,
        created_by_center_user_id: center_user_id,
        role_ids: input.role_ids,
        max_uses: input.max_uses,
        used_count: 0,
        expires_at: input.expires_at,
        status: "active".into(),
        remark: input.remark,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

fn ensure_tenant_user(
    ctx: &ReducerContext,
    center_user: &CenterUser,
    customer: &Customer,
) -> Result<(SystemUser, bool), String> {
    if let Some(mapping) = ctx
        .db
        .user_tenant_mapping()
        .mapping_by_center_customer()
        .filter((center_user.id, customer.customer_id.as_str()))
        .next()
    {
        let user = ctx
            .db
            .system_user()
            .id()
            .find(mapping.customer_user_id)
            .filter(|user| user.customer_id == customer.customer_id && user.status == 1)
            .ok_or("当前账号在目标租户的映射异常")?;
        return Ok((user, true));
    }
    let existing_user = ctx
        .db
        .system_user()
        .business_user_by_customer_username()
        .filter((customer.customer_id.as_str(), center_user.username.as_str()))
        .next();
    let user = if let Some(user) = existing_user {
        if user.status != 1 {
            return Err("目标租户存在同名停用账号".into());
        }
        if let Some(mapping) = ctx
            .db
            .user_tenant_mapping()
            .mapping_by_customer_user()
            .filter((customer.customer_id.as_str(), user.id))
            .next()
            && mapping.center_user_id != center_user.id
        {
            return Err("目标租户同名账号已绑定其他中心用户".into());
        }
        user
    } else {
        ctx.db.system_user().insert(SystemUser {
            id: 0,
            username: center_user.username.clone(),
            customer_id: customer.customer_id.clone(),
            real_name: center_user.real_name.clone(),
            home_path: center_user.home_path.clone(),
            phone: center_user.phone.clone(),
            customer_type: Some(customer.customer_id.clone()),
            status: 1,
            token_version: 1,
            created_at: ctx.timestamp,
            updated_at: None,
            avatar_url: None,
        })
    };
    ctx.db.user_tenant_mapping().insert(UserTenantMapping {
        id: 0,
        center_user_id: center_user.id,
        customer_id: customer.customer_id.clone(),
        customer_user_id: user.id,
        db_name: customer.db_name.clone(),
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok((user, false))
}

#[spacetimedb::reducer]
pub fn join_tenant_by_invitation(ctx: &ReducerContext, code: String) -> Result<(), String> {
    let code = normalize_code(code)?;
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let mut center_user = require_center_user(ctx, center_user_id)?;
    if center_user.status != 1 {
        return Err("中心用户已被禁用".into());
    }
    let mut invitation = ctx
        .db
        .tenant_invitation()
        .tenant_invitation_by_code()
        .filter(code.as_str())
        .next()
        .ok_or("租户邀请码不存在")?;
    let invitation_id = invitation.id;
    let invitation_code = invitation.code.clone();
    ensure_usable(
        &invitation.status,
        invitation.expires_at,
        invitation.max_uses,
        invitation.used_count,
        ctx.timestamp,
    )?;
    let customer = require_customer(ctx, &invitation.customer_id)?;
    validate_target_roles(ctx, &customer.customer_id, &invitation.role_ids)?;
    let (_, organization) = target_organization(ctx, &customer.customer_id)?;
    let previous_customer_id = center_user.customer_type.clone();
    if previous_customer_id
        .as_deref()
        .is_some_and(|previous| previous != "public" && previous != customer.customer_id)
    {
        return Err("当前账号已属于其他组织空间，不能直接加入新租户".into());
    }
    let (tenant_user, already_joined) = ensure_tenant_user(ctx, &center_user, &customer)?;
    for role_id in invitation.role_ids.iter().copied() {
        if ctx
            .db
            .user_role()
            .user_role_by_pair()
            .filter((tenant_user.id, role_id))
            .next()
            .is_none()
        {
            ctx.db.user_role().insert(UserRole {
                id: 0,
                user_id: tenant_user.id,
                role_id,
                created_at: ctx.timestamp,
                updated_at: None,
            });
        }
    }
    if let Some(mut member) = ctx
        .db
        .organization_member()
        .member_by_pair()
        .filter((organization.id, center_user_id))
        .next()
    {
        member.status = "active".into();
        member.source_user_id = member.source_user_id.or(Some(tenant_user.id));
        member.joined_at = Some(ctx.timestamp);
        member.updated_at = Some(ctx.timestamp);
        ctx.db.organization_member().id().update(member);
    } else {
        ctx.db.organization_member().insert(OrganizationMember {
            id: 0,
            organization_id: organization.id,
            center_user_id,
            source_customer_id: customer.customer_id.clone(),
            source_user_id: Some(tenant_user.id),
            member_role: "member".into(),
            status: "active".into(),
            joined_at: Some(ctx.timestamp),
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    if !already_joined {
        center_user.customer_type = Some(customer.customer_id.clone());
        center_user.token_version = center_user.token_version.saturating_add(1);
        center_user.updated_at = Some(ctx.timestamp);
        ctx.db.center_user().id().update(center_user);
        let tokens = ctx
            .db
            .refresh_token()
            .refresh_token_by_user()
            .filter(center_user_id)
            .filter(|token| token.revoked_at.is_none())
            .collect::<Vec<_>>();
        for mut token in tokens {
            token.revoked_at = Some(ctx.timestamp);
            token.updated_at = Some(ctx.timestamp);
            ctx.db.refresh_token().id().update(token);
        }
        invitation.used_count = invitation.used_count.saturating_add(1);
        invitation.updated_at = Some(ctx.timestamp);
        ctx.db.tenant_invitation().id().update(invitation);
    }
    if let Some(mut log) = ctx
        .db
        .tenant_invitation_join_log()
        .tenant_join_log_by_invitation_user()
        .filter((invitation_id, center_user_id))
        .next()
    {
        log.customer_user_id = Some(tenant_user.id);
        log.previous_customer_id = previous_customer_id;
        log.status = "joined".into();
        log.error_message = None;
        log.joined_at = Some(ctx.timestamp);
        log.updated_at = Some(ctx.timestamp);
        ctx.db.tenant_invitation_join_log().id().update(log);
    } else {
        ctx.db
            .tenant_invitation_join_log()
            .insert(TenantInvitationJoinLog {
                id: 0,
                invitation_id,
                code: invitation_code,
                customer_id: customer.customer_id,
                center_user_id,
                customer_user_id: Some(tenant_user.id),
                previous_customer_id,
                status: "joined".into(),
                error_message: None,
                joined_at: Some(ctx.timestamp),
                created_at: ctx.timestamp,
                updated_at: None,
            });
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn revoke_tenant_invitation(ctx: &ReducerContext, invitation_id: u64) -> Result<(), String> {
    let mut invitation = ctx
        .db
        .tenant_invitation()
        .id()
        .find(invitation_id)
        .ok_or("租户邀请码不存在")?;
    let (_, organization) = target_organization(ctx, &invitation.customer_id)?;
    require_organization_owner(ctx, organization.id)?;
    if invitation.status != "active" {
        return Err("租户邀请码已经失效".into());
    }
    invitation.status = "revoked".into();
    invitation.updated_at = Some(ctx.timestamp);
    ctx.db.tenant_invitation().id().update(invitation);
    Ok(())
}
