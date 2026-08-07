//! 当前中心用户有权查看的中心审计日志。

use spacetimedb::ViewContext;

use crate::{
    tables::*,
    views::shared::identity::{current_center_user, current_principal},
};

#[spacetimedb::view(accessor = my_center_api_logs, public)]
pub fn my_center_api_logs(ctx: &ViewContext) -> Vec<CenterApiLog> {
    let Some(user) = current_center_user(ctx) else {
        return vec![];
    };
    // 中心审计日志不分园区：管理员看全部，其他人只看本人。
    let is_admin = current_principal(ctx).is_some_and(|principal| principal.is_admin());
    let mut rows = ctx
        .db
        .center_api_log()
        .center_api_log_by_scope()
        .filter(0u8)
        .filter(|row| is_admin || row.center_user_id == user.id)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.request_time, row.log_id));
    rows.reverse();
    rows
}
