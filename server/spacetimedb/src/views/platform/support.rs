//! 当前用户有权查看的反馈记录。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_feedbacks, public)]
pub fn my_feedbacks(ctx: &ViewContext) -> Vec<Feedback> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    // 反馈没有园区维度：管理员看租户全部，其他人只看自己提交的。
    let mut rows = ctx
        .db
        .feedback()
        .feedback_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| scope.is_unrestricted() || scope.owns_id(row.user_id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows
}
