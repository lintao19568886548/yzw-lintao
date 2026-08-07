//! 组织租户开通任务表。

use spacetimedb::Timestamp;

/// 对应中心库 `tenant_provisioning_job` 表。
#[spacetimedb::table(
    accessor = tenant_provisioning_job,
    index(accessor = provisioning_job_by_scope, btree(columns = [center_scope])),
    index(accessor = provisioning_job_by_initiator, btree(columns = [initiator_center_user_id])),
    index(accessor = provisioning_job_by_source_org, btree(columns = [source_org_id])),
    index(accessor = provisioning_job_by_source_customer, btree(columns = [source_customer_id])),
    index(accessor = provisioning_job_by_target_customer, btree(columns = [target_customer_id])),
    index(accessor = provisioning_job_by_status, btree(columns = [status])),
    index(accessor = provisioning_job_by_status_heartbeat, btree(columns = [status, heartbeat_at])),
    index(accessor = provisioning_job_by_lock_owner, btree(columns = [lock_owner])),
    index(accessor = provisioning_job_by_target_db, btree(columns = [target_db_name]))
)]
pub struct TenantProvisioningJob {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    /// 中心库全局分区标识，固定为 `0`。
    pub center_scope: u8,
    pub initiator_center_user_id: u64,
    pub source_org_id: Option<u64>,
    pub source_customer_id: String,
    pub target_customer_id: Option<String>,
    pub target_city: Option<String>,
    pub target_company_short_name: Option<String>,
    pub target_db_name: Option<String>,
    pub status: String,
    pub step: Option<String>,
    pub retry_count: u32,
    pub last_payment_out_trade_no: Option<String>,
    pub error_message: Option<String>,
    pub lock_owner: Option<String>,
    pub locked_at: Option<Timestamp>,
    pub heartbeat_at: Option<Timestamp>,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
