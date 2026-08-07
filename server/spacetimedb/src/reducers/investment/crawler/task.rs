//! 爬虫任务创建与状态流转。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::super::common::limited_optional;
use super::common::{insert_task_log, require_crawler_source, require_crawler_task};
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct CrawlerTaskResultInput {
    pub status: String,
    pub fetched_count: u64,
    pub created_lead_count: u64,
    pub updated_lead_count: u64,
    pub skipped_count: u64,
    pub error_message: Option<String>,
    pub skip_reason: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_crawler_task(
    ctx: &ReducerContext,
    source_id: u64,
    task_type: String,
    max_retry_count: u32,
    request_config_json: Option<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let source = require_crawler_source(ctx, source_id)?;
    if !source.enabled {
        return Err("爬虫数据源已停用".into());
    }
    if ctx
        .db
        .crawler_task()
        .crawler_task_by_source()
        .filter(source_id)
        .any(|task| matches!(task.status.as_str(), "PENDING" | "RUNNING"))
    {
        return Err("同一数据源已有等待或运行中的任务".into());
    }
    let task_type = required_text(task_type, "爬虫任务类型不能为空")?;
    validate_max_length(&task_type, 50, "爬虫任务类型不能超过50个字符")?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let task = ctx.db.crawler_task().insert(CrawlerTask {
        task_id: 0,
        customer_id,
        source_id,
        task_type,
        status: "PENDING".into(),
        started_at: None,
        finished_at: None,
        crawl_started_at: None,
        crawl_ended_at: None,
        fetched_count: 0,
        created_lead_count: 0,
        updated_lead_count: 0,
        skipped_count: 0,
        error_message: None,
        retry_count: 0,
        max_retry_count,
        next_retry_at: None,
        skip_reason: None,
        request_config_json: limited_optional(request_config_json, 65_535, "任务配置数据过长")?,
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    });
    insert_task_log(
        ctx,
        &task,
        "INFO",
        "TASK_CREATED",
        "爬虫任务已创建".into(),
        None,
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn start_crawler_task(ctx: &ReducerContext, task_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut task = require_crawler_task(ctx, task_id)?;
    if task.status != "PENDING" && task.status != "RETRY_WAITING" {
        return Err("只有等待或等待重试的任务可以启动".into());
    }
    task.status = "RUNNING".into();
    task.started_at.get_or_insert(ctx.timestamp);
    task.crawl_started_at = Some(ctx.timestamp);
    task.next_retry_at = None;
    task.updated_at = ctx.timestamp;
    insert_task_log(
        ctx,
        &task,
        "INFO",
        "TASK_STARTED",
        "爬虫任务开始运行".into(),
        None,
    );
    ctx.db.crawler_task().task_id().update(task);
    Ok(())
}

#[spacetimedb::reducer]
pub fn finish_crawler_task(
    ctx: &ReducerContext,
    task_id: u64,
    input: CrawlerTaskResultInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut task = require_crawler_task(ctx, task_id)?;
    if task.status != "RUNNING" {
        return Err("只有运行中的任务可以结束".into());
    }
    if !["FAILED", "SUCCESS"].contains(&input.status.as_str()) {
        return Err("任务结束状态只能是FAILED或SUCCESS".into());
    }
    task.status = input.status;
    task.fetched_count = input.fetched_count;
    task.created_lead_count = input.created_lead_count;
    task.updated_lead_count = input.updated_lead_count;
    task.skipped_count = input.skipped_count;
    task.error_message = limited_optional(input.error_message, 65_535, "任务错误信息过长")?;
    task.skip_reason = limited_optional(input.skip_reason, 255, "任务跳过原因不能超过255个字符")?;
    task.finished_at = Some(ctx.timestamp);
    task.crawl_ended_at = Some(ctx.timestamp);
    task.updated_at = ctx.timestamp;
    let level = if task.status == "SUCCESS" {
        "INFO"
    } else {
        "ERROR"
    };
    insert_task_log(
        ctx,
        &task,
        level,
        "TASK_FINISHED",
        format!("爬虫任务结束：{}", task.status),
        None,
    );
    let source_id = task.source_id;
    ctx.db.crawler_task().task_id().update(task);
    if let Some(mut source) = ctx.db.crawler_source().source_id().find(source_id) {
        source.last_crawled_at = Some(ctx.timestamp);
        source.updated_at = ctx.timestamp;
        ctx.db.crawler_source().source_id().update(source);
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn cancel_crawler_task(ctx: &ReducerContext, task_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut task = require_crawler_task(ctx, task_id)?;
    if task.status != "PENDING" {
        return Err("只有等待中的任务可以取消".into());
    }
    task.status = "CANCELED".into();
    task.finished_at = Some(ctx.timestamp);
    task.updated_at = ctx.timestamp;
    insert_task_log(
        ctx,
        &task,
        "WARN",
        "TASK_CANCELED",
        "爬虫任务已取消".into(),
        None,
    );
    ctx.db.crawler_task().task_id().update(task);
    Ok(())
}
