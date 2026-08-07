//! 当前租户可公开读取的系统配置。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

const INTERNAL_ONLY_KEYS: &[&str] = &["ALIYUN_BAILIAN_KEY"];

#[spacetimedb::view(accessor = my_public_system_keys, public)]
pub fn my_public_system_keys(ctx: &ViewContext) -> Vec<SystemKey> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .system_key()
        .system_key_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !INTERNAL_ONLY_KEYS.contains(&row.key_name.as_str()))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.key_name.cmp(&right.key_name));
    rows
}
