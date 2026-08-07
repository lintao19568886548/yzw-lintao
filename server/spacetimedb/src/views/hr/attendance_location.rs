//! 当前租户的打卡点。
//!
//! 全员可读，不只管理员：打卡页要在员工点按钮之前就告诉他「你不在范围内」，
//! 而不是等提交被服务端拒了才知道。地点和半径本来就是要让员工知道的规则，
//! 不是秘密。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_attendance_locations, public)]
pub fn my_attendance_locations(ctx: &ViewContext) -> Vec<AttendanceLocation> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .attendance_location()
        .attendance_location_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.location_id);
    rows
}
