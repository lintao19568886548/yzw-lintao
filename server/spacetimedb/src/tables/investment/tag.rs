//! 企业画像标签表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `enterprise_tag` 表。
#[spacetimedb::table(
    accessor = enterprise_tag,
    index(accessor = enterprise_tag_by_customer, btree(columns = [customer_id])),
    index(accessor = enterprise_tag_by_profile, btree(columns = [profile_id])),
    index(accessor = enterprise_tag_by_enterprise, btree(columns = [enterprise_id])),
    index(accessor = enterprise_tag_by_profile_key, btree(columns = [profile_id, tag_type, tag_name])),
    index(accessor = enterprise_tag_by_type, btree(columns = [tag_type])),
    index(accessor = enterprise_tag_by_name, btree(columns = [tag_name]))
)]
pub struct EnterpriseTag {
    #[primary_key]
    #[auto_inc]
    pub tag_id: u64,
    pub customer_id: String,
    /// 显式指向企业画像，替代旧库仅依赖企业名称的隐式关系。
    pub profile_id: u64,
    pub enterprise_id: Option<u64>,
    pub company_name: String,
    pub tag_type: String,
    pub tag_name: String,
    pub tag_source: String,
    pub confidence_score: i32,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
