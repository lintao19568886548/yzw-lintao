//! 工资发放与财务流水的关联。
//!
//! 工资标记「已发放」时会在财务流水自动记一笔「工资支出」，这张表记录两边的
//! 对应关系，改金额、取消发放、删除工资时靠它找到那笔流水去同步或冲销。
//! 做成关联表而不是在工资表上加 `finance_id` 列，是因为 SpacetimeDB 改列
//! 必须清库重建，新增表则可以保留数据自动迁移。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = salary_finance,
    index(accessor = salary_finance_by_customer, btree(columns = [customer_id])),
    index(accessor = salary_finance_by_salary, btree(columns = [salary_id])),
    index(accessor = salary_finance_by_finance, btree(columns = [finance_id]))
)]
pub struct SalaryFinance {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub salary_id: u64,
    pub finance_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
