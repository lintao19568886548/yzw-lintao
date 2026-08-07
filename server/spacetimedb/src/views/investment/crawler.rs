//! 当前用户有权订阅的爬虫来源、任务、抓取项和日志。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_crawler_sources, public)]
pub fn my_crawler_sources(ctx: &ViewContext) -> Vec<CrawlerSource> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .crawler_source()
        .crawler_source_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.updated_at);
    rows
}

#[spacetimedb::view(accessor = my_crawler_tasks, public)]
pub fn my_crawler_tasks(ctx: &ViewContext) -> Vec<CrawlerTask> {
    let source_ids = my_crawler_sources(ctx)
        .into_iter()
        .map(|source| source.source_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for source_id in source_ids {
        rows.extend(
            ctx.db
                .crawler_task()
                .crawler_task_by_source()
                .filter(source_id),
        );
    }
    rows.sort_by_key(|row| row.created_at);
    rows
}

#[spacetimedb::view(accessor = my_crawler_task_items, public)]
pub fn my_crawler_task_items(ctx: &ViewContext) -> Vec<CrawlerTaskItem> {
    let source_ids = my_crawler_sources(ctx)
        .into_iter()
        .map(|source| source.source_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for source_id in source_ids {
        rows.extend(
            ctx.db
                .crawler_task_item()
                .crawler_task_item_by_source()
                .filter(source_id),
        );
    }
    rows.sort_by_key(|row| row.updated_at);
    rows
}

#[spacetimedb::view(accessor = my_crawler_task_logs, public)]
pub fn my_crawler_task_logs(ctx: &ViewContext) -> Vec<CrawlerTaskLog> {
    let task_ids = my_crawler_tasks(ctx)
        .into_iter()
        .map(|task| task.task_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for task_id in task_ids {
        rows.extend(
            ctx.db
                .crawler_task_log()
                .crawler_task_log_by_task()
                .filter(task_id),
        );
    }
    rows.sort_by_key(|row| row.created_at);
    rows
}
