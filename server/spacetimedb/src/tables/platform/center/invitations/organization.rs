//! 组织成员邀请码表。

use spacetimedb::Timestamp;

/// 对应中心库 `organization_invitation` 表。
#[spacetimedb::table(
    accessor = organization_invitation,
    index(accessor = organization_invitation_by_code, btree(columns = [code])),
    index(accessor = organization_invitation_by_org, btree(columns = [organization_id])),
    index(accessor = organization_invitation_by_org_status, btree(columns = [organization_id, status])),
    index(accessor = organization_invitation_by_creator, btree(columns = [created_by_center_user_id])),
    index(accessor = organization_invitation_by_expiry, btree(columns = [expires_at]))
)]
pub struct OrganizationInvitation {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub organization_id: u64,
    pub code: String,
    pub created_by_center_user_id: u64,
    /// 原 MySQL 逗号分隔字段改为强类型角色 ID 集合。
    pub role_ids: Vec<u64>,
    pub max_uses: Option<u32>,
    pub used_count: u32,
    pub expires_at: Option<Timestamp>,
    pub status: String,
    pub remark: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
