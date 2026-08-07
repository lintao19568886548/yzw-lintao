//! 租户加入邀请码表。

use spacetimedb::Timestamp;

/// 对应中心库 `tenant_invitation` 表。
#[spacetimedb::table(
    accessor = tenant_invitation,
    index(accessor = tenant_invitation_by_code, btree(columns = [code])),
    index(accessor = tenant_invitation_by_customer, btree(columns = [customer_id])),
    index(accessor = tenant_invitation_by_customer_status, btree(columns = [customer_id, status])),
    index(accessor = tenant_invitation_by_creator, btree(columns = [created_by_center_user_id])),
    index(accessor = tenant_invitation_by_expiry, btree(columns = [expires_at]))
)]
pub struct TenantInvitation {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub code: String,
    pub customer_id: String,
    pub created_by_center_user_id: u64,
    /// 原 MySQL 逗号分隔字段改为目标租户角色 ID 集合。
    pub role_ids: Vec<u64>,
    pub max_uses: Option<u32>,
    pub used_count: u32,
    pub expires_at: Option<Timestamp>,
    pub status: String,
    pub remark: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
