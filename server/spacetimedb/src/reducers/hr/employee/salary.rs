//! 工资记录创建、更新与物理删除，以及发放状态与财务流水的同步。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::require_employee;
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_salary},
        park_ref::NO_PARK,
        platform::media::image_reducer::delete_image_if_unreferenced,
        validation::normalize_optional_text,
    },
    tables::*,
};

/// 工资记录的可修改字段。
#[derive(SpacetimeType)]
pub struct SalaryInput {
    /// 领取这笔工资的员工。
    pub employee_id: u64,
    pub salary_amount_cents: Option<i64>,
    pub issue_date: Option<Timestamp>,
    pub issued: Option<bool>,
    pub remark: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_salary(ctx: &ReducerContext, input: SalaryInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_salary(ctx, 0, customer_id, input)?;
    let row = ctx.db.employee_salary().insert(row);
    sync_salary_finance(ctx, &row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_salary(
    ctx: &ReducerContext,
    salary_id: u64,
    input: SalaryInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_salary(ctx, salary_id)?;
    let mut row = validated_salary(ctx, salary_id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    let row = ctx.db.employee_salary().salary_id().update(row);
    sync_salary_finance(ctx, &row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_salary(ctx: &ReducerContext, salary_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .employee_salary()
        .salary_id()
        .find(salary_id)
        .filter(|salary| salary.customer_id == customer_id)
        .ok_or("工资记录不存在")?;
    let image_ids = delete_salary_image_links(ctx, salary_id);
    for img_id in image_ids {
        delete_image_if_unreferenced(ctx, img_id);
    }
    detach_salary_finance(ctx, salary_id);
    ctx.db.employee_salary().salary_id().delete(salary_id);
    Ok(())
}

pub(super) fn delete_salary_image_links(ctx: &ReducerContext, salary_id: u64) -> Vec<u64> {
    let links = ctx
        .db
        .salary_image()
        .salary_image_by_salary()
        .filter(salary_id)
        .collect::<Vec<_>>();
    for link in &links {
        ctx.db.salary_image().id().delete(link.id);
    }
    links.into_iter().map(|link| link.img_id).collect()
}

pub(super) fn validated_salary(
    ctx: &ReducerContext,
    salary_id: u64,
    customer_id: String,
    input: SalaryInput,
) -> Result<Salary, String> {
    require_employee(ctx, input.employee_id)?;
    if input.salary_amount_cents.is_some_and(|value| value < 0) {
        return Err("工资金额不能为负数".into());
    }
    check_issued_amount(input.issued, input.salary_amount_cents)?;
    Ok(Salary {
        salary_id,
        customer_id,
        employee_id: input.employee_id,
        salary_amount_cents: input.salary_amount_cents,
        issue_date: input.issue_date,
        issued: input.issued,
        remark: normalize_optional_text(input.remark),
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

/// 已发放的工资必须有金额——否则会往财务流水记一笔 0 元支出。
#[pure_function::pure]
fn check_issued_amount(issued: Option<bool>, amount_cents: Option<i64>) -> Result<(), String> {
    if issued.unwrap_or(false) && amount_cents.is_none() {
        return Err("已发放的工资必须填写金额".into());
    }
    Ok(())
}

/// 让财务流水跟上工资的发放状态。
///
/// 口径对齐报销（`finance/approvals/reimbursement.rs`）：记账靠插入，退回靠
/// `is_deleted` 软删；取消发放后再次发放会复用同一笔流水。工资是公司层面的
/// 支出，不挂园区（[`NO_PARK`]），受园区限制的账号在财务管理里看不到这笔支出。
pub(super) fn sync_salary_finance(ctx: &ReducerContext, salary: &Salary) {
    let link = ctx
        .db
        .salary_finance()
        .salary_finance_by_salary()
        .filter(salary.salary_id)
        .next();
    match (salary.issued.unwrap_or(false), link) {
        (true, None) => {
            let finance = ctx.db.finance().insert(Finance {
                finance_id: 0,
                customer_id: salary.customer_id.clone(),
                bill_name: salary_bill_name(ctx, salary),
                bill_category: "工资支出".into(),
                amount_cents: salary.salary_amount_cents.unwrap_or(0),
                transaction_type: "支出".into(),
                transaction_time: salary.issue_date.unwrap_or(ctx.timestamp),
                remark: Some(format!("工资 #{}", salary.salary_id)),
                park_id: NO_PARK,
                status: 0,
                is_deleted: false,
                created_at: ctx.timestamp,
                updated_at: None,
            });
            ctx.db.salary_finance().insert(SalaryFinance {
                id: 0,
                customer_id: salary.customer_id.clone(),
                salary_id: salary.salary_id,
                finance_id: finance.finance_id,
                created_at: ctx.timestamp,
                updated_at: None,
            });
        }
        (true, Some(link)) => {
            if let Some(mut finance) = ctx.db.finance().finance_id().find(link.finance_id) {
                finance.bill_name = salary_bill_name(ctx, salary);
                finance.amount_cents = salary.salary_amount_cents.unwrap_or(0);
                finance.transaction_time = salary.issue_date.unwrap_or(ctx.timestamp);
                finance.is_deleted = false;
                finance.updated_at = Some(ctx.timestamp);
                ctx.db.finance().finance_id().update(finance);
            }
        }
        (false, Some(link)) => {
            if let Some(mut finance) = ctx.db.finance().finance_id().find(link.finance_id) {
                finance.is_deleted = true;
                finance.updated_at = Some(ctx.timestamp);
                ctx.db.finance().finance_id().update(finance);
            }
        }
        (false, None) => {}
    }
}

/// 物理删除工资前冲掉已记的流水并移除关联。
pub(super) fn detach_salary_finance(ctx: &ReducerContext, salary_id: u64) {
    let links = ctx
        .db
        .salary_finance()
        .salary_finance_by_salary()
        .filter(salary_id)
        .collect::<Vec<_>>();
    for link in links {
        if let Some(mut finance) = ctx.db.finance().finance_id().find(link.finance_id) {
            finance.is_deleted = true;
            finance.updated_at = Some(ctx.timestamp);
            ctx.db.finance().finance_id().update(finance);
        }
        ctx.db.salary_finance().id().delete(link.id);
    }
}

fn salary_bill_name(ctx: &ReducerContext, salary: &Salary) -> String {
    ctx.db
        .employee()
        .employee_id()
        .find(salary.employee_id)
        .map(|employee| format!("{}工资", employee.name))
        .unwrap_or_else(|| "员工工资".into())
}

#[cfg(test)]
mod tests {
    use super::check_issued_amount;

    #[test]
    fn 已发放的工资必须有金额() {
        assert!(check_issued_amount(Some(true), None).is_err());
    }

    #[test]
    fn 未发放的工资金额可以先空着() {
        assert!(check_issued_amount(None, None).is_ok());
        assert!(check_issued_amount(Some(false), None).is_ok());
    }

    #[test]
    fn 已发放且有金额可以通过() {
        assert!(check_issued_amount(Some(true), Some(123_45)).is_ok());
    }
}
