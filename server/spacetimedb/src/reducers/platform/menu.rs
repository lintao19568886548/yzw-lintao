//! 菜单创建逻辑。

use spacetimedb::{ReducerContext, Table};

use crate::reducers::{
    access::{AdminContext, current_customer_id, require_menu},
    validation::{normalize_optional_text, required_text},
};
use crate::tables::*;

#[spacetimedb::reducer]
pub fn create_menu(
    ctx: &ReducerContext,
    name: String,
    menu_type: String,
    path: String,
    component: Option<String>,
    parent_id: Option<u64>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let name = required_text(name, "菜单名称不能为空")?;
    let menu_type = required_text(menu_type, "菜单类型不能为空")?;
    let path = required_text(path, "菜单路径不能为空")?;
    if let Some(parent_id) = parent_id {
        require_menu(ctx, parent_id)?;
    }
    ctx.db.menu().insert(Menu {
        menu_id: 0,
        customer_id,
        name,
        menu_type,
        status: 1,
        path,
        active_path: None,
        redirect: None,
        component: normalize_optional_text(component),
        parent_id,
        auth_code: None,
        template_key: None,
        template_parent_key: None,
        template_version: 1,
        template_managed: false,
        template_internal_only: false,
        template_deleted_at: None,
    });
    Ok(())
}
