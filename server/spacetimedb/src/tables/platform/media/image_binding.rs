//! 通用业务图片绑定关系表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `image_binding` 表。
#[spacetimedb::table(
    accessor = image_binding,
    index(accessor = image_binding_by_customer, btree(columns = [customer_id])),
    index(accessor = image_binding_by_business, btree(columns = [biz_type, biz_id])),
    index(accessor = image_binding_by_business_field, btree(columns = [biz_type, biz_id, field_key])),
    index(accessor = image_binding_by_pair, btree(columns = [biz_type, biz_id, img_id])),
    index(accessor = image_binding_by_image, btree(columns = [img_id]))
)]
pub struct ImageBinding {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    /// SpacetimeDB 共库运行时用于隔离原 MySQL 物理租户库。
    pub customer_id: String,
    pub biz_type: String,
    pub biz_id: u64,
    pub img_id: u64,
    /// 空字符串对应原 MySQL 可空的 `field` 字段。
    pub field_key: String,
    pub sort: i32,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
