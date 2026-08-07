//! 当前用户可见的园区，包括用户直接授权和角色继承授权。

use spacetimedb::{DbContext, ViewContext};

use crate::views::shared::identity::current_principal;
use crate::access;
use crate::tables::*;

#[spacetimedb::view(accessor = my_parks, public)]
pub fn my_parks(ctx: &ViewContext) -> Vec<Park> {
    let Some(principal) = current_principal(ctx) else {
        return vec![];
    };
    access::visible_parks(ctx.db_read_only(), &principal)
}
