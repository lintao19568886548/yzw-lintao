//! 爬虫任务链路共用关系校验与日志写入。

use spacetimedb::{ReducerContext, Table};

use crate::{reducers::shared::access::current_customer_id, tables::*};

pub(super) fn require_crawler_source(
    ctx: &ReducerContext,
    source_id: u64,
) -> Result<CrawlerSource, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .crawler_source()
        .source_id()
        .find(source_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("爬虫数据源不存在".into())
}

pub(super) fn require_crawler_task(
    ctx: &ReducerContext,
    task_id: u64,
) -> Result<CrawlerTask, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .crawler_task()
        .task_id()
        .find(task_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("爬虫任务不存在".into())
}

pub(super) fn require_crawler_item(
    ctx: &ReducerContext,
    item_id: u64,
) -> Result<CrawlerTaskItem, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .crawler_task_item()
        .item_id()
        .find(item_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("爬虫抓取项不存在".into())
}

pub(super) fn insert_task_log(
    ctx: &ReducerContext,
    task: &CrawlerTask,
    level: &str,
    stage: &str,
    message: String,
    detail_json: Option<String>,
) {
    ctx.db.crawler_task_log().insert(CrawlerTaskLog {
        log_id: 0,
        customer_id: task.customer_id.clone(),
        task_id: task.task_id,
        level: level.into(),
        stage: stage.into(),
        message,
        detail_json,
        created_at: ctx.timestamp,
    });
}
