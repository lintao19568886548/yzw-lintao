//! 员工工资记录表。
//!
//! 表名是 `employee_salary` 而不是 `salary`：归属对象从租赁合同方改成
//! 员工是一次破坏性的列变更，SpacetimeDB 不允许在已有表上增删列（哪怕
//! 表是空的）。换个表名后这次变更变成「删掉空的旧表 + 建新表」，两步
//! 都在自动迁移允许的范围内，一次发布即可完成。
//!
//! 原 MySQL 结构把工资挂在 `rental_tenant`（租赁合同方）下，收款人只能
//! 从合同里选，页面上因此出现「给某某有限公司发工资」这类讲不通的记录。
//! 工资发给员工，归属对象应当是员工档案，这里改为 `employee_id`。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = employee_salary,
    index(accessor = employee_salary_by_customer, btree(columns = [customer_id])),
    index(accessor = employee_salary_by_employee, btree(columns = [employee_id]))
)]
pub struct Salary {
    #[primary_key]
    #[auto_inc]
    pub salary_id: u64,
    pub customer_id: String,
    /// 领取这笔工资的员工。
    pub employee_id: u64,
    pub salary_amount_cents: Option<i64>,
    pub issue_date: Option<Timestamp>,
    pub issued: Option<bool>,
    pub remark: Option<String>,
    /// 仅用于兼容 MySQL 历史数据；当前删除 Reducer 会物理删除整行。
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
