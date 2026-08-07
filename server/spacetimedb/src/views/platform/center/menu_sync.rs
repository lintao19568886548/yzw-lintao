//! 当前管理员可查看的菜单模板同步任务及目标日志。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_principal};

/// 中心侧管理台仅区分「系统管理员 / 本人」，不涉及园区维度。
fn is_system_admin(ctx: &ViewContext) -> bool {
    current_principal(ctx).is_some_and(|principal| principal.is_admin())
}

#[spacetimedb::view(accessor = my_menu_template_sync_jobs, public)]
pub fn my_menu_template_sync_jobs(ctx: &ViewContext) -> Vec<MenuTemplateSyncJob> {
    if !is_system_admin(ctx) {
        return vec![];
    }
    let mut rows = ctx
        .db
        .menu_template_sync_job()
        .menu_sync_job_by_scope()
        .filter(0u8)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows.reverse();
    rows
}

#[spacetimedb::view(accessor = my_menu_template_sync_logs, public)]
pub fn my_menu_template_sync_logs(ctx: &ViewContext) -> Vec<MenuTemplateSyncLog> {
    let job_ids = my_menu_template_sync_jobs(ctx)
        .into_iter()
        .map(|job| job.id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for job_id in job_ids {
        rows.extend(
            ctx.db
                .menu_template_sync_log()
                .menu_sync_log_by_job()
                .filter(job_id),
        );
    }
    rows.sort_by_key(|row| (row.job_id, row.id));
    rows
}
