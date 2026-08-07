//! 租赁客户与图片的多对多关系。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = tenant_image,
    index(accessor = tenant_image_by_customer, btree(columns = [customer_id])),
    index(accessor = tenant_image_by_tenant, btree(columns = [rental_tenant_id])),
    index(accessor = tenant_image_by_image, btree(columns = [img_id])),
    index(accessor = tenant_image_by_pair, btree(columns = [rental_tenant_id, img_id]))
)]
pub struct TenantImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub rental_tenant_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
