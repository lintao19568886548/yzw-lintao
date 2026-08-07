//! 可向客户端公开的中心系统配置。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_center_user};

const INTERNAL_ONLY_KEYS: &[&str] = &["ALIYUN_BAILIAN_KEY"];

#[spacetimedb::view(accessor = public_center_system_keys, public)]
pub fn public_center_system_keys(ctx: &ViewContext) -> Vec<CenterSystemKey> {
    if current_center_user(ctx).is_none() {
        return vec![];
    }
    let mut rows = ctx
        .db
        .center_system_key()
        .center_system_key_by_scope()
        .filter(0u8)
        .filter(|row| !INTERNAL_ONLY_KEYS.contains(&row.key_name.as_str()))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.key_name.cmp(&right.key_name));
    rows
}
