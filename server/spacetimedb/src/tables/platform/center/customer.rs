//! 客户/租户及中心用户到租户业务用户的映射。

use spacetimedb::Timestamp;

/// 对应中心库 `customer` 表，每条记录代表一个租户空间。
#[spacetimedb::table(accessor = customer)]
pub struct Customer {
    #[primary_key]
    pub customer_id: String,
    pub name: String,
    pub city: Option<String>,
    pub company_short_name: Option<String>,
    pub code: Option<String>,
    pub status: i8,
    pub db_name: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

/// 对应中心库 `user_tenant_mapping` 表。
///
/// 同一中心用户在每个租户只能映射一个业务用户，同一租户业务用户也只能
/// 归属于一个中心用户；两组组合唯一约束由 reducer 显式维护。
#[spacetimedb::table(
    accessor = user_tenant_mapping,
    index(accessor = mapping_by_center_user, btree(columns = [center_user_id])),
    index(accessor = mapping_by_customer, btree(columns = [customer_id])),
    index(accessor = mapping_by_center_customer, btree(columns = [center_user_id, customer_id])),
    index(accessor = mapping_by_customer_user, btree(columns = [customer_id, customer_user_id]))
)]
pub struct UserTenantMapping {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub center_user_id: u64,
    pub customer_id: String,
    pub customer_user_id: u64,
    pub db_name: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
