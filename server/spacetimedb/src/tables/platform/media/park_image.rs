//! 园区与图片的多对多关系。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = park_image,
    index(accessor = park_image_by_customer, btree(columns = [customer_id])),
    index(accessor = park_image_by_park, btree(columns = [park_id])),
    index(accessor = park_image_by_image, btree(columns = [img_id])),
    index(accessor = park_image_by_pair, btree(columns = [park_id, img_id]))
)]
pub struct ParkImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub park_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
