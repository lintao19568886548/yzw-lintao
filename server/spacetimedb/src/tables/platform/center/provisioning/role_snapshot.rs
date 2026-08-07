//! 租户开通过程中的角色映射快照表。

use spacetimedb::Timestamp;

/// 对应中心库 `tenant_provisioning_role_snapshot` 表。
#[spacetimedb::table(
    accessor = tenant_provisioning_role_snapshot,
    index(accessor = provisioning_snapshot_by_job, btree(columns = [job_id])),
    index(accessor = provisioning_snapshot_by_job_source, btree(columns = [job_id, source_role_id])),
    index(accessor = provisioning_snapshot_by_source_org, btree(columns = [source_org_id])),
    index(accessor = provisioning_snapshot_by_source_role, btree(columns = [source_role_id])),
    index(accessor = provisioning_snapshot_by_target_role, btree(columns = [target_role_id]))
)]
pub struct TenantProvisioningRoleSnapshot {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub job_id: u64,
    pub source_org_id: u64,
    pub source_role_id: u64,
    pub target_role_id: u64,
    pub role_name: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
