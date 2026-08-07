//! 当前用户有权访问的员工、考勤与设备审计数据。

use spacetimedb::{SpacetimeType, ViewContext};

use crate::{
    access::{Duty, ReadScope},
    tables::*,
    views::shared::identity::current_read_scope,
};

/// 人事数据范围：读取范围本身，加上「全员人事数据」这一职能。
///
/// 职能由 `hr:manage` 权限码授予，系统管理员默认具备。
fn current_scope(ctx: &ViewContext) -> Option<(ReadScope, bool)> {
    let scope = current_read_scope(ctx)?;
    let is_admin = scope.has_duty(Duty::HrManage);
    Some((scope, is_admin))
}

/// 员工表单绑定业务账号时使用的最小账号信息。
#[derive(SpacetimeType)]
pub struct EmployeeUserOption {
    pub user_id: u64,
    pub username: String,
    pub real_name: String,
    pub phone: Option<String>,
    pub status: i8,
    pub bound_employee_id: Option<u64>,
    /// 账号当前拥有的角色，用于员工档案内展示和编辑。
    pub role_ids: Vec<u64>,
    pub role_names: Vec<String>,
}

#[spacetimedb::view(accessor = my_employee_user_options, public)]
pub fn my_employee_user_options(ctx: &ViewContext) -> Vec<EmployeeUserOption> {
    let Some((scope, is_admin)) = current_scope(ctx) else {
        return vec![];
    };
    let employees = ctx
        .db
        .employee()
        .employee_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    let mut rows = ctx
        .db
        .system_user()
        .business_user_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| is_admin || row.id == scope.user_id)
        .map(|row| {
            let mut roles = ctx
                .db
                .user_role()
                .user_role_by_user()
                .filter(row.id)
                .filter_map(|link| ctx.db.role().role_id().find(link.role_id))
                .collect::<Vec<_>>();
            roles.sort_by_key(|role| role.role_id);
            EmployeeUserOption {
                user_id: row.id,
                username: row.username,
                real_name: row.real_name,
                phone: row.phone,
                status: row.status,
                bound_employee_id: employees
                    .iter()
                    .find(|employee| employee.user_id == Some(row.id))
                    .map(|employee| employee.employee_id),
                role_ids: roles.iter().map(|role| role.role_id).collect(),
                role_names: roles.into_iter().map(|role| role.name).collect(),
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.user_id);
    rows
}

#[spacetimedb::view(accessor = my_employees, public)]
pub fn my_employees(ctx: &ViewContext) -> Vec<Employee> {
    let Some((scope, is_admin)) = current_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .employee()
        .employee_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted && (is_admin || scope.owns(row.user_id)))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.employee_id);
    rows
}

#[spacetimedb::view(accessor = my_attendances, public)]
pub fn my_attendances(ctx: &ViewContext) -> Vec<Attendance> {
    let Some((scope, is_admin)) = current_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .attendance()
        .attendance_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| is_admin || scope.owns(row.user_id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.attendance_id);
    rows
}

#[spacetimedb::view(accessor = my_localizations, public)]
pub fn my_localizations(ctx: &ViewContext) -> Vec<Localization> {
    let Some((scope, is_admin)) = current_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .localization()
        .localization_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| is_admin || row.user_id == scope.user_id)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.localization_id);
    rows
}

#[spacetimedb::view(accessor = my_attendance_device_bindings, public)]
pub fn my_attendance_device_bindings(ctx: &ViewContext) -> Vec<AttendanceDeviceBinding> {
    let Some((scope, is_admin)) = current_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .attendance_device_binding()
        .attendance_device_binding_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| is_admin || row.user_id == scope.user_id)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows
}

#[spacetimedb::view(accessor = my_attendance_device_abnormal_logs, public)]
pub fn my_attendance_device_abnormal_logs(ctx: &ViewContext) -> Vec<AttendanceDeviceAbnormalLog> {
    let Some((scope, is_admin)) = current_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .attendance_device_abnormal_log()
        .attendance_device_abnormal_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| is_admin || row.user_id == scope.user_id)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows
}

/// 当前账号可见员工的工资记录。
///
/// 数据范围跟着员工走：`my_employees` 已按租户和「本人 / 管理员看全部」
/// 过滤过，普通员工因此只能看到自己的工资，看不到同事的。
#[spacetimedb::view(accessor = my_salaries, public)]
pub fn my_salaries(ctx: &ViewContext) -> Vec<Salary> {
    let employee_ids = my_employees(ctx)
        .into_iter()
        .map(|employee| employee.employee_id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut salaries = Vec::new();
    for employee_id in employee_ids {
        salaries.extend(
            ctx.db
                .employee_salary()
                .employee_salary_by_employee()
                .filter(employee_id)
                .filter(|salary| !salary.is_deleted),
        );
    }
    salaries.sort_by_key(|salary| salary.salary_id);
    salaries
}
