//! 角色的创建、层级校验与级联删除逻辑。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::reducers::{
    access::{
        ADMIN_ROLE_NAME, SYSTEM_SCOPE, current_customer_id, require_permission_manager,
        require_role,
    },
    hr::relations::cleanup::delete_role_relations,
    validation::{normalize_optional_text, required_text},
};
use crate::tables::*;

/// 角色基本信息、菜单权限与园区范围的原子保存参数。
#[derive(SpacetimeType)]
pub struct RolePolicyInput {
    /// `None` 表示新增；有值表示编辑现有角色。
    pub role_id: Option<u64>,
    pub name: String,
    pub remark: Option<String>,
    pub status: i8,
    pub parent_id: Option<u64>,
    pub reimbursement_auth: Option<i32>,
    pub rates: Option<i32>,
    pub menu_ids: Vec<u64>,
    pub park_ids: Vec<u64>,
}

#[spacetimedb::reducer]
pub fn create_role(
    ctx: &ReducerContext,
    name: String,
    remark: Option<String>,
    parent_id: Option<u64>,
    scope: String,
) -> Result<(), String> {
    require_permission_manager(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let name = required_text(name, "角色名称不能为空")?;
    let scope = normalize_optional_text(Some(scope)).unwrap_or_else(|| SYSTEM_SCOPE.into());
    if let Some(parent_id) = parent_id {
        require_role(ctx, parent_id)?;
    }
    if ctx
        .db
        .role()
        .iter()
        .any(|role| role.customer_id == customer_id && role.name == name && role.scope == scope)
    {
        return Err("同一作用域内角色名称已存在".into());
    }
    ctx.db.role().insert(Role {
        role_id: 0,
        customer_id,
        name,
        remark: normalize_optional_text(remark),
        status: 1,
        rates: None,
        parent_id,
        reimbursement_auth: None,
        organization_id: None,
        scope,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_role(ctx: &ReducerContext, role_id: u64) -> Result<(), String> {
    require_permission_manager(ctx)?;
    let role = require_role(ctx, role_id)?;
    if role.name == ADMIN_ROLE_NAME && role.scope == SYSTEM_SCOPE {
        return Err("不能删除系统管理员角色".into());
    }
    if ctx
        .db
        .role()
        .iter()
        .any(|candidate| candidate.parent_id == Some(role_id))
    {
        return Err("请先处理子角色".into());
    }
    delete_role_relations(ctx, role_id);
    ctx.db.role().role_id().delete(role_id);
    Ok(())
}

/// 按原系统角色表单口径，一次事务保存角色、菜单权限、按钮权限码和园区范围。
#[spacetimedb::reducer]
pub fn save_role_policy(ctx: &ReducerContext, input: RolePolicyInput) -> Result<(), String> {
    require_permission_manager(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let name = required_text(input.name, "角色名称不能为空")?;
    if !matches!(input.status, 0 | 1) {
        return Err("角色状态只能是启用或禁用".into());
    }
    // 拒绝审核意味着该角色没有任何审核额度，服务端统一持久化为 0。
    let rates = if input.reimbursement_auth == Some(0) {
        Some(0)
    } else {
        input.rates
    };
    if rates.is_some_and(|rates| rates < 0) {
        return Err("审核金额不能小于 0".into());
    }

    let menu_ids = input.menu_ids.into_iter().collect::<BTreeSet<_>>();
    let park_ids = input.park_ids.into_iter().collect::<BTreeSet<_>>();
    validate_policy_targets(ctx, &customer_id, &menu_ids, &park_ids)?;

    let parent = input
        .parent_id
        .map(|parent_id| require_role(ctx, parent_id))
        .transpose()?;
    if let Some(parent) = parent.as_ref() {
        if input.role_id == Some(parent.role_id) {
            return Err("角色不能把自己设为上级".into());
        }
        if input
            .role_id
            .is_some_and(|role_id| is_descendant(ctx, parent.role_id, role_id))
        {
            return Err("不能把下级角色设为上级".into());
        }
        let parent_menu_ids = ctx
            .db
            .role_menu()
            .role_menu_by_role()
            .filter(parent.role_id)
            .filter(|link| !link.is_deleted)
            .map(|link| link.menu_id)
            .collect::<BTreeSet<_>>();
        // 原系统的系统 Super 没有逐条菜单关系时代表拥有全部权限。
        // 只有父角色存在显式权限，或父角色不是系统 Super 时，才校验权限子集。
        let parent_has_implicit_full_access = parent_menu_ids.is_empty()
            && parent.name == ADMIN_ROLE_NAME
            && parent.scope == SYSTEM_SCOPE;
        if !parent_has_implicit_full_access {
            if let Some(menu_id) = menu_ids.difference(&parent_menu_ids).next() {
                return Err(format!(
                    "子角色权限不能超出父角色范围，无效菜单 ID：{menu_id}"
                ));
            }
        }
    }

    if ctx
        .db
        .role()
        .role_by_customer()
        .filter(customer_id.as_str())
        .any(|role| {
            role.role_id != input.role_id.unwrap_or_default()
                && role.name == name
                && role.scope
                    == input
                        .role_id
                        .and_then(|role_id| ctx.db.role().role_id().find(role_id))
                        .map(|role| role.scope)
                        .unwrap_or_else(|| SYSTEM_SCOPE.into())
        })
    {
        return Err("同一作用域内角色名称已存在".into());
    }

    let role = if let Some(role_id) = input.role_id {
        let mut role = require_role(ctx, role_id)?;
        role.name = name;
        role.remark = normalize_optional_text(input.remark);
        role.status = input.status;
        role.parent_id = input.parent_id;
        role.reimbursement_auth = input.reimbursement_auth;
        role.rates = rates;
        role.updated_at = Some(ctx.timestamp);
        ctx.db.role().role_id().update(role)
    } else {
        ctx.db.role().insert(Role {
            role_id: 0,
            customer_id: customer_id.clone(),
            name,
            remark: normalize_optional_text(input.remark),
            status: input.status,
            rates,
            parent_id: input.parent_id,
            reimbursement_auth: input.reimbursement_auth,
            organization_id: parent.as_ref().and_then(|role| role.organization_id),
            scope: parent
                .as_ref()
                .map(|role| role.scope.clone())
                .unwrap_or_else(|| SYSTEM_SCOPE.into()),
            created_at: ctx.timestamp,
            updated_at: None,
        })
    };

    sync_role_menus(ctx, role.role_id, &menu_ids);
    sync_role_parks(ctx, role.role_id, &park_ids);
    sync_role_codes(ctx, &customer_id, role.role_id, &menu_ids);
    Ok(())
}

fn validate_policy_targets(
    ctx: &ReducerContext,
    customer_id: &str,
    menu_ids: &BTreeSet<u64>,
    park_ids: &BTreeSet<u64>,
) -> Result<(), String> {
    for menu_id in menu_ids {
        let menu = ctx
            .db
            .menu()
            .menu_id()
            .find(*menu_id)
            .ok_or("权限菜单不存在")?;
        if menu.customer_id != customer_id || menu.status != 1 || menu.template_deleted_at.is_some()
        {
            return Err(format!("菜单 {menu_id} 不属于当前租户或已停用"));
        }
    }
    for park_id in park_ids {
        let park = ctx.db.park().park_id().find(*park_id).ok_or("园区不存在")?;
        if park.customer_id != customer_id || park.is_deleted {
            return Err(format!("园区 {park_id} 不属于当前租户或已删除"));
        }
    }
    Ok(())
}

fn is_descendant(ctx: &ReducerContext, mut candidate_id: u64, role_id: u64) -> bool {
    let mut visited = BTreeSet::new();
    while visited.insert(candidate_id) {
        let Some(candidate) = ctx.db.role().role_id().find(candidate_id) else {
            return false;
        };
        let Some(parent_id) = candidate.parent_id else {
            return false;
        };
        if parent_id == role_id {
            return true;
        }
        candidate_id = parent_id;
    }
    false
}

fn sync_role_menus(ctx: &ReducerContext, role_id: u64, menu_ids: &BTreeSet<u64>) {
    let existing = ctx
        .db
        .role_menu()
        .role_menu_by_role()
        .filter(role_id)
        .collect::<Vec<_>>();
    for mut link in existing {
        let should_exist = menu_ids.contains(&link.menu_id);
        if link.is_deleted == should_exist {
            link.is_deleted = !should_exist;
            link.updated_at = Some(ctx.timestamp);
            ctx.db.role_menu().id().update(link);
        }
    }
    for menu_id in menu_ids {
        if ctx
            .db
            .role_menu()
            .role_menu_by_pair()
            .filter((role_id, *menu_id))
            .next()
            .is_none()
        {
            ctx.db.role_menu().insert(RoleMenu {
                id: 0,
                role_id,
                menu_id: *menu_id,
                is_deleted: false,
                created_at: ctx.timestamp,
                updated_at: None,
            });
        }
    }
}

fn sync_role_parks(ctx: &ReducerContext, role_id: u64, park_ids: &BTreeSet<u64>) {
    let existing = ctx
        .db
        .role_park()
        .role_park_by_role()
        .filter(role_id)
        .collect::<Vec<_>>();
    for mut link in existing {
        let should_exist = park_ids.contains(&link.park_id);
        if link.is_deleted == should_exist {
            link.is_deleted = !should_exist;
            link.updated_at = Some(ctx.timestamp);
            ctx.db.role_park().id().update(link);
        }
    }
    for park_id in park_ids {
        if ctx
            .db
            .role_park()
            .role_park_by_pair()
            .filter((role_id, *park_id))
            .next()
            .is_none()
        {
            ctx.db.role_park().insert(RolePark {
                id: 0,
                role_id,
                park_id: *park_id,
                is_deleted: false,
                created_at: ctx.timestamp,
                updated_at: None,
            });
        }
    }
}

fn sync_role_codes(
    ctx: &ReducerContext,
    customer_id: &str,
    role_id: u64,
    menu_ids: &BTreeSet<u64>,
) {
    let wanted_code_ids = ctx
        .db
        .code()
        .code_by_customer()
        .filter(customer_id)
        .filter(|code| {
            code.template_deleted_at.is_none()
                && code
                    .menu_id
                    .is_some_and(|menu_id| menu_ids.contains(&menu_id))
        })
        .map(|code| code.code_id)
        .collect::<BTreeSet<_>>();
    let existing = ctx
        .db
        .role_code()
        .role_code_by_role()
        .filter(role_id)
        .collect::<Vec<_>>();
    for link in existing {
        if !wanted_code_ids.contains(&link.code_id) {
            ctx.db.role_code().id().delete(link.id);
        }
    }
    for code_id in wanted_code_ids {
        if ctx
            .db
            .role_code()
            .role_code_by_pair()
            .filter((role_id, code_id))
            .next()
            .is_none()
        {
            ctx.db.role_code().insert(RoleCode {
                id: 0,
                customer_id: customer_id.into(),
                role_id,
                code_id,
                created_at: ctx.timestamp,
                updated_at: None,
            });
        }
    }
}
