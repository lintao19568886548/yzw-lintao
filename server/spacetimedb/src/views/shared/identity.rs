//! 当前 SpacetimeDB 身份对应的业务用户。
//!
//! 判定逻辑全部委托给 `crate::access`，与 reducer 共用同一份实现。
//! View 拿不到时间源，会话过期依赖 `sweep_expired_sessions` 删除过期行。

use spacetimedb::{DbContext, ViewContext};

use crate::access::{self, Principal, ReadScope};
use crate::tables::*;

/// 解析调用者；用户被禁用或不属于当前租户时返回 `None`。
pub(crate) fn current_principal(ctx: &ViewContext) -> Option<Principal> {
    access::principal(ctx.db_read_only(), ctx.sender(), None)
}

/// 解析调用者的读取范围（租户 × 园区 × 行主），业务 view 的统一入口。
pub(crate) fn current_read_scope(ctx: &ViewContext) -> Option<ReadScope> {
    access::read_scope(ctx.db_read_only(), ctx.sender(), None)
}

#[spacetimedb::view(accessor = current_user, public)]
pub fn current_user(ctx: &ViewContext) -> Option<SystemUser> {
    current_principal(ctx).map(|principal| principal.user)
}

#[spacetimedb::view(accessor = current_center_user, public)]
pub fn current_center_user(ctx: &ViewContext) -> Option<CenterUser> {
    current_center_user_id(ctx)
        .and_then(|center_user_id| ctx.db.center_user().id().find(center_user_id))
}

pub(crate) fn current_center_user_id(ctx: &ViewContext) -> Option<u64> {
    access::current_center_user_id(ctx.db_read_only(), ctx.sender(), None)
}

pub(crate) fn current_user_id(ctx: &ViewContext) -> Option<u64> {
    current_principal(ctx).map(|principal| principal.user.id)
}
