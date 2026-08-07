//! 工资记录与 R2 图片元数据的一体化事务。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::salary::{sync_salary_finance, validated_salary};
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_image, require_salary},
        platform::media::image_reducer::{delete_image_if_unreferenced, ensure_image},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct UploadedSalaryImageInput {
    pub img_url: String,
    pub hash: String,
}

#[spacetimedb::reducer]
pub fn create_salary_with_images(
    ctx: &ReducerContext,
    input: super::salary::SalaryInput,
    uploads: Vec<UploadedSalaryImageInput>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let salary = ctx
        .db
        .employee_salary()
        .insert(validated_salary(ctx, 0, customer_id.clone(), input)?);
    sync_salary_finance(ctx, &salary);
    replace_salary_images(ctx, salary.salary_id, customer_id, Vec::new(), uploads)
}

#[spacetimedb::reducer]
pub fn update_salary_with_images(
    ctx: &ReducerContext,
    salary_id: u64,
    input: super::salary::SalaryInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedSalaryImageInput>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_salary(ctx, salary_id)?;
    let mut salary = validated_salary(ctx, salary_id, existing.customer_id.clone(), input)?;
    salary.created_at = existing.created_at;
    salary.updated_at = Some(ctx.timestamp);
    let salary = ctx.db.employee_salary().salary_id().update(salary);
    sync_salary_finance(ctx, &salary);
    replace_salary_images(
        ctx,
        salary_id,
        existing.customer_id,
        existing_image_ids,
        uploads,
    )
}

fn replace_salary_images(
    ctx: &ReducerContext,
    salary_id: u64,
    customer_id: String,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedSalaryImageInput>,
) -> Result<(), String> {
    let old_links = ctx
        .db
        .salary_image()
        .salary_image_by_salary()
        .filter(salary_id)
        .collect::<Vec<_>>();
    for link in &old_links {
        ctx.db.salary_image().id().delete(link.id);
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
        ctx.db.salary_image().insert(SalaryImage {
            id: 0,
            customer_id: customer_id.clone(),
            salary_id,
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
