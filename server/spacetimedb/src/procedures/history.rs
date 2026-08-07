//! 财务、账单和工资历史数据的一次性分页查询。
//!
//! 这些 Procedure 替代客户端对历史整表的长期订阅。所有查询都在短事务中完成，
//! 并复用当前用户的租户与园区权限范围。

use std::collections::BTreeSet;

use spacetimedb::{DbContext, ProcedureContext, ReducerContext, SpacetimeType};

use crate::{
    reducers::shared::access::{current_user_id, require_park_access, require_user},
    tables::{
        AmountBill, Finance, FinanceImage, Salary, amount_bill, employee, employee_salary, finance,
        finance_image,
    },
};

const DEFAULT_PAGE_SIZE: u32 = 20;
const MAX_PAGE_SIZE: u32 = 100;

#[derive(SpacetimeType)]
pub struct FinancePageResult {
    pub success: bool,
    pub message: String,
    pub rows: Vec<Finance>,
    pub images: Vec<FinanceImage>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub income_cents: i64,
    pub expense_cents: i64,
    pub income_count: u64,
    pub expense_count: u64,
}

#[derive(SpacetimeType)]
pub struct BillingPageResult {
    pub success: bool,
    pub message: String,
    pub rows: Vec<AmountBill>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub receivable_cents: i64,
    pub received_cents: i64,
    pub outstanding_cents: i64,
    pub overpaid_cents: i64,
}

#[derive(SpacetimeType)]
pub struct SalaryPageResult {
    pub success: bool,
    pub message: String,
    pub rows: Vec<Salary>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub paid_count: u64,
    pub pending_count: u64,
    pub paid_cents: i64,
}

#[spacetimedb::procedure]
pub fn query_finance_page(
    ctx: &mut ProcedureContext,
    page: u32,
    page_size: u32,
    keyword: String,
    transaction_type: Option<String>,
    park_id: Option<u64>,
    start_time_micros: Option<i64>,
    end_time_micros: Option<i64>,
) -> FinancePageResult {
    let (page, page_size) = normalized_page(page, page_size);
    let result = ctx.try_with_tx(|tx| -> Result<FinancePageResult, String> {
        let customer_id = query_customer_id(tx)?;
        let keyword = keyword.trim().to_lowercase();
        let transaction_type = normalized_filter(transaction_type.clone());
        let mut rows = tx
            .db
            .finance()
            .finance_by_customer()
            .filter(customer_id.as_str())
            .filter(|row| {
                !row.is_deleted
                    && require_park_access(tx, row.park_id).is_ok()
                    && park_id.is_none_or(|value| row.park_id == value)
                    && transaction_type
                        .as_deref()
                        .is_none_or(|value| row.transaction_type == value)
                    && start_time_micros.is_none_or(|value| {
                        row.transaction_time.to_micros_since_unix_epoch() >= value
                    })
                    && end_time_micros.is_none_or(|value| {
                        row.transaction_time.to_micros_since_unix_epoch() <= value
                    })
                    && (keyword.is_empty()
                        || row.bill_name.to_lowercase().contains(&keyword)
                        || row.bill_category.to_lowercase().contains(&keyword)
                        || row
                            .remark
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&keyword))
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| {
            (
                std::cmp::Reverse(row.transaction_time),
                std::cmp::Reverse(row.finance_id),
            )
        });
        let income_cents = rows
            .iter()
            .filter(|row| row.transaction_type == "收入")
            .map(|row| row.amount_cents)
            .sum();
        let expense_cents = rows
            .iter()
            .filter(|row| row.transaction_type == "支出")
            .map(|row| row.amount_cents)
            .sum();
        let income_count = rows
            .iter()
            .filter(|row| row.transaction_type == "收入")
            .count() as u64;
        let expense_count = rows
            .iter()
            .filter(|row| row.transaction_type == "支出")
            .count() as u64;
        let total = rows.len() as u64;
        let rows = page_rows(rows, page, page_size);
        let finance_ids = rows
            .iter()
            .map(|row| row.finance_id)
            .collect::<BTreeSet<_>>();
        let mut images = tx
            .db
            .finance_image()
            .finance_image_by_customer()
            .filter(customer_id.as_str())
            .filter(|image| finance_ids.contains(&image.finance_id))
            .collect::<Vec<_>>();
        images.sort_by_key(|image| (image.finance_id, image.id));
        Ok(FinancePageResult {
            success: true,
            message: String::new(),
            rows,
            images,
            total,
            page,
            page_size,
            income_cents,
            expense_cents,
            income_count,
            expense_count,
        })
    });
    result.unwrap_or_else(|message| FinancePageResult {
        success: false,
        message,
        rows: Vec::new(),
        images: Vec::new(),
        total: 0,
        page,
        page_size,
        income_cents: 0,
        expense_cents: 0,
        income_count: 0,
        expense_count: 0,
    })
}

#[spacetimedb::procedure]
pub fn query_billing_page(
    ctx: &mut ProcedureContext,
    page: u32,
    page_size: u32,
    project_keyword: String,
    tenant_keyword: String,
    collection_status: Option<String>,
    park_id: Option<u64>,
    start_time_micros: Option<i64>,
    end_time_micros: Option<i64>,
) -> BillingPageResult {
    let (page, page_size) = normalized_page(page, page_size);
    let result = ctx.try_with_tx(|tx| -> Result<BillingPageResult, String> {
        let customer_id = query_customer_id(tx)?;
        let project_keyword = project_keyword.trim().to_lowercase();
        let tenant_keyword = tenant_keyword.trim().to_lowercase();
        let collection_status = normalized_filter(collection_status.clone());
        let mut rows = tx
            .db
            .amount_bill()
            .amount_bill_by_customer()
            .filter(customer_id.as_str())
            .filter(|row| {
                require_park_access(tx, row.park_id).is_ok()
                    && park_id.is_none_or(|value| row.park_id == value)
                    && (project_keyword.is_empty()
                        || row.project_name.to_lowercase().contains(&project_keyword))
                    && (tenant_keyword.is_empty()
                        || row
                            .tenant_name
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&tenant_keyword))
                    && collection_status
                        .as_deref()
                        .is_none_or(|value| bill_status(row) == value)
                    && start_time_micros.is_none_or(|value| {
                        row.receipt_time
                            .unwrap_or(row.created_at)
                            .to_micros_since_unix_epoch()
                            >= value
                    })
                    && end_time_micros.is_none_or(|value| {
                        row.receipt_time
                            .unwrap_or(row.created_at)
                            .to_micros_since_unix_epoch()
                            <= value
                    })
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| std::cmp::Reverse(row.bill_id));
        let receivable_cents = rows.iter().map(|row| row.total_fee_cents).sum();
        let received_cents = rows.iter().map(|row| row.receipt_amount_cents).sum();
        let outstanding_cents = rows.iter().map(outstanding_cents).sum();
        let overpaid_cents = rows.iter().map(overpaid_cents).sum();
        let total = rows.len() as u64;
        Ok(BillingPageResult {
            success: true,
            message: String::new(),
            rows: page_rows(rows, page, page_size),
            total,
            page,
            page_size,
            receivable_cents,
            received_cents,
            outstanding_cents,
            overpaid_cents,
        })
    });
    result.unwrap_or_else(|message| BillingPageResult {
        success: false,
        message,
        rows: Vec::new(),
        total: 0,
        page,
        page_size,
        receivable_cents: 0,
        received_cents: 0,
        outstanding_cents: 0,
        overpaid_cents: 0,
    })
}

#[spacetimedb::procedure]
pub fn query_salary_page(
    ctx: &mut ProcedureContext,
    page: u32,
    page_size: u32,
    name_keyword: String,
    phone_keyword: String,
    issued: Option<bool>,
    department: String,
    start_time_micros: Option<i64>,
    end_time_micros: Option<i64>,
) -> SalaryPageResult {
    let (page, page_size) = normalized_page(page, page_size);
    let result = ctx.try_with_tx(|tx| -> Result<SalaryPageResult, String> {
        let customer_id = query_customer_id(tx)?;
        let name_keyword = name_keyword.trim().to_lowercase();
        let phone_keyword = phone_keyword.trim();
        let department = department.trim().to_lowercase();
        // 工资的可见范围跟员工一致：持 hr:manage 职能的看全员，其他人只
        // 看自己的——薪资是敏感数据，不能因为换了个入口就放宽。
        let scope = crate::access::read_scope(tx.db_read_only(), tx.sender(), Some(tx.timestamp));
        let is_hr_manager = scope
            .as_ref()
            .is_some_and(|scope| scope.has_duty(crate::access::Duty::HrManage));
        let mut rows = tx
            .db
            .employee_salary()
            .employee_salary_by_customer()
            .filter(customer_id.as_str())
            .filter(|row| {
                if row.is_deleted || issued.is_some_and(|value| row.issued != Some(value)) {
                    return false;
                }
                let Some(employee) = tx
                    .db
                    .employee()
                    .employee_id()
                    .find(row.employee_id)
                    .filter(|employee| !employee.is_deleted)
                else {
                    return false;
                };
                let visible = is_hr_manager
                    || scope
                        .as_ref()
                        .is_some_and(|scope| scope.owns(employee.user_id));
                visible
                    && (department.is_empty()
                        || employee
                            .department
                            .as_deref()
                            .is_some_and(|value| value.to_lowercase().contains(&department)))
                    && (name_keyword.is_empty()
                        || employee.name.to_lowercase().contains(&name_keyword))
                    && (phone_keyword.is_empty() || employee.phone.contains(phone_keyword))
                    && start_time_micros.is_none_or(|value| {
                        row.issue_date
                            .is_some_and(|date| date.to_micros_since_unix_epoch() >= value)
                    })
                    && end_time_micros.is_none_or(|value| {
                        row.issue_date
                            .is_some_and(|date| date.to_micros_since_unix_epoch() <= value)
                    })
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| std::cmp::Reverse(row.salary_id));
        let paid_count = rows.iter().filter(|row| row.issued == Some(true)).count() as u64;
        let pending_count = rows.len() as u64 - paid_count;
        let paid_cents = rows
            .iter()
            .filter(|row| row.issued == Some(true))
            .filter_map(|row| row.salary_amount_cents)
            .sum();
        let total = rows.len() as u64;
        Ok(SalaryPageResult {
            success: true,
            message: String::new(),
            rows: page_rows(rows, page, page_size),
            total,
            page,
            page_size,
            paid_count,
            pending_count,
            paid_cents,
        })
    });
    result.unwrap_or_else(|message| SalaryPageResult {
        success: false,
        message,
        rows: Vec::new(),
        total: 0,
        page,
        page_size,
        paid_count: 0,
        pending_count: 0,
        paid_cents: 0,
    })
}

fn query_customer_id(ctx: &ReducerContext) -> Result<String, String> {
    let user_id = current_user_id(ctx).ok_or("当前身份未绑定用户")?;
    let user = require_user(ctx, user_id)?;
    if user.status != 1 {
        return Err("用户已被禁用".into());
    }
    Ok(user.customer_id)
}

fn normalized_page(page: u32, page_size: u32) -> (u32, u32) {
    (
        page.max(1),
        if page_size == 0 {
            DEFAULT_PAGE_SIZE
        } else {
            page_size.min(MAX_PAGE_SIZE)
        },
    )
}

fn normalized_filter(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty() && value != "全部" && value != "all").then_some(value)
    })
}

fn page_rows<T>(rows: Vec<T>, page: u32, page_size: u32) -> Vec<T> {
    let start = (page.saturating_sub(1) as usize).saturating_mul(page_size as usize);
    rows.into_iter()
        .skip(start)
        .take(page_size as usize)
        .collect()
}

fn outstanding_cents(row: &AmountBill) -> i64 {
    row.total_fee_cents
        .saturating_sub(row.receipt_amount_cents)
        .max(0)
}

fn overpaid_cents(row: &AmountBill) -> i64 {
    row.receipt_amount_cents
        .saturating_sub(row.total_fee_cents)
        .max(0)
}

fn bill_status(row: &AmountBill) -> &'static str {
    if overpaid_cents(row) > 0 {
        "overpaid"
    } else if outstanding_cents(row) == 0 {
        "paid"
    } else if row.receipt_amount_cents > 0 {
        "partial"
    } else {
        "unpaid"
    }
}
