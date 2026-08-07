//! 爬虫数据源创建、更新与停用。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::super::common::limited_optional;
use super::common::require_crawler_source;
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct CrawlerSourceInput {
    pub source_code: String,
    pub source_name: String,
    pub source_type: String,
    pub base_url: String,
    pub robots_url: Option<String>,
    pub enabled: bool,
    pub crawl_interval_minutes: u32,
    pub rate_limit_per_minute: u32,
    pub allowed_paths: Vec<String>,
    pub blocked_paths: Vec<String>,
    pub keyword_include: Vec<String>,
    pub keyword_exclude: Vec<String>,
    pub region_scope: Vec<String>,
}

#[spacetimedb::reducer]
pub fn create_crawler_source(
    ctx: &ReducerContext,
    input: CrawlerSourceInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_source(ctx, 0, customer_id.clone(), input)?;
    if ctx
        .db
        .crawler_source()
        .crawler_source_by_customer_code()
        .filter((customer_id.as_str(), row.source_code.as_str()))
        .any(|source| !source.is_deleted)
    {
        return Err("爬虫数据源编码已存在".into());
    }
    ctx.db.crawler_source().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_crawler_source(
    ctx: &ReducerContext,
    source_id: u64,
    input: CrawlerSourceInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_crawler_source(ctx, source_id)?;
    let mut row = validated_source(ctx, source_id, existing.customer_id.clone(), input)?;
    if ctx
        .db
        .crawler_source()
        .crawler_source_by_customer_code()
        .filter((existing.customer_id.as_str(), row.source_code.as_str()))
        .any(|source| source.source_id != source_id && !source.is_deleted)
    {
        return Err("爬虫数据源编码已存在".into());
    }
    row.created_at = existing.created_at;
    row.last_crawled_at = existing.last_crawled_at;
    row.updated_at = ctx.timestamp;
    ctx.db.crawler_source().source_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_crawler_source(ctx: &ReducerContext, source_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut source = require_crawler_source(ctx, source_id)?;
    let has_active_task = ctx
        .db
        .crawler_task()
        .crawler_task_by_source()
        .filter(source_id)
        .any(|task| matches!(task.status.as_str(), "PENDING" | "RUNNING"));
    if has_active_task {
        return Err("数据源仍有等待或运行中的任务".into());
    }
    source.enabled = false;
    source.is_deleted = true;
    source.updated_at = ctx.timestamp;
    ctx.db.crawler_source().source_id().update(source);
    Ok(())
}

fn validated_source(
    ctx: &ReducerContext,
    source_id: u64,
    customer_id: String,
    input: CrawlerSourceInput,
) -> Result<CrawlerSource, String> {
    let source_code = required_text(input.source_code, "数据源编码不能为空")?;
    let source_name = required_text(input.source_name, "数据源名称不能为空")?;
    let source_type = required_text(input.source_type, "数据源类型不能为空")?;
    let base_url = required_text(input.base_url, "数据源地址不能为空")?;
    validate_max_length(&source_code, 100, "数据源编码不能超过100个字符")?;
    validate_max_length(&source_name, 100, "数据源名称不能超过100个字符")?;
    validate_max_length(&source_type, 50, "数据源类型不能超过50个字符")?;
    validate_max_length(&base_url, 500, "数据源地址不能超过500个字符")?;
    if input.crawl_interval_minutes == 0 || input.rate_limit_per_minute == 0 {
        return Err("抓取间隔和每分钟限速必须大于0".into());
    }
    Ok(CrawlerSource {
        source_id,
        customer_id,
        source_code,
        source_name,
        source_type,
        base_url,
        robots_url: limited_optional(input.robots_url, 500, "robots地址不能超过500个字符")?,
        enabled: input.enabled,
        crawl_interval_minutes: input.crawl_interval_minutes,
        rate_limit_per_minute: input.rate_limit_per_minute,
        allowed_paths: normalized_list(input.allowed_paths)?,
        blocked_paths: normalized_list(input.blocked_paths)?,
        keyword_include: normalized_list(input.keyword_include)?,
        keyword_exclude: normalized_list(input.keyword_exclude)?,
        region_scope: normalized_list(input.region_scope)?,
        last_crawled_at: None,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    })
}

fn normalized_list(values: Vec<String>) -> Result<Vec<String>, String> {
    let mut result = BTreeSet::new();
    for value in values {
        let value = required_text(value, "数据源配置项不能为空")?;
        validate_max_length(&value, 500, "数据源配置项不能超过500个字符")?;
        result.insert(value);
    }
    Ok(result.into_iter().collect())
}
