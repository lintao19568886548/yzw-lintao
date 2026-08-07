//! 菜单模板同步任务状态机。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::common::require_menu_sync_job;
use crate::{
    reducers::{
        access::{AdminContext, require_customer},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 创建同步任务时由管理端提供的范围。
#[derive(SpacetimeType)]
pub struct MenuTemplateSyncJobInput {
    pub mode: String,
    pub source_customer_id: String,
    pub all_tenants: bool,
    pub target_customer_id: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_menu_template_sync_job(
    ctx: &ReducerContext,
    input: MenuTemplateSyncJobInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mode = required_text(input.mode, "同步模式不能为空")?;
    if !matches!(mode.as_str(), "dry_run" | "execute") {
        return Err("同步模式仅支持 dry_run 或 execute".into());
    }
    let source_customer_id = required_text(input.source_customer_id, "源租户标识不能为空")?;
    validate_max_length(&source_customer_id, 50, "源租户标识不能超过 50 个字符")?;
    require_customer(ctx, &source_customer_id)?;

    let target_customer_id = normalize_optional_text(input.target_customer_id);
    if input.all_tenants == target_customer_id.is_some() {
        return Err("必须且只能选择全部租户或一个目标租户".into());
    }
    let (target_scope, target_customer_id) = if input.all_tenants {
        ("all_tenants".to_string(), None)
    } else {
        let target_customer_id = target_customer_id.expect("已校验目标租户存在");
        validate_max_length(&target_customer_id, 50, "目标租户标识不能超过 50 个字符")?;
        if target_customer_id == "public" || target_customer_id == source_customer_id {
            return Err("目标租户不能是公共租户或源租户".into());
        }
        require_customer(ctx, &target_customer_id)?;
        (target_customer_id.clone(), Some(target_customer_id))
    };

    ctx.db.menu_template_sync_job().insert(MenuTemplateSyncJob {
        id: 0,
        center_scope: 0,
        mode,
        source_customer_id,
        target_scope,
        target_customer_id,
        status: "running".into(),
        summary_json: None,
        error_message: None,
        started_at: Some(ctx.timestamp),
        completed_at: None,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn finish_menu_template_sync_job(ctx: &ReducerContext, job_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut job = require_menu_sync_job(ctx, job_id)?;
    if job.status != "running" {
        return Err("仅运行中的菜单同步任务可以结束".into());
    }
    let logs = ctx
        .db
        .menu_template_sync_log()
        .menu_sync_log_by_job()
        .filter(job_id)
        .collect::<Vec<_>>();
    if logs.is_empty() {
        return Err("菜单同步任务至少需要一条目标日志".into());
    }
    let ready = logs.iter().filter(|row| row.status == "ready").count();
    let completed = logs.iter().filter(|row| row.status == "completed").count();
    let blocked = logs.iter().filter(|row| row.status == "blocked").count();
    let failed = logs.iter().filter(|row| row.status == "failed").count();
    job.status = if job.mode == "dry_run" {
        if blocked > 0 || failed > 0 {
            "blocked"
        } else {
            "ready"
        }
    } else if blocked > 0 || failed > 0 {
        "failed"
    } else {
        "completed"
    }
    .into();
    job.summary_json = Some(
        serde_json::json!({
            "total": logs.len(),
            "ready": ready,
            "completed": completed,
            "blocked": blocked,
            "failed": failed,
        })
        .to_string(),
    );
    job.completed_at = Some(ctx.timestamp);
    job.updated_at = Some(ctx.timestamp);
    ctx.db.menu_template_sync_job().id().update(job);
    Ok(())
}

#[spacetimedb::reducer]
pub fn fail_menu_template_sync_job(
    ctx: &ReducerContext,
    job_id: u64,
    error_message: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut job = require_menu_sync_job(ctx, job_id)?;
    if job.status != "running" {
        return Err("仅运行中的菜单同步任务可以标记失败".into());
    }
    let error_message = required_text(error_message, "失败原因不能为空")?;
    job.status = "failed".into();
    job.error_message = Some(error_message.chars().take(4_000).collect());
    job.completed_at = Some(ctx.timestamp);
    job.updated_at = Some(ctx.timestamp);
    ctx.db.menu_template_sync_job().id().update(job);
    Ok(())
}
