//! 删除主记录时清理关联表，模拟原后端事务中的级联行为。

use spacetimedb::ReducerContext;

use crate::tables::*;

pub(crate) fn delete_user_relations(ctx: &ReducerContext, user_id: u64, customer_id: &str) {
    // 审批历史保留申请人和审核人姓名快照，同时清除已删除账号外键。
    let leave_rows = ctx
        .db
        .leave_application()
        .leave_application_by_customer()
        .filter(customer_id)
        .filter(|row| row.user_id == Some(user_id) || row.audit_user_id == Some(user_id))
        .collect::<Vec<_>>();
    for mut row in leave_rows {
        if row.user_id == Some(user_id) {
            row.user_id = None;
        }
        if row.audit_user_id == Some(user_id) {
            row.audit_user_id = None;
        }
        row.updated_at = Some(ctx.timestamp);
        ctx.db.leave_application().id().update(row);
    }
    let reimbursements = ctx
        .db
        .reimbursement()
        .reimbursement_by_customer()
        .filter(customer_id)
        .filter(|row| row.user_id == Some(user_id))
        .collect::<Vec<_>>();
    for mut row in reimbursements {
        row.user_id = None;
        row.updated_at = Some(ctx.timestamp);
        ctx.db.reimbursement().id().update(row);
    }
    // 员工档案保留，但解除已删除账号的软关联。
    let employees = ctx
        .db
        .employee()
        .employee_by_customer()
        .filter(customer_id)
        .filter(|row| row.user_id == Some(user_id))
        .collect::<Vec<_>>();
    for mut employee in employees {
        employee.user_id = None;
        employee.updated_at = Some(ctx.timestamp);
        ctx.db.employee().employee_id().update(employee);
    }
    // 当前设备绑定依赖有效账号；考勤和异常日志作为审计记录继续保留。
    let binding_ids = ctx
        .db
        .attendance_device_binding()
        .attendance_device_binding_by_user()
        .filter(user_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in binding_ids {
        ctx.db.attendance_device_binding().id().delete(id);
    }
    let role_links = ctx
        .db
        .user_role()
        .user_role_by_user()
        .filter(user_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in role_links {
        ctx.db.user_role().id().delete(id);
    }
    let park_links = ctx
        .db
        .user_park()
        .user_park_by_user()
        .filter(user_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in park_links {
        ctx.db.user_park().id().delete(id);
    }
    let code_links = ctx
        .db
        .user_code()
        .user_code_by_user()
        .filter(user_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in code_links {
        ctx.db.user_code().id().delete(id);
    }
    // 删除业务用户时同步清理中心库映射，避免产生悬空账号关系。
    let mapping_ids = ctx
        .db
        .user_tenant_mapping()
        .mapping_by_customer_user()
        .filter((customer_id, user_id))
        .map(|mapping| mapping.id)
        .collect::<Vec<_>>();
    for id in mapping_ids {
        ctx.db.user_tenant_mapping().id().delete(id);
    }
}

pub(crate) fn delete_role_relations(ctx: &ReducerContext, role_id: u64) {
    let user_links = ctx
        .db
        .user_role()
        .user_role_by_role()
        .filter(role_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in user_links {
        ctx.db.user_role().id().delete(id);
    }
    let menu_links = ctx
        .db
        .role_menu()
        .role_menu_by_role()
        .filter(role_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in menu_links {
        ctx.db.role_menu().id().delete(id);
    }
    let park_links = ctx
        .db
        .role_park()
        .role_park_by_role()
        .filter(role_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in park_links {
        ctx.db.role_park().id().delete(id);
    }
    let code_links = ctx
        .db
        .role_code()
        .role_code_by_role()
        .filter(role_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in code_links {
        ctx.db.role_code().id().delete(id);
    }
}
