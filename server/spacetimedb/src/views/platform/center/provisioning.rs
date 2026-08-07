//! 当前中心用户可查看的租户开通任务及角色快照。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::{
    tables::*,
    views::{
        platform::center::organizations::my_organizations,
        shared::identity::{current_center_user, current_principal},
    },
};

#[spacetimedb::view(accessor = my_tenant_provisioning_jobs, public)]
pub fn my_tenant_provisioning_jobs(ctx: &ViewContext) -> Vec<TenantProvisioningJob> {
    let Some(user) = current_center_user(ctx) else {
        return vec![];
    };
    let is_admin = current_principal(ctx).is_some_and(|principal| principal.is_admin());
    let organization_ids = my_organizations(ctx)
        .into_iter()
        .map(|organization| organization.id)
        .collect::<BTreeSet<_>>();
    let mut rows = ctx
        .db
        .tenant_provisioning_job()
        .provisioning_job_by_scope()
        .filter(0u8)
        .filter(|job| {
            is_admin
                || job.initiator_center_user_id == user.id
                || job
                    .source_org_id
                    .is_some_and(|organization_id| organization_ids.contains(&organization_id))
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|job| job.id);
    rows.reverse();
    rows
}

#[spacetimedb::view(accessor = my_tenant_provisioning_role_snapshots, public)]
pub fn my_tenant_provisioning_role_snapshots(
    ctx: &ViewContext,
) -> Vec<TenantProvisioningRoleSnapshot> {
    let job_ids = my_tenant_provisioning_jobs(ctx)
        .into_iter()
        .map(|job| job.id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for job_id in job_ids {
        rows.extend(
            ctx.db
                .tenant_provisioning_role_snapshot()
                .provisioning_snapshot_by_job()
                .filter(job_id),
        );
    }
    rows.sort_by_key(|row| (row.job_id, row.id));
    rows
}
