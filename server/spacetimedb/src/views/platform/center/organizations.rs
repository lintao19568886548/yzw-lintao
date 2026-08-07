//! 当前中心用户所属组织及其租户映射。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_center_user_id};

#[spacetimedb::view(accessor = my_organizations, public)]
pub fn my_organizations(ctx: &ViewContext) -> Vec<Organization> {
    let Some(center_user_id) = current_center_user_id(ctx) else {
        return vec![];
    };
    let mut organizations = ctx
        .db
        .organization_member()
        .member_by_center_user()
        .filter(center_user_id)
        .filter(|member| member.status == "active")
        .filter_map(|member| ctx.db.organization().id().find(member.organization_id))
        .filter(|organization| organization.status == "active")
        .collect::<Vec<_>>();
    organizations.sort_by_key(|organization| organization.id);
    organizations.dedup_by_key(|organization| organization.id);
    organizations
}

#[spacetimedb::view(accessor = my_organization_tenants, public)]
pub fn my_organization_tenants(ctx: &ViewContext) -> Vec<OrganizationTenantMapping> {
    let mut mappings = Vec::new();
    for organization in my_organizations(ctx) {
        mappings.extend(
            ctx.db
                .organization_tenant_mapping()
                .organization_tenant_by_org()
                .filter(organization.id)
                .filter(|mapping| mapping.status == "active"),
        );
    }
    mappings.sort_by_key(|mapping| mapping.id);
    mappings.dedup_by_key(|mapping| mapping.id);
    mappings
}
