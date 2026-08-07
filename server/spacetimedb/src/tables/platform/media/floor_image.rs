//! 厂房楼层与图片的多对多关系。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = factory_floor_image,
    index(accessor = floor_image_by_customer, btree(columns = [customer_id])),
    index(accessor = floor_image_by_floor, btree(columns = [floor_id])),
    index(accessor = floor_image_by_image, btree(columns = [img_id])),
    index(accessor = floor_image_by_pair, btree(columns = [floor_id, img_id]))
)]
pub struct FactoryFloorImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub floor_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
