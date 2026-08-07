//! 请假申请表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `leave_application` 表。
#[spacetimedb::table(
    accessor = leave_application,
    index(accessor = leave_application_by_customer, btree(columns = [customer_id])),
    index(accessor = leave_application_by_park, btree(columns = [park_id])),
    index(accessor = leave_application_by_status, btree(columns = [status])),
    index(accessor = leave_application_by_created, btree(columns = [created_at]))
)]
pub struct LeaveApplication {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub start_date: Timestamp,
    pub end_date: Timestamp,
    pub leave_type: String,
    pub reason: String,
    /// `0` 待审批、`1` 已批准、`2` 已驳回。
    pub status: i8,
    pub reply: Option<String>,
    pub username: Option<String>,
    pub park_name: Option<String>,
    pub applicant_name: Option<String>,
    pub audit_user_name: Option<String>,
    pub user_id: Option<u64>,
    /// 园区外键，必填，见 `reducers::shared::park_ref`。
    pub park_id: u64,
    pub audit_user_id: Option<u64>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
