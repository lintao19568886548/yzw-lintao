//! 组织、组织成员和组织租户映射。

use spacetimedb::Timestamp;

/// 对应中心库 `organization` 表。
#[spacetimedb::table(
    accessor = organization,
    index(accessor = organization_by_source, btree(columns = [source_customer_id])),
    index(accessor = organization_by_status, btree(columns = [status]))
)]
pub struct Organization {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub name: String,
    pub city: Option<String>,
    pub company_short_name: Option<String>,
    pub source_customer_id: String,
    pub status: String,
    pub created_by_center_user_id: Option<u64>,
    pub legacy: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

/// 对应中心库 `organization_member` 表。
#[spacetimedb::table(
    accessor = organization_member,
    index(accessor = member_by_organization, btree(columns = [organization_id])),
    index(accessor = member_by_center_user, btree(columns = [center_user_id])),
    index(accessor = member_by_pair, btree(columns = [organization_id, center_user_id]))
)]
pub struct OrganizationMember {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub organization_id: u64,
    pub center_user_id: u64,
    pub source_customer_id: String,
    pub source_user_id: Option<u64>,
    pub member_role: String,
    pub status: String,
    pub joined_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

/// 对应中心库 `organization_tenant_mapping` 表。
#[spacetimedb::table(
    accessor = organization_tenant_mapping,
    index(accessor = organization_tenant_by_org, btree(columns = [organization_id]))
)]
pub struct OrganizationTenantMapping {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub organization_id: u64,
    #[unique]
    pub target_customer_id: String,
    pub target_db_name: Option<String>,
    pub tenant_provisioning_job_id: Option<u64>,
    pub status: String,
    pub legacy: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
