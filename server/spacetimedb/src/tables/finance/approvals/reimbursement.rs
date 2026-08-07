//! 报销申请及图片关系表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `reimbursement` 表。
#[spacetimedb::table(
    accessor = reimbursement,
    index(accessor = reimbursement_by_customer, btree(columns = [customer_id])),
    index(accessor = reimbursement_by_park, btree(columns = [park_id])),
    index(accessor = reimbursement_by_status, btree(columns = [status])),
    index(accessor = reimbursement_by_created, btree(columns = [created_at]))
)]
pub struct Reimbursement {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub purpose: String,
    pub amount_cents: i64,
    pub payee: String,
    pub reimbursement_date: Timestamp,
    pub department: Option<String>,
    pub username: Option<String>,
    pub remark: Option<String>,
    /// `0` 待审批、`1` 已批准、`2` 已驳回。
    pub status: i8,
    pub is_deleted: bool,
    pub user_id: Option<u64>,
    /// 园区外键，必填，见 `reducers::shared::park_ref`。
    pub park_id: u64,
    pub audit_opinion: Option<String>,
    pub claimant: Option<String>,
    /// SpacetimeDB 使用显式关系替代原项目依赖备注字符串查找财务流水。
    pub finance_id: Option<u64>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

/// 对应 MySQL 业务库中的 `reimbursement_image` 表。
#[spacetimedb::table(
    accessor = reimbursement_image,
    index(accessor = reimbursement_image_by_reimbursement, btree(columns = [reimbursement_id])),
    index(accessor = reimbursement_image_by_image, btree(columns = [img_id])),
    index(accessor = reimbursement_image_by_pair, btree(columns = [reimbursement_id, img_id]))
)]
pub struct ReimbursementImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub reimbursement_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
