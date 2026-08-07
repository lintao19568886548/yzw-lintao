//! 员工档案表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `employee` 表。
#[spacetimedb::table(
    accessor = employee,
    index(accessor = employee_by_customer, btree(columns = [customer_id])),
    index(accessor = employee_by_customer_id_number, btree(columns = [customer_id, id_number]))
)]
pub struct Employee {
    #[primary_key]
    #[auto_inc]
    pub employee_id: u64,
    pub customer_id: String,
    pub name: String,
    pub gender: String,
    pub phone: String,
    /// 与租户业务用户的软关联。
    pub user_id: Option<u64>,
    pub age: Option<i32>,
    pub id_number: Option<String>,
    pub address: Option<String>,
    pub education: Option<String>,
    pub department: Option<String>,
    pub hire_date: Option<Timestamp>,
    pub leave_date: Option<Timestamp>,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub is_resigned: bool,
    /// MySQL `time` 转为当天零点后的秒数。
    pub check_in_seconds: Option<u32>,
    /// MySQL `time` 转为当天零点后的秒数。
    pub check_out_seconds: Option<u32>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
