use crate::spacetime_bindings::{
    billing_page_result_type::BillingPageResult, finance_page_result_type::FinancePageResult,
    query_billing_page_procedure::query_billing_page,
    query_finance_page_procedure::query_finance_page,
    query_salary_page_procedure::query_salary_page, salary_page_result_type::SalaryPageResult,
};

use super::spacetime::with_connection;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FinancePageQuery {
    pub page: u32,
    pub page_size: u32,
    pub keyword: String,
    pub transaction_type: Option<String>,
    pub park_id: Option<u64>,
    pub start_time_micros: Option<i64>,
    pub end_time_micros: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BillingPageQuery {
    pub page: u32,
    pub page_size: u32,
    pub project_keyword: String,
    pub tenant_keyword: String,
    pub collection_status: Option<String>,
    pub park_id: Option<u64>,
    pub start_time_micros: Option<i64>,
    pub end_time_micros: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SalaryPageQuery {
    pub page: u32,
    pub page_size: u32,
    /// 员工姓名关键字。
    pub name_keyword: String,
    pub phone_keyword: String,
    pub issued: Option<bool>,
    /// 部门关键字。员工没有园区归属，按部门筛选才对得上组织结构。
    pub department: String,
    pub start_time_micros: Option<i64>,
    pub end_time_micros: Option<i64>,
}

fn procedure_result<T>(
    result: Result<T, spacetimedb_sdk::__codegen::InternalError>,
    success: impl FnOnce(&T) -> bool,
    message: impl FnOnce(&T) -> String,
) -> Result<T, String> {
    match result {
        Ok(value) if success(&value) => Ok(value),
        Ok(value) => Err(message(&value)),
        Err(error) => Err(format!("Procedure 调用失败: {error:?}")),
    }
}

pub fn query_finance_history(
    query: FinancePageQuery,
    callback: impl FnOnce(Result<FinancePageResult, String>) + Send + 'static,
) -> Result<(), String> {
    with_connection(move |connection| {
        connection.procedures.query_finance_page_then(
            query.page.max(1),
            query.page_size.max(1),
            query.keyword,
            query.transaction_type,
            query.park_id,
            query.start_time_micros,
            query.end_time_micros,
            move |_, result| {
                callback(procedure_result(
                    result,
                    |value| value.success,
                    |value| value.message.clone(),
                ));
            },
        );
        Ok(())
    })
}

pub fn query_billing_history(
    query: BillingPageQuery,
    callback: impl FnOnce(Result<BillingPageResult, String>) + Send + 'static,
) -> Result<(), String> {
    with_connection(move |connection| {
        connection.procedures.query_billing_page_then(
            query.page.max(1),
            query.page_size.max(1),
            query.project_keyword,
            query.tenant_keyword,
            query.collection_status,
            query.park_id,
            query.start_time_micros,
            query.end_time_micros,
            move |_, result| {
                callback(procedure_result(
                    result,
                    |value| value.success,
                    |value| value.message.clone(),
                ));
            },
        );
        Ok(())
    })
}

pub fn query_salary_history(
    query: SalaryPageQuery,
    callback: impl FnOnce(Result<SalaryPageResult, String>) + Send + 'static,
) -> Result<(), String> {
    with_connection(move |connection| {
        connection.procedures.query_salary_page_then(
            query.page.max(1),
            query.page_size.max(1),
            query.name_keyword,
            query.phone_keyword,
            query.issued,
            query.department,
            query.start_time_micros,
            query.end_time_micros,
            move |_, result| {
                callback(procedure_result(
                    result,
                    |value| value.success,
                    |value| value.message.clone(),
                ));
            },
        );
        Ok(())
    })
}
