//! 租户邀请码加入结果表。

use spacetimedb::Timestamp;

/// 对应中心库 `tenant_invitation_join_log` 表。
#[spacetimedb::table(
    accessor = tenant_invitation_join_log,
    index(accessor = tenant_join_log_by_invitation, btree(columns = [invitation_id])),
    index(accessor = tenant_join_log_by_invitation_user, btree(columns = [invitation_id, center_user_id])),
    index(accessor = tenant_join_log_by_center_user, btree(columns = [center_user_id])),
    index(accessor = tenant_join_log_by_customer, btree(columns = [customer_id])),
    index(accessor = tenant_join_log_by_status, btree(columns = [status]))
)]
pub struct TenantInvitationJoinLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub invitation_id: u64,
    pub code: String,
    pub customer_id: String,
    pub center_user_id: u64,
    pub customer_user_id: Option<u64>,
    pub previous_customer_id: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub joined_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
