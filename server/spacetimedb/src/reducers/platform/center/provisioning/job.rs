//! 租户开通任务状态机。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::common::{require_active_lease, require_provisioning_job};
use crate::{
    reducers::{
        access::{AdminContext, current_center_user_id, require_customer, require_organization},
        platform::center::organization::require_organization_owner,
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

const DEFAULT_MAX_RETRY: u32 = 3;
const STALE_LEASE_MICROS: i64 = 5 * 60 * 1_000_000;

/// 支付或管理流程创建开通任务时提供的目标信息。
#[derive(SpacetimeType)]
pub struct TenantProvisioningJobInput {
    pub target_customer_id: Option<String>,
    pub target_city: Option<String>,
    pub target_company_short_name: Option<String>,
    pub target_db_name: Option<String>,
    pub last_payment_out_trade_no: Option<String>,
}

#[pure_function::pure]
fn normalize_limited(
    value: Option<String>,
    max_chars: usize,
    message: &'static str,
) -> Result<Option<String>, String> {
    let value = normalize_optional_text(value);
    if let Some(value) = value.as_deref() {
        validate_max_length(value, max_chars, message)?;
    }
    Ok(value)
}

#[spacetimedb::reducer]
pub fn create_tenant_provisioning_job(
    ctx: &ReducerContext,
    source_org_id: u64,
    input: TenantProvisioningJobInput,
) -> Result<(), String> {
    require_organization_owner(ctx, source_org_id)?;
    let organization = require_organization(ctx, source_org_id)?;
    let initiator_center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let target_customer_id = normalize_limited(
        input.target_customer_id,
        50,
        "目标租户标识不能超过 50 个字符",
    )?;
    let target_city = normalize_limited(input.target_city, 30, "目标城市不能超过 30 个字符")?;
    let target_company_short_name = normalize_limited(
        input.target_company_short_name,
        50,
        "目标公司简称不能超过 50 个字符",
    )?;
    let target_db_name = normalize_limited(
        input.target_db_name,
        100,
        "目标数据库名称不能超过 100 个字符",
    )?;
    let last_payment_out_trade_no = normalize_limited(
        input.last_payment_out_trade_no,
        64,
        "支付订单号不能超过 64 个字符",
    )?;
    if let Some(target_customer_id) = target_customer_id.as_deref()
        && ctx.db.tenant_provisioning_job().iter().any(|job| {
            job.target_customer_id.as_deref() == Some(target_customer_id)
                && job.status != "cancelled"
        })
    {
        return Err("目标租户已经存在开通任务".into());
    }
    ctx.db
        .tenant_provisioning_job()
        .insert(TenantProvisioningJob {
            id: 0,
            center_scope: 0,
            initiator_center_user_id,
            source_org_id: Some(source_org_id),
            source_customer_id: organization.source_customer_id,
            target_customer_id,
            target_city,
            target_company_short_name,
            target_db_name,
            status: "pending".into(),
            step: Some("queued".into()),
            retry_count: 0,
            last_payment_out_trade_no,
            error_message: None,
            lock_owner: None,
            locked_at: None,
            heartbeat_at: None,
            started_at: None,
            completed_at: None,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    Ok(())
}

#[spacetimedb::reducer]
pub fn claim_tenant_provisioning_job(
    ctx: &ReducerContext,
    job_id: u64,
    lock_owner: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let lock_owner = required_text(lock_owner, "任务锁所有者不能为空")?;
    validate_max_length(&lock_owner, 128, "任务锁所有者不能超过 128 个字符")?;
    let mut job = require_provisioning_job(ctx, job_id)?;
    let heartbeat_micros = job
        .heartbeat_at
        .map(|time| time.to_micros_since_unix_epoch())
        .unwrap_or(i64::MIN);
    let stale_before = ctx
        .timestamp
        .to_micros_since_unix_epoch()
        .saturating_sub(STALE_LEASE_MICROS);
    let claimable = matches!(job.status.as_str(), "pending" | "failed_retryable")
        || (job.status == "provisioning" && heartbeat_micros < stale_before);
    if !claimable || job.retry_count >= DEFAULT_MAX_RETRY {
        return Err("租户开通任务当前不可认领".into());
    }
    job.status = "provisioning".into();
    job.step = Some("claimed".into());
    job.error_message = None;
    job.lock_owner = Some(lock_owner);
    job.locked_at = Some(ctx.timestamp);
    job.heartbeat_at = Some(ctx.timestamp);
    job.started_at = Some(ctx.timestamp);
    job.updated_at = Some(ctx.timestamp);
    ctx.db.tenant_provisioning_job().id().update(job);
    Ok(())
}

#[spacetimedb::reducer]
pub fn heartbeat_tenant_provisioning_job(
    ctx: &ReducerContext,
    job_id: u64,
    lock_owner: String,
    step: Option<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let lock_owner = required_text(lock_owner, "任务锁所有者不能为空")?;
    let mut job = require_active_lease(ctx, job_id, &lock_owner)?;
    if let Some(step) = normalize_optional_text(step) {
        validate_max_length(&step, 64, "任务步骤不能超过 64 个字符")?;
        job.step = Some(step);
    }
    job.heartbeat_at = Some(ctx.timestamp);
    job.updated_at = Some(ctx.timestamp);
    ctx.db.tenant_provisioning_job().id().update(job);
    Ok(())
}

#[spacetimedb::reducer]
pub fn fail_tenant_provisioning_job(
    ctx: &ReducerContext,
    job_id: u64,
    lock_owner: String,
    error_message: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let lock_owner = required_text(lock_owner, "任务锁所有者不能为空")?;
    let mut job = require_active_lease(ctx, job_id, &lock_owner)?;
    let error_message = required_text(error_message, "失败原因不能为空")?;
    job.retry_count = job.retry_count.saturating_add(1);
    let failed_manual = job.retry_count >= DEFAULT_MAX_RETRY;
    job.status = if failed_manual {
        "failed_manual"
    } else {
        "failed_retryable"
    }
    .into();
    job.step = Some(
        if failed_manual {
            "failed"
        } else {
            "retry_waiting"
        }
        .into(),
    );
    job.error_message = Some(error_message.chars().take(4_000).collect());
    job.lock_owner = None;
    job.locked_at = None;
    job.heartbeat_at = None;
    job.updated_at = Some(ctx.timestamp);
    ctx.db.tenant_provisioning_job().id().update(job);
    Ok(())
}

#[spacetimedb::reducer]
pub fn complete_tenant_provisioning_job(
    ctx: &ReducerContext,
    job_id: u64,
    lock_owner: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let lock_owner = required_text(lock_owner, "任务锁所有者不能为空")?;
    let mut job = require_active_lease(ctx, job_id, &lock_owner)?;
    let source_org_id = job.source_org_id.ok_or("开通任务缺少源组织")?;
    require_organization(ctx, source_org_id)?;
    let target_customer_id = job
        .target_customer_id
        .clone()
        .ok_or("开通任务缺少目标租户")?;
    let customer = require_customer(ctx, &target_customer_id)?;
    if let Some(mut mapping) = ctx
        .db
        .organization_tenant_mapping()
        .target_customer_id()
        .find(&target_customer_id)
    {
        if mapping.organization_id != source_org_id {
            return Err("目标租户已经绑定其他组织".into());
        }
        mapping.target_db_name = job.target_db_name.clone().or(customer.db_name);
        mapping.tenant_provisioning_job_id = Some(job_id);
        mapping.status = "active".into();
        mapping.updated_at = Some(ctx.timestamp);
        ctx.db.organization_tenant_mapping().id().update(mapping);
    } else {
        ctx.db
            .organization_tenant_mapping()
            .insert(OrganizationTenantMapping {
                id: 0,
                organization_id: source_org_id,
                target_customer_id,
                target_db_name: job.target_db_name.clone().or(customer.db_name),
                tenant_provisioning_job_id: Some(job_id),
                status: "active".into(),
                legacy: false,
                created_at: ctx.timestamp,
                updated_at: None,
            });
    }
    job.status = "active".into();
    job.step = Some("completed".into());
    job.error_message = None;
    job.lock_owner = None;
    job.locked_at = None;
    job.heartbeat_at = None;
    job.completed_at = Some(ctx.timestamp);
    job.updated_at = Some(ctx.timestamp);
    ctx.db.tenant_provisioning_job().id().update(job);
    Ok(())
}

#[spacetimedb::reducer]
pub fn requeue_tenant_provisioning_job(ctx: &ReducerContext, job_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut job = require_provisioning_job(ctx, job_id)?;
    if job.status != "failed_manual" {
        return Err("仅允许重排人工处理状态的任务".into());
    }
    job.status = "pending".into();
    job.step = Some("manual_requeued".into());
    job.retry_count = 0;
    job.error_message = None;
    job.lock_owner = None;
    job.locked_at = None;
    job.heartbeat_at = None;
    job.started_at = None;
    job.completed_at = None;
    job.updated_at = Some(ctx.timestamp);
    ctx.db.tenant_provisioning_job().id().update(job);
    Ok(())
}
