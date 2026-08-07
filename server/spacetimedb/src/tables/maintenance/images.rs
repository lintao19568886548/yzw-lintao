//! 消防设施和变压器的图片关系表。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = firefighting_asset_image,
    index(accessor = firefighting_asset_image_by_owner, btree(columns = [asset_id])),
    index(accessor = firefighting_asset_image_by_image, btree(columns = [img_id])),
    index(accessor = firefighting_asset_image_by_pair, btree(columns = [asset_id, img_id]))
)]
pub struct FirefightingAssetImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub asset_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

#[spacetimedb::table(
    accessor = firefighting_inspection_image,
    index(accessor = firefighting_inspection_image_by_owner, btree(columns = [inspection_id])),
    index(accessor = firefighting_inspection_image_by_image, btree(columns = [img_id])),
    index(accessor = firefighting_inspection_image_by_pair, btree(columns = [inspection_id, img_id]))
)]
pub struct FirefightingInspectionImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub inspection_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

#[spacetimedb::table(
    accessor = elevator_asset_image,
    index(accessor = elevator_asset_image_by_owner, btree(columns = [asset_id])),
    index(accessor = elevator_asset_image_by_image, btree(columns = [img_id])),
    index(accessor = elevator_asset_image_by_pair, btree(columns = [asset_id, img_id]))
)]
pub struct ElevatorAssetImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub asset_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

#[spacetimedb::table(
    accessor = elevator_inspection_image,
    index(accessor = elevator_inspection_image_by_owner, btree(columns = [inspection_id])),
    index(accessor = elevator_inspection_image_by_image, btree(columns = [img_id])),
    index(accessor = elevator_inspection_image_by_pair, btree(columns = [inspection_id, img_id]))
)]
pub struct ElevatorInspectionImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub inspection_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

#[spacetimedb::table(
    accessor = transformer_asset_image,
    index(accessor = transformer_asset_image_by_owner, btree(columns = [asset_id])),
    index(accessor = transformer_asset_image_by_image, btree(columns = [img_id])),
    index(accessor = transformer_asset_image_by_pair, btree(columns = [asset_id, img_id]))
)]
pub struct TransformerAssetImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub asset_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

#[spacetimedb::table(
    accessor = transformer_inspection_image,
    index(accessor = transformer_inspection_image_by_owner, btree(columns = [inspection_id])),
    index(accessor = transformer_inspection_image_by_image, btree(columns = [img_id])),
    index(accessor = transformer_inspection_image_by_pair, btree(columns = [inspection_id, img_id]))
)]
pub struct TransformerInspectionImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub inspection_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

#[spacetimedb::table(
    accessor = firefighting_image,
    index(accessor = firefighting_image_by_owner, btree(columns = [firefighting_id])),
    index(accessor = firefighting_image_by_image, btree(columns = [img_id])),
    index(accessor = firefighting_image_by_pair, btree(columns = [firefighting_id, img_id]))
)]
pub struct FirefightingImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub firefighting_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

#[spacetimedb::table(
    accessor = transformer_image,
    index(accessor = transformer_image_by_owner, btree(columns = [transformer_id])),
    index(accessor = transformer_image_by_image, btree(columns = [img_id])),
    index(accessor = transformer_image_by_pair, btree(columns = [transformer_id, img_id]))
)]
pub struct TransformerImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub transformer_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
