//! 财务流水附件维护。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_finance},
        validation::required_text,
    },
    tables::*,
};

#[spacetimedb::reducer]
pub fn add_finance_image(ctx: &ReducerContext, finance_id: u64, url: String) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_finance(ctx, finance_id)?;
    let url = required_text(url, "财务附件地址不能为空")?;
    if ctx
        .db
        .finance_image()
        .finance_image_by_finance()
        .filter(finance_id)
        .any(|image| image.url == url)
    {
        return Err("财务流水已经包含该附件".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.finance_image().insert(FinanceImage {
        id: 0,
        customer_id,
        finance_id,
        url,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn remove_finance_image(
    ctx: &ReducerContext,
    finance_id: u64,
    image_id: u64,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_finance(ctx, finance_id)?;
    let image = ctx
        .db
        .finance_image()
        .id()
        .find(image_id)
        .filter(|image| image.finance_id == finance_id)
        .ok_or("财务附件不存在")?;
    ctx.db.finance_image().id().delete(image.id);
    Ok(())
}

pub(super) fn delete_finance_image_links(ctx: &ReducerContext, finance_id: u64) {
    let ids = ctx
        .db
        .finance_image()
        .finance_image_by_finance()
        .filter(finance_id)
        .map(|image| image.id)
        .collect::<Vec<_>>();
    for id in ids {
        ctx.db.finance_image().id().delete(id);
    }
}

pub(super) fn replace_finance_images(
    ctx: &ReducerContext,
    finance_id: u64,
    customer_id: String,
    urls: Vec<String>,
) -> Result<(), String> {
    if urls.len() > 8 {
        return Err("每条财务流水最多上传 8 张凭证".into());
    }
    let mut normalized = Vec::with_capacity(urls.len());
    let mut seen = BTreeSet::new();
    for url in urls {
        let url = required_text(url, "财务附件地址不能为空")?;
        if seen.insert(url.clone()) {
            normalized.push(url);
        }
    }
    delete_finance_image_links(ctx, finance_id);
    for url in normalized {
        ctx.db.finance_image().insert(FinanceImage {
            id: 0,
            customer_id: customer_id.clone(),
            finance_id,
            url,
        });
    }
    Ok(())
}
