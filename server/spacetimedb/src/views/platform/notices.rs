//! 客户端可订阅的有效公告视图。

use spacetimedb::ViewContext;

use crate::tables::*;

#[spacetimedb::view(accessor = valid_notices, public)]
pub fn valid_notices(ctx: &ViewContext) -> Vec<Notice> {
    let mut rows = ctx
        .db
        .notice()
        .notice_by_validity()
        .filter(true)
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .date
            .cmp(&left.date)
            .then_with(|| right.notice_id.cmp(&left.notice_id))
    });
    rows
}
