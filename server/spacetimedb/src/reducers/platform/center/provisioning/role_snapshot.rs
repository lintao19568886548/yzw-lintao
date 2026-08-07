//! 租户开通过程中的角色复制和映射快照。

use spacetimedb::{ReducerContext, Table};

use super::common::require_active_lease;
use crate::{
    reducers::shared::access::{AdminContext, require_customer},
    tables::*,
};

#[spacetimedb::reducer]
pub fn copy_tenant_provisioning_role(
    ctx: &ReducerContext,
    job_id: u64,
    lock_owner: String,
    source_role_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let job = require_active_lease(ctx, job_id, lock_owner.trim())?;
    let source_org_id = job.source_org_id.ok_or("开通任务缺少源组织")?;
    let target_customer_id = job.target_customer_id.ok_or("开通任务缺少目标租户")?;
    require_customer(ctx, &target_customer_id)?;
    let source_role = ctx
        .db
        .role()
        .role_id()
        .find(source_role_id)
        .filter(|role| role.customer_id == job.source_customer_id && role.status == 1)
        .ok_or("源角色不存在或已停用")?;
    let target_role = ctx
        .db
        .role()
        .iter()
        .find(|role| {
            role.customer_id == target_customer_id
                && role.name == source_role.name
                && role.scope == source_role.scope
        })
        .unwrap_or_else(|| {
            ctx.db.role().insert(Role {
                role_id: 0,
                customer_id: target_customer_id,
                name: source_role.name.clone(),
                remark: source_role.remark.clone(),
                status: source_role.status,
                rates: source_role.rates,
                parent_id: None,
                reimbursement_auth: source_role.reimbursement_auth,
                organization_id: Some(source_org_id),
                scope: source_role.scope.clone(),
                created_at: ctx.timestamp,
                updated_at: None,
            })
        });
    if let Some(mut snapshot) = ctx
        .db
        .tenant_provisioning_role_snapshot()
        .provisioning_snapshot_by_job_source()
        .filter((job_id, source_role_id))
        .next()
    {
        snapshot.target_role_id = target_role.role_id;
        snapshot.role_name = Some(source_role.name);
        snapshot.updated_at = Some(ctx.timestamp);
        ctx.db
            .tenant_provisioning_role_snapshot()
            .id()
            .update(snapshot);
    } else {
        ctx.db
            .tenant_provisioning_role_snapshot()
            .insert(TenantProvisioningRoleSnapshot {
                id: 0,
                job_id,
                source_org_id,
                source_role_id,
                target_role_id: target_role.role_id,
                role_name: Some(source_role.name),
                created_at: ctx.timestamp,
                updated_at: None,
            });
    }
    Ok(())
}
