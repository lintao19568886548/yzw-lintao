//! 当前租户可用的客户端应用版本。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_app_versions, public)]
pub fn my_app_versions(ctx: &ViewContext) -> Vec<AppVersion> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .app_version()
        .app_version_by_customer()
        .filter(scope.customer_id.as_str())
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.created_at, row.id));
    rows.reverse();
    rows
}

#[spacetimedb::view(accessor = latest_app_version, public)]
pub fn latest_app_version(ctx: &ViewContext) -> Option<AppVersion> {
    my_app_versions(ctx).into_iter().next()
}
