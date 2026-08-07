//! 当前中心用户可管理的组织与租户邀请码。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::{
    tables::*,
    views::platform::center::organizations::{my_organization_tenants, my_organizations},
};

#[spacetimedb::view(accessor = my_organization_invitations, public)]
pub fn my_organization_invitations(ctx: &ViewContext) -> Vec<OrganizationInvitation> {
    let mut rows = Vec::new();
    for organization in my_organizations(ctx) {
        rows.extend(
            ctx.db
                .organization_invitation()
                .organization_invitation_by_org()
                .filter(organization.id),
        );
    }
    rows.sort_by_key(|row| row.id);
    rows.reverse();
    rows
}

#[spacetimedb::view(accessor = my_tenant_invitations, public)]
pub fn my_tenant_invitations(ctx: &ViewContext) -> Vec<TenantInvitation> {
    let mut customer_ids = my_organization_tenants(ctx)
        .into_iter()
        .filter(|mapping| mapping.status == "active")
        .map(|mapping| mapping.target_customer_id)
        .collect::<Vec<_>>();
    customer_ids.sort();
    customer_ids.dedup();
    let mut rows = Vec::new();
    for customer_id in customer_ids {
        rows.extend(
            ctx.db
                .tenant_invitation()
                .tenant_invitation_by_customer()
                .filter(customer_id.as_str()),
        );
    }
    rows.sort_by_key(|row| row.id);
    rows.reverse();
    rows
}

#[spacetimedb::view(accessor = my_tenant_invitation_join_logs, public)]
pub fn my_tenant_invitation_join_logs(ctx: &ViewContext) -> Vec<TenantInvitationJoinLog> {
    let invitation_ids = my_tenant_invitations(ctx)
        .into_iter()
        .map(|invitation| invitation.id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for invitation_id in invitation_ids {
        rows.extend(
            ctx.db
                .tenant_invitation_join_log()
                .tenant_join_log_by_invitation()
                .filter(invitation_id),
        );
    }
    rows.sort_by_key(|row| (row.invitation_id, row.id));
    rows.reverse();
    rows
}
