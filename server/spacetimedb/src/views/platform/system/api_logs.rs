//! 当前用户有权查看的 API 操作审计日志。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_api_logs, public)]
pub fn my_api_logs(ctx: &ViewContext) -> Vec<ApiLog> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    // 审计日志不做园区过滤：只有系统管理员能越过「仅本人」的限制。
    let mut rows = ctx
        .db
        .api_log()
        .api_log_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| scope.is_unrestricted() || scope.owns_id(row.user_id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.request_time, row.log_id));
    rows.reverse();
    rows
}
