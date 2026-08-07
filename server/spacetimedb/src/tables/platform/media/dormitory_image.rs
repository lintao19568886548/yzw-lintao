//! 宿舍与图片的多对多关系。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = dormitory_image,
    index(accessor = dormitory_image_by_customer, btree(columns = [customer_id])),
    index(accessor = dormitory_image_by_dormitory, btree(columns = [dormitory_id])),
    index(accessor = dormitory_image_by_image, btree(columns = [img_id])),
    index(accessor = dormitory_image_by_pair, btree(columns = [dormitory_id, img_id]))
)]
pub struct DormitoryImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub dormitory_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
