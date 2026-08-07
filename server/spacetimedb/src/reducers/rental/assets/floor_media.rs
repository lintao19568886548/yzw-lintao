//! 楼层主档与 R2 图片元数据的一体化事务。
//!
//! 与 `update_factory_floor` 权限一致（园区管理菜单 + 所属园区数据范围），
//! 图片替换和字段更新在同一事务内完成。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::floor_reducer::{FactoryFloorInput, validated_floor};
use crate::{
    reducers::{
        access::{
            require_factory, require_factory_floor, require_image, require_park_access,
            require_rental_manager,
        },
        platform::media::image_reducer::{delete_image_if_unreferenced, ensure_image},
        park_ref::has_park,
    },
    tables::*,
};

/// 单个楼层最多保留的图片数量。
const MAX_FLOOR_IMAGES: usize = 8;

/// 用户确认保存后已经上传到 R2 的楼层图片。
#[derive(SpacetimeType)]
pub struct UploadedFloorImageInput {
    pub img_url: String,
    pub hash: String,
}

/// 修改楼层主档并同步替换图片。
#[spacetimedb::reducer]
pub fn update_factory_floor_with_images(
    ctx: &ReducerContext,
    floor_id: u64,
    input: FactoryFloorInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedFloorImageInput>,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let existing = require_factory_floor(ctx, floor_id)?;
    let factory = require_factory(ctx, existing.factory_id)?;
    if has_park(factory.park_id) {
        require_park_access(ctx, factory.park_id)?;
    }
    let mut row = validated_floor(
        ctx,
        floor_id,
        existing.factory_id,
        existing.customer_id.clone(),
        input,
    )?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.factory_floor().floor_id().update(row);
    replace_floor_images(
        ctx,
        floor_id,
        existing.customer_id,
        existing_image_ids,
        uploads,
    )
}

fn replace_floor_images(
    ctx: &ReducerContext,
    floor_id: u64,
    customer_id: String,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedFloorImageInput>,
) -> Result<(), String> {
    if existing_image_ids.len() + uploads.len() > MAX_FLOOR_IMAGES {
        return Err(format!("每个楼层最多保留 {MAX_FLOOR_IMAGES} 张图片"));
    }
    let old_links = ctx
        .db
        .factory_floor_image()
        .floor_image_by_floor()
        .filter(floor_id)
        .collect::<Vec<_>>();
    for link in &old_links {
        ctx.db.factory_floor_image().id().delete(link.id);
    }

    let mut image_ids = BTreeSet::new();
    for img_id in existing_image_ids {
        require_image(ctx, img_id)?;
        image_ids.insert(img_id);
    }
    for upload in uploads {
        image_ids.insert(ensure_image(
            ctx,
            customer_id.clone(),
            upload.img_url,
            upload.hash,
        )?);
    }
    for img_id in &image_ids {
        ctx.db.factory_floor_image().insert(FactoryFloorImage {
            id: 0,
            customer_id: customer_id.clone(),
            floor_id,
            img_id: *img_id,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    for img_id in old_links.into_iter().map(|link| link.img_id) {
        if !image_ids.contains(&img_id) {
            delete_image_if_unreferenced(ctx, img_id);
        }
    }
    Ok(())
}
