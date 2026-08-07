//! 合同主记录与 R2 图片元数据的一体化事务。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::tenant::upsert_tenant_with_links;
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_image, require_rental_tenant},
        platform::media::image_reducer::{delete_image_if_unreferenced, ensure_image},
    },
    tables::*,
};

/// 用户确认保存后已经上传到 R2 的合同图片。
#[derive(SpacetimeType)]
pub struct UploadedTenantImageInput {
    pub img_url: String,
    pub hash: String,
}

#[spacetimedb::reducer]
pub fn create_rental_tenant_with_images(
    ctx: &ReducerContext,
    input: super::tenant::RentalTenantInput,
    uploads: Vec<UploadedTenantImageInput>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let rental_tenant_id = upsert_tenant_with_links(ctx, 0, customer_id.clone(), input)?;
    replace_tenant_images(ctx, rental_tenant_id, customer_id, Vec::new(), uploads)
}

#[spacetimedb::reducer]
pub fn update_rental_tenant_with_images(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    input: super::tenant::RentalTenantInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedTenantImageInput>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_rental_tenant(ctx, rental_tenant_id)?;
    upsert_tenant_with_links(ctx, rental_tenant_id, existing.customer_id.clone(), input)?;
    replace_tenant_images(
        ctx,
        rental_tenant_id,
        existing.customer_id,
        existing_image_ids,
        uploads,
    )
}

fn replace_tenant_images(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    customer_id: String,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedTenantImageInput>,
) -> Result<(), String> {
    if existing_image_ids.len() + uploads.len() > 8 {
        return Err("每份合同最多保留 8 张图片".into());
    }
    let old_links = ctx
        .db
        .tenant_image()
        .tenant_image_by_tenant()
        .filter(rental_tenant_id)
        .collect::<Vec<_>>();
    for link in &old_links {
        ctx.db.tenant_image().id().delete(link.id);
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
        ctx.db.tenant_image().insert(TenantImage {
            id: 0,
            customer_id: customer_id.clone(),
            rental_tenant_id,
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
