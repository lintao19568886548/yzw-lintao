//! 园区主档与 R2 图片元数据的一体化事务。
//!
//! 权限与 `update_park` 保持一致（园区管理菜单 + 园区数据范围），而不是像
//! 早期的合同图片那样要求全站管理员——园区图片本来就该由园区管理员维护。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::{ParkInput, records::validated_park};
use crate::{
    reducers::{
        access::{
            current_customer_id, require_image, require_park, require_park_access,
            require_rental_manager,
        },
        platform::media::image_reducer::{delete_image_if_unreferenced, ensure_image},
    },
    tables::*,
};

/// 单个园区最多保留的图片数量，与合同、工资等业务口径一致。
const MAX_PARK_IMAGES: usize = 8;

/// 用户确认保存后已经上传到 R2 的园区图片。
#[derive(SpacetimeType)]
pub struct UploadedParkImageInput {
    pub img_url: String,
    pub hash: String,
}

/// 建档并写入园区图片，两者在同一事务内完成。
///
/// 不走「先 `create_park` 再补图片」两步：自增出来的 `park_id` 要落库之后
/// 才拿得到，前端分两步写就必须先建后查再补，中间任何一步失败都会留下一个
/// 没有图片的园区，而用户已经选好图并点了保存。
#[spacetimedb::reducer]
pub fn create_park_with_images(
    ctx: &ReducerContext,
    input: ParkInput,
    uploads: Vec<UploadedParkImageInput>,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_park(ctx, 0, customer_id.clone(), input)?;
    let park_id = ctx.db.park().insert(row).park_id;
    replace_park_images(ctx, park_id, customer_id, Vec::new(), uploads)
}

/// 修改园区主档并同步替换图片，两者在同一事务内完成。
#[spacetimedb::reducer]
pub fn update_park_with_images(
    ctx: &ReducerContext,
    park_id: u64,
    input: ParkInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedParkImageInput>,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    require_park_access(ctx, park_id)?;
    let existing = require_park(ctx, park_id)?;
    let mut row = validated_park(ctx, park_id, existing.customer_id.clone(), input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.park().park_id().update(row);
    replace_park_images(
        ctx,
        park_id,
        existing.customer_id,
        existing_image_ids,
        uploads,
    )
}

fn replace_park_images(
    ctx: &ReducerContext,
    park_id: u64,
    customer_id: String,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedParkImageInput>,
) -> Result<(), String> {
    if existing_image_ids.len() + uploads.len() > MAX_PARK_IMAGES {
        return Err(format!("每个园区最多保留 {MAX_PARK_IMAGES} 张图片"));
    }
    let old_links = ctx
        .db
        .park_image()
        .park_image_by_park()
        .filter(park_id)
        .collect::<Vec<_>>();
    for link in &old_links {
        ctx.db.park_image().id().delete(link.id);
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
        ctx.db.park_image().insert(ParkImage {
            id: 0,
            customer_id: customer_id.clone(),
            park_id,
            img_id: *img_id,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    // 被移出园区且没有其他业务引用的图片一并清理，避免 R2 元数据堆积。
    for img_id in old_links.into_iter().map(|link| link.img_id) {
        if !image_ids.contains(&img_id) {
            delete_image_if_unreferenced(ctx, img_id);
        }
    }
    Ok(())
}
