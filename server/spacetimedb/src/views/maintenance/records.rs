//! 当前用户有权访问的设施维保记录及图片关系。

use std::collections::BTreeSet;

use spacetimedb::{SpacetimeType, ViewContext};

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_firefighting_assets, public)]
pub fn my_firefighting_assets(ctx: &ViewContext) -> Vec<FirefightingAsset> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for park_id in scope.parks() {
        rows.extend(
            ctx.db
                .firefighting_asset()
                .firefighting_asset_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted),
        );
    }
    rows.sort_by_key(|row| row.asset_id);
    rows
}

/// 巡检记录没有 `park_id` 列，园区范围经由资产表判定。
#[spacetimedb::view(accessor = my_firefighting_inspections, public)]
pub fn my_firefighting_inspections(ctx: &ViewContext) -> Vec<FirefightingInspection> {
    let assets = my_firefighting_assets(ctx)
        .into_iter()
        .map(|row| row.asset_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for asset_id in assets {
        rows.extend(
            ctx.db
                .firefighting_inspection()
                .firefighting_inspection_by_asset()
                .filter(asset_id),
        );
    }
    rows.sort_by_key(|row| row.inspection_id);
    rows
}

/// 消防台账直接使用的设施图片信息（含 URL）。
#[derive(SpacetimeType)]
pub struct FirefightingAssetImagePreview {
    pub asset_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_firefighting_asset_image_previews, public)]
pub fn my_firefighting_asset_image_previews(
    ctx: &ViewContext,
) -> Vec<FirefightingAssetImagePreview> {
    let owners = my_firefighting_assets(ctx)
        .into_iter()
        .map(|row| row.asset_id)
        .collect::<BTreeSet<_>>();
    let mut previews = Vec::new();
    for owner_id in owners {
        previews.extend(
            ctx.db
                .firefighting_asset_image()
                .firefighting_asset_image_by_owner()
                .filter(owner_id)
                .filter_map(|link| {
                    let image = ctx.db.image().img_id().find(link.img_id)?;
                    Some(FirefightingAssetImagePreview {
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

/// 消防巡检时间线直接使用的现场照片信息（含 URL）。
#[derive(SpacetimeType)]
pub struct FirefightingInspectionImagePreview {
    pub inspection_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_firefighting_inspection_image_previews, public)]
pub fn my_firefighting_inspection_image_previews(
    ctx: &ViewContext,
) -> Vec<FirefightingInspectionImagePreview> {
    let owners = my_firefighting_inspections(ctx)
        .into_iter()
        .map(|row| row.inspection_id)
        .collect::<BTreeSet<_>>();
    let mut previews = Vec::new();
    for owner_id in owners {
        previews.extend(
            ctx.db
                .firefighting_inspection_image()
                .firefighting_inspection_image_by_owner()
                .filter(owner_id)
                .filter_map(|link| {
                    let image = ctx.db.image().img_id().find(link.img_id)?;
                    Some(FirefightingInspectionImagePreview {
                        inspection_id: link.inspection_id,
                        img_id: link.img_id,
                        img_url: image.img_url,
                    })
                }),
        );
    }
    previews.sort_by_key(|row| (row.inspection_id, row.img_id));
    previews
}

#[spacetimedb::view(accessor = my_transformer_assets, public)]
pub fn my_transformer_assets(ctx: &ViewContext) -> Vec<TransformerAsset> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for park_id in scope.parks() {
        rows.extend(
            ctx.db
                .transformer_asset()
                .transformer_asset_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted),
        );
    }
    rows.sort_by_key(|row| row.asset_id);
    rows
}

/// 巡检记录没有 `park_id` 列，园区范围经由资产表判定：
/// 只返回数据范围内在册资产名下的巡检。
#[spacetimedb::view(accessor = my_transformer_inspections, public)]
pub fn my_transformer_inspections(ctx: &ViewContext) -> Vec<TransformerInspection> {
    let assets = my_transformer_assets(ctx)
        .into_iter()
        .map(|row| row.asset_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for asset_id in assets {
        rows.extend(
            ctx.db
                .transformer_inspection()
                .transformer_inspection_by_asset()
                .filter(asset_id),
        );
    }
    rows.sort_by_key(|row| row.inspection_id);
    rows
}

#[spacetimedb::view(accessor = my_factory_maintenance_records, public)]
pub fn my_factory_maintenance_records(ctx: &ViewContext) -> Vec<FactoryMaintenance> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for park_id in scope.parks() {
        rows.extend(
            ctx.db
                .factory_maintenance()
                .factory_maintenance_by_park()
                .filter(park_id),
        );
    }
    rows.sort_by_key(|row| row.factory_maintenance_id);
    rows
}

#[spacetimedb::view(accessor = my_hygiene_checks, public)]
pub fn my_hygiene_checks(ctx: &ViewContext) -> Vec<HygieneCheck> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for park_id in scope.parks() {
        rows.extend(
            ctx.db
                .hygiene_check()
                .hygiene_check_by_park()
                .filter(park_id),
        );
    }
    rows.sort_by_key(|row| row.hygiene_check_id);
    rows
}

#[spacetimedb::view(accessor = my_elevator_assets, public)]
pub fn my_elevator_assets(ctx: &ViewContext) -> Vec<ElevatorAsset> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for park_id in scope.parks() {
        rows.extend(
            ctx.db
                .elevator_asset()
                .elevator_asset_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted),
        );
    }
    rows.sort_by_key(|row| row.asset_id);
    rows
}

/// 巡检记录没有 `park_id` 列，园区范围经由资产表判定。
#[spacetimedb::view(accessor = my_elevator_inspections, public)]
pub fn my_elevator_inspections(ctx: &ViewContext) -> Vec<ElevatorInspection> {
    let assets = my_elevator_assets(ctx)
        .into_iter()
        .map(|row| row.asset_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for asset_id in assets {
        rows.extend(
            ctx.db
                .elevator_inspection()
                .elevator_inspection_by_asset()
                .filter(asset_id),
        );
    }
    rows.sort_by_key(|row| row.inspection_id);
    rows
}

/// 电梯台账直接使用的设备图片信息（含 URL）。
#[derive(SpacetimeType)]
pub struct ElevatorAssetImagePreview {
    pub asset_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_elevator_asset_image_previews, public)]
pub fn my_elevator_asset_image_previews(ctx: &ViewContext) -> Vec<ElevatorAssetImagePreview> {
    let owners = my_elevator_assets(ctx)
        .into_iter()
        .map(|row| row.asset_id)
        .collect::<BTreeSet<_>>();
    let mut previews = Vec::new();
    for owner_id in owners {
        previews.extend(
            ctx.db
                .elevator_asset_image()
                .elevator_asset_image_by_owner()
                .filter(owner_id)
                .filter_map(|link| {
                    let image = ctx.db.image().img_id().find(link.img_id)?;
                    Some(ElevatorAssetImagePreview {
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

/// 电梯巡检时间线直接使用的现场照片信息（含 URL）。
#[derive(SpacetimeType)]
pub struct ElevatorInspectionImagePreview {
    pub inspection_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_elevator_inspection_image_previews, public)]
pub fn my_elevator_inspection_image_previews(
    ctx: &ViewContext,
) -> Vec<ElevatorInspectionImagePreview> {
    let owners = my_elevator_inspections(ctx)
        .into_iter()
        .map(|row| row.inspection_id)
        .collect::<BTreeSet<_>>();
    let mut previews = Vec::new();
    for owner_id in owners {
        previews.extend(
            ctx.db
                .elevator_inspection_image()
                .elevator_inspection_image_by_owner()
                .filter(owner_id)
                .filter_map(|link| {
                    let image = ctx.db.image().img_id().find(link.img_id)?;
                    Some(ElevatorInspectionImagePreview {
                        inspection_id: link.inspection_id,
                        img_id: link.img_id,
                        img_url: image.img_url,
                    })
                }),
        );
    }
    previews.sort_by_key(|row| (row.inspection_id, row.img_id));
    previews
}

#[spacetimedb::view(accessor = my_repair_orders, public)]
pub fn my_repair_orders(ctx: &ViewContext) -> Vec<RepairOrder> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for park_id in scope.parks() {
        rows.extend(ctx.db.repair_order().repair_order_by_park().filter(park_id));
    }
    rows.sort_by_key(|row| row.repair_order_id);
    rows
}

/// 变压器台账直接使用的设备图片信息（含 URL），避免客户端再订阅图片主表。
#[derive(SpacetimeType)]
pub struct TransformerAssetImagePreview {
    pub asset_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_transformer_asset_image_previews, public)]
pub fn my_transformer_asset_image_previews(ctx: &ViewContext) -> Vec<TransformerAssetImagePreview> {
    let owners = my_transformer_assets(ctx)
        .into_iter()
        .map(|row| row.asset_id)
        .collect::<BTreeSet<_>>();
    let mut previews = Vec::new();
    for owner_id in owners {
        previews.extend(
            ctx.db
                .transformer_asset_image()
                .transformer_asset_image_by_owner()
                .filter(owner_id)
                .filter_map(|link| {
                    let image = ctx.db.image().img_id().find(link.img_id)?;
                    Some(TransformerAssetImagePreview {
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

/// 巡检时间线直接使用的现场照片信息（含 URL）。
#[derive(SpacetimeType)]
pub struct TransformerInspectionImagePreview {
    pub inspection_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_transformer_inspection_image_previews, public)]
pub fn my_transformer_inspection_image_previews(
    ctx: &ViewContext,
) -> Vec<TransformerInspectionImagePreview> {
    let owners = my_transformer_inspections(ctx)
        .into_iter()
        .map(|row| row.inspection_id)
        .collect::<BTreeSet<_>>();
    let mut previews = Vec::new();
    for owner_id in owners {
        previews.extend(
            ctx.db
                .transformer_inspection_image()
                .transformer_inspection_image_by_owner()
                .filter(owner_id)
                .filter_map(|link| {
                    let image = ctx.db.image().img_id().find(link.img_id)?;
                    Some(TransformerInspectionImagePreview {
                        inspection_id: link.inspection_id,
                        img_id: link.img_id,
                        img_url: image.img_url,
                    })
                }),
        );
    }
    previews.sort_by_key(|row| (row.inspection_id, row.img_id));
    previews
}
