//! 员工档案、账号绑定与角色关系的创建、更新和逻辑删除。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::{require_employee, require_hr_manager};
use crate::{
    reducers::{
        access::{
            AdminContext, current_customer_id, current_user_id, require_permission_manager,
            require_role, require_user,
        },
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 员工档案可修改字段，考勤时间使用当天零点后的秒数。
#[derive(SpacetimeType)]
pub struct EmployeeInput {
    pub name: String,
    pub gender: String,
    pub phone: String,
    pub user_id: Option<u64>,
    pub age: Option<i32>,
    pub id_number: Option<String>,
    pub address: Option<String>,
    pub education: Option<String>,
    pub department: Option<String>,
    pub hire_date: Option<Timestamp>,
    pub leave_date: Option<Timestamp>,
    pub remark: Option<String>,
    pub is_resigned: bool,
    pub check_in_seconds: Option<u32>,
    pub check_out_seconds: Option<u32>,
}

#[spacetimedb::reducer]
pub fn create_employee(ctx: &ReducerContext, input: EmployeeInput) -> Result<(), String> {
    create_employee_inner(ctx, input, None)
}

/// 在同一事务中新增员工，并按需为绑定账号设置角色。
///
/// `role_ids = None` 表示调用方没有修改角色；`Some` 表示用传入集合覆盖账号现有角色。
#[spacetimedb::reducer]
pub fn create_employee_with_roles(
    ctx: &ReducerContext,
    input: EmployeeInput,
    role_ids: Option<Vec<u64>>,
) -> Result<(), String> {
    create_employee_inner(ctx, input, role_ids)
}

fn create_employee_inner(
    ctx: &ReducerContext,
    input: EmployeeInput,
    role_ids: Option<Vec<u64>>,
) -> Result<(), String> {
    require_hr_manager(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_employee(ctx, 0, customer_id, input)?;
    sync_employee_user_roles(ctx, row.user_id, role_ids)?;
    ctx.db.employee().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_employee(
    ctx: &ReducerContext,
    employee_id: u64,
    input: EmployeeInput,
) -> Result<(), String> {
    update_employee_inner(ctx, employee_id, input, None)
}

/// 在同一事务中更新员工档案、账号绑定和账号角色。
#[spacetimedb::reducer]
pub fn update_employee_with_roles(
    ctx: &ReducerContext,
    employee_id: u64,
    input: EmployeeInput,
    role_ids: Option<Vec<u64>>,
) -> Result<(), String> {
    update_employee_inner(ctx, employee_id, input, role_ids)
}

fn update_employee_inner(
    ctx: &ReducerContext,
    employee_id: u64,
    input: EmployeeInput,
    role_ids: Option<Vec<u64>>,
) -> Result<(), String> {
    require_hr_manager(ctx)?;
    let existing = require_employee(ctx, employee_id)?;
    let mut row = validated_employee(ctx, employee_id, existing.customer_id, input)?;
    sync_employee_user_roles(ctx, row.user_id, role_ids)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.employee().employee_id().update(row);
    Ok(())
}

/// 按表单提交结果同步账号角色；所有校验和员工保存共享同一事务。
fn sync_employee_user_roles(
    ctx: &ReducerContext,
    user_id: Option<u64>,
    role_ids: Option<Vec<u64>>,
) -> Result<(), String> {
    let Some(role_ids) = role_ids else {
        // 没有角色管理权限的调用方只维护员工档案，不触碰账号角色。
        return Ok(());
    };
    require_permission_manager(ctx)?;

    let wanted = role_ids.into_iter().collect::<BTreeSet<_>>();
    let Some(user_id) = user_id else {
        return wanted
            .is_empty()
            .then_some(())
            .ok_or("请先绑定登录账号，再分配角色".into());
    };
    let user = require_user(ctx, user_id)?;
    if user.status != 1 {
        return Err("停用账号不能分配角色".into());
    }

    let mut includes_system_admin = false;
    for role_id in &wanted {
        let role = require_role(ctx, *role_id)?;
        if role.status != 1 {
            return Err(format!("角色“{}”已停用，不能继续分配", role.name));
        }
        if role.name == "Super" && role.scope == "system" {
            includes_system_admin = true;
        }
    }
    // 系统 Super 只能由另一个系统 Super 授予，避免普通角色管理员越权提权。
    if includes_system_admin {
        AdminContext::require(ctx)?;
    }

    let existing = ctx
        .db
        .user_role()
        .user_role_by_user()
        .filter(user_id)
        .collect::<Vec<_>>();
    let existing_system_admin = existing.iter().any(|link| {
        ctx.db
            .role()
            .role_id()
            .find(link.role_id)
            .is_some_and(|role| role.name == "Super" && role.scope == "system")
    });
    if current_user_id(ctx) == Some(user_id) && existing_system_admin && !includes_system_admin {
        return Err("不能移除当前账号自身的系统管理员角色".into());
    }

    for link in &existing {
        if !wanted.contains(&link.role_id) {
            ctx.db.user_role().id().delete(link.id);
        }
    }
    let existing_ids = existing
        .into_iter()
        .map(|link| link.role_id)
        .collect::<BTreeSet<_>>();
    for role_id in wanted.difference(&existing_ids) {
        ctx.db.user_role().insert(UserRole {
            id: 0,
            user_id,
            role_id: *role_id,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_employee(ctx: &ReducerContext, employee_id: u64) -> Result<(), String> {
    require_hr_manager(ctx)?;
    let mut employee = require_employee(ctx, employee_id)?;
    employee.is_deleted = true;
    employee.leave_date = employee.leave_date.or(Some(ctx.timestamp));
    employee.updated_at = Some(ctx.timestamp);
    ctx.db.employee().employee_id().update(employee);
    Ok(())
}

fn validated_employee(
    ctx: &ReducerContext,
    employee_id: u64,
    customer_id: String,
    input: EmployeeInput,
) -> Result<Employee, String> {
    let name = required_text(input.name, "姓名不能为空")?;
    let gender = required_text(input.gender, "性别不能为空")?;
    let phone = required_text(input.phone, "电话不能为空")?;
    validate_max_length(&name, 50, "姓名不能超过50个字符")?;
    validate_max_length(&gender, 10, "性别不能超过10个字符")?;
    validate_max_length(&phone, 20, "电话不能超过20个字符")?;
    if input.age.is_some_and(|value| !(0..=150).contains(&value)) {
        return Err("年龄必须在0到150之间".into());
    }
    if input.check_in_seconds.is_some_and(|value| value >= 86_400)
        || input.check_out_seconds.is_some_and(|value| value >= 86_400)
    {
        return Err("考勤时间必须在当天范围内".into());
    }
    if let (Some(check_in), Some(check_out)) = (input.check_in_seconds, input.check_out_seconds)
        && check_out <= check_in
    {
        return Err("下班时间必须晚于上班时间".into());
    }
    let id_number = normalized_limited(input.id_number, 18, "身份证号不能超过18个字符")?;
    let address = normalized_limited(input.address, 200, "住址不能超过200个字符")?;
    let education = normalized_limited(input.education, 50, "学历不能超过50个字符")?;
    let department = normalized_limited(input.department, 50, "部门不能超过50个字符")?;
    let remark = normalized_limited(input.remark, 500, "备注不能超过500个字符")?;

    let active_rows = ctx
        .db
        .employee()
        .employee_by_customer()
        .filter(customer_id.as_str())
        .filter(|row| row.employee_id != employee_id && !row.is_deleted)
        .collect::<Vec<_>>();
    if let Some(value) = &id_number
        && active_rows
            .iter()
            .any(|row| row.id_number.as_ref() == Some(value))
    {
        return Err("该身份证号已存在".into());
    }
    if let Some(user_id) = input.user_id {
        let user = require_user(ctx, user_id).map_err(|_| "绑定账号不存在或已删除")?;
        if user.status == 2 {
            return Err("绑定账号不存在或已删除".into());
        }
        if active_rows.iter().any(|row| row.user_id == Some(user_id)) {
            return Err("该账号已绑定其他员工".into());
        }
    }
    let leave_date = input.is_resigned.then_some(input.leave_date).flatten();
    if let (Some(hire_date), Some(leave_date)) = (input.hire_date, leave_date)
        && leave_date < hire_date
    {
        return Err("离职日期不能早于入职日期".into());
    }
    Ok(Employee {
        employee_id,
        customer_id,
        name,
        gender,
        phone,
        user_id: input.user_id,
        age: input.age,
        id_number,
        address,
        education,
        department,
        hire_date: input.hire_date,
        leave_date,
        remark,
        is_deleted: false,
        is_resigned: input.is_resigned,
        check_in_seconds: input.check_in_seconds,
        check_out_seconds: input.check_out_seconds,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

fn normalized_limited(
    value: Option<String>,
    max_chars: usize,
    message: &'static str,
) -> Result<Option<String>, String> {
    let value = normalize_optional_text(value);
    if let Some(text) = &value {
        validate_max_length(text, max_chars, message)?;
    }
    Ok(value)
}
