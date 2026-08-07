//! 爬虫抓取项创建、领取与结果回写。

use spacetimedb::{ReducerContext, SpacetimeType, Table, TimeDuration, Timestamp};

use super::super::common::limited_optional;
use super::common::{require_crawler_item, require_crawler_source, require_crawler_task};
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct CrawlerTaskItemInput {
    pub source_id: u64,
    pub source_ref_type: Option<String>,
    pub source_ref_id: Option<u64>,
    pub source_url: String,
    pub url_hash: String,
    pub max_retry_count: u32,
    pub published_at: Option<Timestamp>,
}

#[spacetimedb::reducer]
pub fn create_crawler_task_item(
    ctx: &ReducerContext,
    input: CrawlerTaskItemInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_crawler_source(ctx, input.source_id)?;
    let source_url = required_text(input.source_url, "抓取项地址不能为空")?;
    let url_hash = required_text(input.url_hash, "抓取项地址哈希不能为空")?;
    validate_max_length(&source_url, 800, "抓取项地址不能超过800个字符")?;
    validate_max_length(&url_hash, 80, "抓取项地址哈希不能超过80个字符")?;
    if input.max_retry_count == 0 {
        return Err("抓取项最大重试次数必须大于0".into());
    }
    if ctx
        .db
        .crawler_task_item()
        .crawler_task_item_by_source_hash()
        .filter((input.source_id, url_hash.as_str()))
        .next()
        .is_some()
    {
        return Err("数据源中已存在相同地址的抓取项".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.crawler_task_item().insert(CrawlerTaskItem {
        item_id: 0,
        customer_id,
        source_id: input.source_id,
        last_task_id: 0,
        source_ref_type: limited_optional(
            input.source_ref_type,
            50,
            "来源引用类型不能超过50个字符",
        )?,
        source_ref_id: input.source_ref_id,
        source_url,
        url_hash,
        status: "PENDING".into(),
        retry_count: 0,
        max_retry_count: input.max_retry_count,
        next_retry_at: None,
        last_http_status: None,
        last_error: None,
        skip_reason: None,
        published_at: input.published_at,
        last_started_at: None,
        last_finished_at: None,
        last_success_at: None,
        response_hash: None,
        parsed_payload_json: None,
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn claim_crawler_task_item(
    ctx: &ReducerContext,
    item_id: u64,
    task_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut item = require_crawler_item(ctx, item_id)?;
    let task = require_crawler_task(ctx, task_id)?;
    if task.status != "RUNNING" {
        return Err("抓取项只能由运行中的任务领取".into());
    }
    if task.source_id != item.source_id {
        return Err("抓取项与任务不属于同一数据源".into());
    }
    if !matches!(item.status.as_str(), "PENDING" | "RETRY_WAITING") {
        return Err("抓取项当前状态不允许领取".into());
    }
    if item.next_retry_at.is_some_and(|time| time > ctx.timestamp) {
        return Err("抓取项尚未到重试时间".into());
    }
    item.last_task_id = task_id;
    item.status = "RUNNING".into();
    item.last_started_at = Some(ctx.timestamp);
    item.next_retry_at = None;
    item.updated_at = ctx.timestamp;
    ctx.db.crawler_task_item().item_id().update(item);
    Ok(())
}

#[spacetimedb::reducer]
pub fn complete_crawler_task_item(
    ctx: &ReducerContext,
    item_id: u64,
    http_status: Option<i32>,
    response_hash: Option<String>,
    parsed_payload_json: Option<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut item = require_crawler_item(ctx, item_id)?;
    require_running_item(&item)?;
    item.status = "SUCCESS".into();
    item.last_http_status = http_status;
    item.response_hash = limited_optional(response_hash, 80, "响应哈希不能超过80个字符")?;
    item.parsed_payload_json = limited_optional(parsed_payload_json, 65_535, "解析结果数据过长")?;
    item.last_error = None;
    item.last_finished_at = Some(ctx.timestamp);
    item.last_success_at = Some(ctx.timestamp);
    item.updated_at = ctx.timestamp;
    ctx.db.crawler_task_item().item_id().update(item);
    Ok(())
}

#[spacetimedb::reducer]
pub fn fail_crawler_task_item(
    ctx: &ReducerContext,
    item_id: u64,
    error: String,
    http_status: Option<i32>,
    retry_delay_micros: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut item = require_crawler_item(ctx, item_id)?;
    require_running_item(&item)?;
    let error = required_text(error, "抓取失败原因不能为空")?;
    item.retry_count = item.retry_count.saturating_add(1);
    item.status = if item.retry_count >= item.max_retry_count {
        "FAILED".into()
    } else {
        "RETRY_WAITING".into()
    };
    item.next_retry_at = if item.status == "FAILED" {
        None
    } else {
        Some(ctx.timestamp + TimeDuration::from_micros(retry_delay_micros as i64))
    };
    item.last_error = Some(error);
    item.last_http_status = http_status;
    item.last_finished_at = Some(ctx.timestamp);
    item.updated_at = ctx.timestamp;
    ctx.db.crawler_task_item().item_id().update(item);
    Ok(())
}

#[spacetimedb::reducer]
pub fn skip_crawler_task_item(
    ctx: &ReducerContext,
    item_id: u64,
    reason: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut item = require_crawler_item(ctx, item_id)?;
    if !matches!(
        item.status.as_str(),
        "PENDING" | "RUNNING" | "RETRY_WAITING"
    ) {
        return Err("抓取项当前状态不允许跳过".into());
    }
    let reason = required_text(reason, "跳过原因不能为空")?;
    validate_max_length(&reason, 255, "跳过原因不能超过255个字符")?;
    item.status = "SKIPPED".into();
    item.skip_reason = Some(reason);
    item.next_retry_at = None;
    item.last_finished_at = Some(ctx.timestamp);
    item.updated_at = ctx.timestamp;
    ctx.db.crawler_task_item().item_id().update(item);
    Ok(())
}

fn require_running_item(item: &CrawlerTaskItem) -> Result<(), String> {
    (item.status == "RUNNING")
        .then_some(())
        .ok_or("只有运行中的抓取项可以回写结果".into())
}
