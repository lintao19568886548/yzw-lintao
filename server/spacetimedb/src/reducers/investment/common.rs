//! 招商业务共用的记录、图片和字段校验。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::{current_customer_id, require_image},
        validation::{normalize_optional_text, validate_max_length},
    },
    tables::*,
};

pub(super) fn require_investment(
    ctx: &ReducerContext,
    investment_id: u64,
) -> Result<Investment, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .investment()
        .investment_id()
        .find(investment_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("招商记录不存在".into())
}

pub(super) fn require_investment_tenant(
    ctx: &ReducerContext,
    tenant_id: u64,
) -> Result<InvestmentTenant, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .investment_tenant()
        .tenant_id()
        .find(tenant_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("招商租户收支记录不存在".into())
}

pub(super) fn limited_optional(
    value: Option<String>,
    max_chars: usize,
    message: &'static str,
) -> Result<Option<String>, String> {
    let value = normalize_optional_text(value);
    if let Some(text) = &value {
        validate_max_length(text, max_chars, message)?;
    }
    Ok(value)
}

pub(super) fn validated_images(
    ctx: &ReducerContext,
    image_ids: Vec<u64>,
) -> Result<Vec<u64>, String> {
    let mut ids = BTreeSet::new();
    for img_id in image_ids {
        require_image(ctx, img_id)?;
        ids.insert(img_id);
    }
    Ok(ids.into_iter().collect())
}

pub(super) fn sync_investment_images(
    ctx: &ReducerContext,
    investment_id: u64,
    image_ids: Vec<u64>,
) {
    let old_ids = ctx
        .db
        .investment_image()
        .investment_image_by_investment()
        .filter(investment_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in old_ids {
        ctx.db.investment_image().id().delete(id);
    }
    for img_id in image_ids {
        ctx.db.investment_image().insert(InvestmentImage {
            id: 0,
            investment_id,
            img_id,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
}
