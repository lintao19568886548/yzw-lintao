//! 当前用户可见的角色、菜单和权限码。

use std::collections::BTreeMap;

use spacetimedb::{DbContext, SpacetimeType, ViewContext};

use crate::views::shared::identity::{current_principal, current_user_id};
use crate::access;
use crate::tables::*;

const ROLE_MANAGEMENT_PATH: &str = "/system/role";

#[spacetimedb::view(accessor = my_roles, public)]
pub fn my_roles(ctx: &ViewContext) -> Vec<Role> {
    current_principal(ctx)
        .map(|principal| principal.roles)
        .unwrap_or_default()
}

#[spacetimedb::view(accessor = my_menus, public)]
pub fn my_menus(ctx: &ViewContext) -> Vec<Menu> {
    let Some(principal) = current_principal(ctx) else {
        return vec![];
    };
    let user = &principal.user;
    if principal.is_admin() {
        let mut menus = ctx
            .db
            .menu()
            .menu_by_customer()
            .filter(user.customer_id.as_str())
            .filter(|menu| menu.status == 1 && menu.template_deleted_at.is_none())
            .collect::<Vec<_>>();
        menus.sort_by_key(|menu| menu.menu_id);
        return menus;
    }
    let mut menus = Vec::new();
    for role in &principal.roles {
        for link in ctx
            .db
            .role_menu()
            .role_menu_by_role()
            .filter(role.role_id)
            .filter(|link| !link.is_deleted)
        {
            if let Some(menu) = ctx.db.menu().menu_id().find(link.menu_id)
                && menu.customer_id == user.customer_id
                && menu.status == 1
                && menu.template_deleted_at.is_none()
            {
                menus.push(menu);
            }
        }
    }
    // 一个用户可能通过多个角色获得同一菜单，返回前按主键去重。
    menus.sort_by_key(|menu| menu.menu_id);
    menus.dedup_by_key(|menu| menu.menu_id);
    menus
}

#[spacetimedb::view(accessor = my_menu_metadata, public)]
pub fn my_menu_metadata(ctx: &ViewContext) -> Vec<MenuMeta> {
    let menu_ids = my_menus(ctx)
        .into_iter()
        .map(|menu| menu.menu_id)
        .collect::<Vec<_>>();
    let mut metadata = menu_ids
        .into_iter()
        .filter_map(|menu_id| {
            ctx.db
                .menu_meta()
                .menu_meta_by_menu()
                .filter(menu_id)
                .next()
        })
        .collect::<Vec<_>>();
    metadata.sort_by_key(|row| (row.order, row.meta_id));
    metadata
}

/// 客户端最终使用的有效权限码，并标明权限来源。
#[derive(SpacetimeType)]
pub struct EffectivePermission {
    pub code: String,
    /// `role` 表示角色继承，`user` 表示用户直接授权。
    pub source: String,
}

#[spacetimedb::view(accessor = my_codes, public)]
pub fn my_codes(ctx: &ViewContext) -> Vec<EffectivePermission> {
    let Some(user_id) = current_user_id(ctx) else {
        return vec![];
    };
    let mut codes = BTreeMap::new();
    for role in my_roles(ctx) {
        for link in ctx.db.role_code().role_code_by_role().filter(role.role_id) {
            if let Some(code) = ctx.db.code().code_id().find(link.code_id)
                && code.template_deleted_at.is_none()
            {
                codes.entry(code.code).or_insert_with(|| "role".to_string());
            }
        }
    }
    // 用户直接权限优先，便于客户端识别权限的最终来源。
    for direct in ctx.db.user_code().user_code_by_user().filter(user_id) {
        codes.insert(direct.code, "user".to_string());
    }
    codes
        .into_iter()
        .map(|(code, source)| EffectivePermission { code, source })
        .collect()
}

/// 有权维护权限策略时返回调用者所属租户；否则返回 `None`。
///
/// 三个 `manageable_*` 视图共用，避免各自重复解析一次身份。
fn manageable_customer_id(ctx: &ViewContext) -> Option<String> {
    let principal = current_principal(ctx)?;
    access::can_access_path(ctx.db_read_only(), &principal, ROLE_MANAGEMENT_PATH)
        .then_some(principal.user.customer_id)
}

/// 权限管理页面可维护的全部角色；普通业务页面仍只能订阅 `my_roles`。
#[spacetimedb::view(accessor = manageable_roles, public)]
pub fn manageable_roles(ctx: &ViewContext) -> Vec<Role> {
    let Some(customer_id) = manageable_customer_id(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .role()
        .role_by_customer()
        .filter(customer_id.as_str())
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.role_id);
    rows
}

/// 角色表单中的完整菜单树。
#[spacetimedb::view(accessor = manageable_menus, public)]
pub fn manageable_menus(ctx: &ViewContext) -> Vec<Menu> {
    let Some(customer_id) = manageable_customer_id(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .menu()
        .menu_by_customer()
        .filter(customer_id.as_str())
        .filter(|menu| menu.status == 1 && menu.template_deleted_at.is_none())
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.menu_id);
    rows
}

/// 角色表单中的全部有效园区选项。
#[spacetimedb::view(accessor = manageable_parks, public)]
pub fn manageable_parks(ctx: &ViewContext) -> Vec<Park> {
    let Some(customer_id) = manageable_customer_id(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .park()
        .park_by_customer()
        .filter(customer_id.as_str())
        .filter(|park| !park.is_deleted)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.park_id);
    rows
}

/// 当前租户角色的菜单授权关系。
#[spacetimedb::view(accessor = manageable_role_menus, public)]
pub fn manageable_role_menus(ctx: &ViewContext) -> Vec<RoleMenu> {
    let mut rows = Vec::new();
    for role in manageable_roles(ctx) {
        rows.extend(
            ctx.db
                .role_menu()
                .role_menu_by_role()
                .filter(role.role_id)
                .filter(|link| !link.is_deleted),
        );
    }
    rows.sort_by_key(|row| (row.role_id, row.menu_id));
    rows
}

/// 当前租户角色的园区数据范围。
#[spacetimedb::view(accessor = manageable_role_parks, public)]
pub fn manageable_role_parks(ctx: &ViewContext) -> Vec<RolePark> {
    let mut rows = Vec::new();
    for role in manageable_roles(ctx) {
        rows.extend(
            ctx.db
                .role_park()
                .role_park_by_role()
                .filter(role.role_id)
                .filter(|link| !link.is_deleted),
        );
    }
    rows.sort_by_key(|row| (row.role_id, row.park_id));
    rows
}
