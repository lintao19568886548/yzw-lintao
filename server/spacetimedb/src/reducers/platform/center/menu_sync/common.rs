//! 菜单模板同步事务的共用校验。

use spacetimedb::ReducerContext;

use crate::tables::*;

pub(super) fn require_menu_sync_job(
    ctx: &ReducerContext,
    job_id: u64,
) -> Result<MenuTemplateSyncJob, String> {
    ctx.db
        .menu_template_sync_job()
        .id()
        .find(job_id)
        .ok_or("菜单模板同步任务不存在".into())
}

pub(super) fn canonicalize_optional_json(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<String>, String> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let parsed: serde_json::Value =
        serde_json::from_str(value.trim()).map_err(|_| format!("{field_name}必须是有效 JSON"))?;
    serde_json::to_string(&parsed)
        .map(Some)
        .map_err(|_| format!("{field_name}无法序列化"))
}
