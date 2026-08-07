//! 组织成员邀请码事务逻辑。

use spacetimedb::{ReducerContext, Table};

use super::common::{InvitationInput, ensure_usable, normalize_code, validated_input};
use crate::{
    reducers::{
        access::{current_center_user_id, require_center_user, require_organization},
        platform::center::organization::require_organization_owner,
    },
    tables::*,
};

fn validate_roles(ctx: &ReducerContext, customer_id: &str, role_ids: &[u64]) -> Result<(), String> {
    for role_id in role_ids {
        ctx.db
            .role()
            .role_id()
            .find(*role_id)
            .filter(|role| role.customer_id == customer_id && role.status == 1)
            .ok_or_else(|| format!("组织角色不存在或已停用: {role_id}"))?;
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn create_organization_invitation(
    ctx: &ReducerContext,
    organization_id: u64,
    input: InvitationInput,
) -> Result<(), String> {
    require_organization_owner(ctx, organization_id)?;
    let organization = require_organization(ctx, organization_id)?;
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let input = validated_input(ctx, input, true)?;
    validate_roles(ctx, &organization.source_customer_id, &input.role_ids)?;
    if ctx
        .db
        .organization_invitation()
        .organization_invitation_by_code()
        .filter(input.code.as_str())
        .next()
        .is_some()
    {
        return Err("组织邀请码已经存在".into());
    }
    ctx.db
        .organization_invitation()
        .insert(OrganizationInvitation {
            id: 0,
            organization_id,
            code: input.code,
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

#[spacetimedb::reducer]
pub fn join_organization_by_invitation(ctx: &ReducerContext, code: String) -> Result<(), String> {
    let code = normalize_code(code)?;
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let center_user = require_center_user(ctx, center_user_id)?;
    if center_user.status != 1 {
        return Err("中心用户已被禁用".into());
    }
    let mut invitation = ctx
        .db
        .organization_invitation()
        .organization_invitation_by_code()
        .filter(code.as_str())
        .next()
        .ok_or("组织邀请码不存在")?;
    ensure_usable(
        &invitation.status,
        invitation.expires_at,
        invitation.max_uses,
        invitation.used_count,
        ctx.timestamp,
    )?;
    let organization = require_organization(ctx, invitation.organization_id)?;
    validate_roles(ctx, &organization.source_customer_id, &invitation.role_ids)?;
    let existing_member = ctx
        .db
        .organization_member()
        .member_by_pair()
        .filter((organization.id, center_user_id))
        .next();
    let already_joined = existing_member
        .as_ref()
        .is_some_and(|member| member.status == "active");
    let source_user_id = ctx
        .db
        .user_tenant_mapping()
        .mapping_by_center_customer()
        .filter((center_user_id, organization.source_customer_id.as_str()))
        .next()
        .map(|mapping| mapping.customer_user_id);
    if let Some(mut member) = existing_member {
        member.status = "active".into();
        member.joined_at = Some(ctx.timestamp);
        member.source_user_id = member.source_user_id.or(source_user_id);
        member.updated_at = Some(ctx.timestamp);
        ctx.db.organization_member().id().update(member);
    } else {
        ctx.db.organization_member().insert(OrganizationMember {
            id: 0,
            organization_id: organization.id,
            center_user_id,
            source_customer_id: organization.source_customer_id.clone(),
            source_user_id,
            member_role: "member".into(),
            status: "active".into(),
            joined_at: Some(ctx.timestamp),
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    if let Some(source_user_id) = source_user_id {
        for role_id in invitation.role_ids.iter().copied() {
            if ctx
                .db
                .user_role()
                .user_role_by_pair()
                .filter((source_user_id, role_id))
                .next()
                .is_none()
            {
                ctx.db.user_role().insert(UserRole {
                    id: 0,
                    user_id: source_user_id,
                    role_id,
                    created_at: ctx.timestamp,
                    updated_at: None,
                });
            }
        }
    }
    if !already_joined {
        invitation.used_count = invitation.used_count.saturating_add(1);
        invitation.updated_at = Some(ctx.timestamp);
        ctx.db.organization_invitation().id().update(invitation);
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn revoke_organization_invitation(
    ctx: &ReducerContext,
    invitation_id: u64,
) -> Result<(), String> {
    let mut invitation = ctx
        .db
        .organization_invitation()
        .id()
        .find(invitation_id)
        .ok_or("组织邀请码不存在")?;
    require_organization_owner(ctx, invitation.organization_id)?;
    if invitation.status != "active" {
        return Err("组织邀请码已经失效".into());
    }
    invitation.status = "revoked".into();
    invitation.updated_at = Some(ctx.timestamp);
    ctx.db.organization_invitation().id().update(invitation);
    Ok(())
}
