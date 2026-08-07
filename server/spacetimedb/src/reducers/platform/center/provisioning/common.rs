//! 租户开通事务共用的任务与租约校验。

use spacetimedb::ReducerContext;

use crate::tables::*;

pub(super) fn require_provisioning_job(
    ctx: &ReducerContext,
    job_id: u64,
) -> Result<TenantProvisioningJob, String> {
    ctx.db
        .tenant_provisioning_job()
        .id()
        .find(job_id)
        .ok_or("租户开通任务不存在".into())
}

pub(super) fn require_active_lease(
    ctx: &ReducerContext,
    job_id: u64,
    lock_owner: &str,
) -> Result<TenantProvisioningJob, String> {
    let job = require_provisioning_job(ctx, job_id)?;
    if job.status != "provisioning" || job.lock_owner.as_deref() != Some(lock_owner) {
        return Err("租户开通任务租约已失效".into());
    }
    Ok(job)
}
