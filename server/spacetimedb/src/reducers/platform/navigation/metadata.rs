//! 菜单展示元数据的新增与更新逻辑。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_menu},
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

/// 客户端提交的菜单展示信息。
#[derive(SpacetimeType)]
pub struct MenuMetaInput {
    pub title: String,
    pub icon: Option<String>,
    pub order: Option<i32>,
    pub active_icon: Option<String>,
    pub active_path: Option<String>,
    pub affix_tab: Option<bool>,
    pub affix_tab_order: Option<i32>,
    pub badge_content: Option<String>,
    pub badge_type: Option<String>,
    pub badge_variants: Option<String>,
    pub hide_children_in_menu: Option<bool>,
    pub hide_in_breadcrumb: Option<bool>,
    pub hide_in_menu: Option<bool>,
    pub hide_in_tab: Option<bool>,
    pub iframe_src: Option<String>,
    pub keep_alive: Option<bool>,
    pub link: Option<String>,
    pub max_num_of_open_tab: Option<i32>,
    pub no_basic_layout: Option<bool>,
    pub open_in_new_window: Option<bool>,
    pub color: Option<String>,
    pub is_app: Option<bool>,
}

#[spacetimedb::reducer]
pub fn upsert_menu_meta(
    ctx: &ReducerContext,
    menu_id: u64,
    input: MenuMetaInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_menu(ctx, menu_id)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let title = required_text(input.title, "菜单标题不能为空")?;
    let existing = ctx
        .db
        .menu_meta()
        .menu_meta_by_menu()
        .filter(menu_id)
        .find(|row| row.customer_id == customer_id);
    let row = MenuMeta {
        meta_id: existing.as_ref().map_or(0, |row| row.meta_id),
        customer_id,
        menu_id,
        title,
        icon: normalize_optional_text(input.icon),
        order: input.order.unwrap_or(1),
        active_icon: normalize_optional_text(input.active_icon),
        active_path: normalize_optional_text(input.active_path),
        affix_tab: input.affix_tab,
        affix_tab_order: input.affix_tab_order,
        badge_content: normalize_optional_text(input.badge_content),
        badge_type: normalize_optional_text(input.badge_type),
        badge_variants: normalize_optional_text(input.badge_variants),
        hide_children_in_menu: input.hide_children_in_menu,
        hide_in_breadcrumb: input.hide_in_breadcrumb,
        hide_in_menu: input.hide_in_menu,
        hide_in_tab: input.hide_in_tab,
        iframe_src: normalize_optional_text(input.iframe_src),
        keep_alive: input.keep_alive,
        link: normalize_optional_text(input.link),
        max_num_of_open_tab: input.max_num_of_open_tab,
        no_basic_layout: input.no_basic_layout,
        open_in_new_window: input.open_in_new_window,
        color: normalize_optional_text(input.color),
        is_app: input.is_app,
    };
    if existing.is_some() {
        ctx.db.menu_meta().meta_id().update(row);
    } else {
        ctx.db.menu_meta().insert(row);
    }
    Ok(())
}
