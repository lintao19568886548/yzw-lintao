//! 当前用户有权访问的设备台账及图片关系。

use std::collections::BTreeSet;

use spacetimedb::{SpacetimeType, ViewContext};

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_device_assets, public)]
pub fn my_device_assets(ctx: &ViewContext) -> Vec<DeviceAsset> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for park_id in scope.parks() {
        rows.extend(
            ctx.db
                .device_asset()
                .device_asset_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted),
        );
    }
    rows.sort_by_key(|row| row.asset_id);
    rows
}

/// 设备台账直接使用的现场照片信息（含 URL），避免客户端再订阅图片主表。
#[derive(SpacetimeType)]
pub struct DeviceAssetImagePreview {
    pub asset_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_device_asset_image_previews, public)]
pub fn my_device_asset_image_previews(ctx: &ViewContext) -> Vec<DeviceAssetImagePreview> {
    let owners = my_device_assets(ctx)
        .into_iter()
        .map(|row| row.asset_id)
        .collect::<BTreeSet<_>>();
    let mut previews = Vec::new();
    for owner_id in owners {
        previews.extend(
            ctx.db
                .device_asset_image()
                .device_asset_image_by_owner()
                .filter(owner_id)
                .filter_map(|link| {
                    let image = ctx.db.image().img_id().find(link.img_id)?;
                    Some(DeviceAssetImagePreview {
                        asset_id: link.asset_id,
                        img_id: link.img_id,
                        img_url: image.img_url,
                    })
                }),
        );
    }
    previews.sort_by_key(|row| (row.asset_id, row.img_id));
    previews
}

#[spacetimedb::view(accessor = my_edge_gateways, public)]
pub fn my_edge_gateways(ctx: &ViewContext) -> Vec<EdgeGateway> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for park_id in scope.parks() {
        rows.extend(
            ctx.db
                .edge_gateway()
                .edge_gateway_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted),
        );
    }
    rows.sort_by_key(|row| row.gateway_id);
    rows
}
