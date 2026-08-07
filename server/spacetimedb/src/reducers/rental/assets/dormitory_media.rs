//! 宿舍主档与 R2 图片元数据的一体化事务。
//!
//! 权限与 `update_dormitory` 一致（园区管理菜单 + 所属园区数据范围）。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::dormitory_reducer::{DormitoryInput, validated_dormitory};
use crate::{
    reducers::{
        access::{require_dormitory, require_image, require_park_access, require_rental_manager},
        platform::media::image_reducer::{delete_image_if_unreferenced, ensure_image},
    },
    tables::*,
};

/// 单个宿舍最多保留的图片数量。
const MAX_DORMITORY_IMAGES: usize = 8;

/// 用户确认保存后已经上传到 R2 的宿舍图片。
#[derive(SpacetimeType)]
pub struct UploadedDormitoryImageInput {
    pub img_url: String,
    pub hash: String,
}

/// 修改宿舍主档并同步替换图片。
#[spacetimedb::reducer]
pub fn update_dormitory_with_images(
    ctx: &ReducerContext,
    dormitory_id: u64,
    input: DormitoryInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedDormitoryImageInput>,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let existing = require_dormitory(ctx, dormitory_id)?;
    require_park_access(ctx, existing.park_id)?;
    require_park_access(ctx, input.park_id)?;
    let mut row = validated_dormitory(ctx, dormitory_id, existing.customer_id.clone(), input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.dormitory_building().dormitory_id().update(row);
    replace_dormitory_images(
        ctx,
        dormitory_id,
        existing.customer_id,
        existing_image_ids,
        uploads,
    )
}

fn replace_dormitory_images(
    ctx: &ReducerContext,
    dormitory_id: u64,
    customer_id: String,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedDormitoryImageInput>,
) -> Result<(), String> {
    if existing_image_ids.len() + uploads.len() > MAX_DORMITORY_IMAGES {
        return Err(format!("每个宿舍最多保留 {MAX_DORMITORY_IMAGES} 张图片"));
    }
    let old_links = ctx
        .db
        .dormitory_image()
        .dormitory_image_by_dormitory()
        .filter(dormitory_id)
        .collect::<Vec<_>>();
    for link in &old_links {
        ctx.db.dormitory_image().id().delete(link.id);
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
        ctx.db.dormitory_image().insert(DormitoryImage {
            id: 0,
            customer_id: customer_id.clone(),
            dormitory_id,
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
