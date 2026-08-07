//! 爬虫任务运行日志写入。

use spacetimedb::ReducerContext;

use super::super::common::limited_optional;
use super::common::{insert_task_log, require_crawler_task};
use crate::reducers::{
    access::AdminContext,
    validation::{required_text, validate_max_length},
};

#[spacetimedb::reducer]
pub fn append_crawler_task_log(
    ctx: &ReducerContext,
    task_id: u64,
    level: String,
    stage: String,
    message: String,
    detail_json: Option<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let task = require_crawler_task(ctx, task_id)?;
    let level = required_text(level, "日志等级不能为空")?;
    let stage = required_text(stage, "日志阶段不能为空")?;
    let message = required_text(message, "日志内容不能为空")?;
    if !["DEBUG", "ERROR", "INFO", "WARN"].contains(&level.as_str()) {
        return Err("日志等级无效".into());
    }
    validate_max_length(&level, 20, "日志等级不能超过20个字符")?;
    validate_max_length(&stage, 50, "日志阶段不能超过50个字符")?;
    validate_max_length(&message, 500, "日志内容不能超过500个字符")?;
    insert_task_log(
        ctx,
        &task,
        &level,
        &stage,
        message,
        limited_optional(detail_json, 65_535, "日志详情数据过长")?,
    );
    Ok(())
}
