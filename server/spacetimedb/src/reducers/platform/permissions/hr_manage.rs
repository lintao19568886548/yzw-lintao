//! 把人事全员数据权限从角色名迁移到权限码。
//!
//! 旧口径按角色名判断（`董事长` / `人事部`）：在权限管理页给角色改个名，就会
//! 静默改变一批人能看到的人事数据，既不报错也无痕迹。新口径由 `hr:manage`
//! 权限码显式授予，改名不再影响可见范围。
//!
//! 迁移是自愈的：`client_connected` 会为每个租户补齐一次，因此发布后不需要
//! 人工执行任何步骤，也不存在「新模块已上线但授权还没迁」的权限断档窗口。

use spacetimedb::{ReducerContext, Table};

use crate::{
    access::CODE_HR_MANAGE,
    reducers::shared::access::{AdminContext, current_customer_id},
    tables::*,
};

/// 迁移前按角色名授予人事全员数据权限的角色。
///
/// 只在租户第一次补齐时使用一次。此后新建的同名角色不会自动获得权限——这正是
/// 这次迁移要消除的行为。
const LEGACY_HR_ROLE_NAMES: [&str; 2] = ["董事长", "人事部"];

/// 手工触发一次迁移，用于排查或补救。日常不需要调用。
#[spacetimedb::reducer]
pub fn migrate_hr_manage_code(ctx: &ReducerContext) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ensure_hr_manage_grants(ctx, &customer_id);
    Ok(())
}

/// 确保租户已经完成人事权限码迁移。
///
/// 幂等且廉价：权限码已存在时只做一次索引查找就返回，因此可以放在连接钩子里。
pub(crate) fn ensure_hr_manage_grants(ctx: &ReducerContext, customer_id: &str) {
    if find_hr_manage_code(ctx, customer_id).is_some() {
        return;
    }
    let code_id = ctx
        .db
        .code()
        .insert(Code {
            code_id: 0,
            customer_id: customer_id.to_string(),
            code: CODE_HR_MANAGE.to_string(),
            name: "人事全员数据".to_string(),
            content: Some("可以查看和维护全公司的员工档案、考勤和请假记录".to_string()),
            menu_id: None,
            created_at: ctx.timestamp,
            updated_at: None,
            template_deleted_at: None,
            template_internal_only: false,
            template_key: None,
            template_managed: false,
            template_version: 1,
        })
        .code_id;

    let role_ids = ctx
        .db
        .role()
        .role_by_customer()
        .filter(customer_id)
        .filter(|role| LEGACY_HR_ROLE_NAMES.contains(&role.name.as_str()))
        .map(|role| role.role_id)
        .collect::<Vec<_>>();
    let granted = role_ids.len();
    for role_id in role_ids {
        ctx.db.role_code().insert(RoleCode {
            id: 0,
            customer_id: customer_id.to_string(),
            role_id,
            code_id,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    log::info!("租户 {customer_id} 人事权限码迁移完成：授予 {granted} 个历史角色");
}

fn find_hr_manage_code(ctx: &ReducerContext, customer_id: &str) -> Option<u64> {
    ctx.db
        .code()
        .code_by_customer_value()
        .filter((customer_id, CODE_HR_MANAGE))
        .find(|row| row.template_deleted_at.is_none())
        .map(|row| row.code_id)
}
