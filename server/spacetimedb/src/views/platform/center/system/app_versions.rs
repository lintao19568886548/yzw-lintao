//! 中心发布的客户端应用版本。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_center_user};

#[spacetimedb::view(accessor = center_app_versions, public)]
pub fn center_app_versions(ctx: &ViewContext) -> Vec<CenterAppVersion> {
    if current_center_user(ctx).is_none() {
        return vec![];
    }
    let mut rows = ctx
        .db
        .center_app_version()
        .center_app_version_by_scope()
        .filter(0u8)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.created_at, row.id));
    rows.reverse();
    rows
}

#[spacetimedb::view(accessor = latest_center_app_version, public)]
pub fn latest_center_app_version(ctx: &ViewContext) -> Option<CenterAppVersion> {
    center_app_versions(ctx).into_iter().next()
}
