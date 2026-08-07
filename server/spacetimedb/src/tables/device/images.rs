//! 设备台账的图片关系表。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = device_asset_image,
    index(accessor = device_asset_image_by_owner, btree(columns = [asset_id])),
    index(accessor = device_asset_image_by_image, btree(columns = [img_id])),
    index(accessor = device_asset_image_by_pair, btree(columns = [asset_id, img_id]))
)]
pub struct DeviceAssetImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub asset_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
