//! 当前用户有权访问的请假申请。

use spacetimedb::ViewContext;

use crate::{access::Duty, tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_leave_applications, public)]
pub fn my_leave_applications(ctx: &ViewContext) -> Vec<LeaveApplication> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    // 本人的请假始终可见；持有人事全员数据权限码时可见全部。
    let is_admin = scope.has_duty(Duty::HrManage);
    let mut rows = ctx
        .db
        .leave_application()
        .leave_application_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| is_admin || scope.owns(row.user_id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows
}
