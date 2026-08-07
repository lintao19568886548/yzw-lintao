//! 菜单模板同步目标日志写入逻辑。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::common::{canonicalize_optional_json, require_menu_sync_job};
use crate::{
    reducers::{
        access::{AdminContext, require_customer},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 外部菜单比对执行器完成单个租户后提交的确定性结果。
#[derive(SpacetimeType)]
pub struct MenuTemplateSyncLogInput {
    pub target_customer_id: String,
    pub status: String,
    pub summary_json: Option<String>,
    pub details_json: Option<String>,
    pub error_message: Option<String>,
}

#[spacetimedb::reducer]
pub fn record_menu_template_sync_log(
    ctx: &ReducerContext,
    job_id: u64,
    input: MenuTemplateSyncLogInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let job = require_menu_sync_job(ctx, job_id)?;
    if job.status != "running" {
        return Err("仅运行中的菜单同步任务可以写入日志".into());
    }
    let target_customer_id = required_text(input.target_customer_id, "目标租户标识不能为空")?;
    validate_max_length(&target_customer_id, 50, "目标租户标识不能超过 50 个字符")?;
    if target_customer_id == "public" || target_customer_id == job.source_customer_id {
        return Err("目标租户不能是公共租户或源租户".into());
    }
    if let Some(expected) = job.target_customer_id.as_deref()
        && expected != target_customer_id
    {
        return Err("目标租户不属于当前同步任务范围".into());
    }
    let customer = require_customer(ctx, &target_customer_id)?;
    if ctx
        .db
        .menu_template_sync_log()
        .menu_sync_log_by_job()
        .filter(job_id)
        .any(|row| row.target_customer_id == target_customer_id)
    {
        return Err("当前任务已记录该目标租户".into());
    }

    let status = required_text(input.status, "同步日志状态不能为空")?;
    let allowed = if job.mode == "dry_run" {
        matches!(status.as_str(), "ready" | "blocked")
    } else {
        matches!(status.as_str(), "completed" | "blocked" | "failed")
    };
    if !allowed {
        return Err("同步日志状态与任务模式不匹配".into());
    }
    let summary_json = canonicalize_optional_json(input.summary_json, "同步摘要")?;
    let details_json = canonicalize_optional_json(input.details_json, "同步明细")?;
    let error_message = normalize_optional_text(input.error_message)
        .map(|value| value.chars().take(4_000).collect());

    ctx.db.menu_template_sync_log().insert(MenuTemplateSyncLog {
        id: 0,
        job_id,
        target_customer_id,
        target_db_name: customer.db_name,
        status,
        summary_json,
        details_json,
        error_message,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}
