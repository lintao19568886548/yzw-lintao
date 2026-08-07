//! 共享图片登记逻辑。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::required_text,
    },
    tables::*,
};

/// 按租户和文件哈希去重；同一文件迁移到 R2 时更新原图片地址。
#[spacetimedb::reducer]
pub fn register_image(ctx: &ReducerContext, img_url: String, hash: String) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ensure_image(ctx, customer_id, img_url, hash).map(|_| ())
}

/// 登记或复用当前租户下的图片，并返回后续关系表需要的主键。
pub(crate) fn ensure_image(
    ctx: &ReducerContext,
    customer_id: String,
    img_url: String,
    hash: String,
) -> Result<u64, String> {
    let img_url = required_text(img_url, "图片地址不能为空")?;
    let hash = required_text(hash, "图片哈希不能为空")?;
    if let Some(mut existing) = ctx
        .db
        .image()
        .image_by_customer_hash()
        .filter((customer_id.as_str(), hash.as_str()))
        .next()
    {
        let img_id = existing.img_id;
        if existing.img_url != img_url {
            if !is_content_addressed_r2_url(&img_url, &hash) {
                return Err("相同哈希已经登记其他图片地址".into());
            }
            // MySQL 历史数据保存的是 /uploads 地址；迁移后复用原主键，
            // 让所有既有业务关系同时指向新的 R2 对象。
            existing.img_url = img_url;
            existing.updated_at = Some(ctx.timestamp);
            ctx.db.image().img_id().update(existing);
        }
        return Ok(img_id);
    }
    let image = ctx.db.image().insert(Image {
        img_id: 0,
        customer_id,
        img_url,
        hash,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(image.img_id)
}

/// 只允许把相同哈希迁移到项目约定的内容寻址 R2 路径。
fn is_content_addressed_r2_url(url: &str, hash: &str) -> bool {
    url.starts_with("https://")
        && ["jpg", "png", "webp"]
            .iter()
            .any(|extension| url.ends_with(&format!("/yizu/salary-images/{hash}.{extension}")))
}

/// 图片不再被任何业务关系引用时，物理删除共享图片元数据。
pub(crate) fn delete_image_if_unreferenced(ctx: &ReducerContext, img_id: u64) -> bool {
    let referenced = ctx
        .db
        .park_image()
        .park_image_by_image()
        .filter(img_id)
        .next()
        .is_some()
        || ctx
            .db
            .factory_floor_image()
            .floor_image_by_image()
            .filter(img_id)
            .next()
            .is_some()
        || ctx
            .db
            .dormitory_image()
            .dormitory_image_by_image()
            .filter(img_id)
            .next()
            .is_some()
        || ctx
            .db
            .tenant_image()
            .tenant_image_by_image()
            .filter(img_id)
            .next()
            .is_some()
        || ctx
            .db
            .salary_image()
            .salary_image_by_image()
            .filter(img_id)
            .next()
            .is_some()
        || ctx
            .db
            .image_binding()
            .image_binding_by_image()
            .filter(img_id)
            .next()
            .is_some()
        || ctx
            .db
            .firefighting_image()
            .firefighting_image_by_image()
            .filter(img_id)
            .next()
            .is_some()
        || ctx
            .db
            .transformer_image()
            .transformer_image_by_image()
            .filter(img_id)
            .next()
            .is_some()
        || ctx
            .db
            .reimbursement_image()
            .reimbursement_image_by_image()
            .filter(img_id)
            .next()
            .is_some()
        || ctx
            .db
            .investment_image()
            .investment_image_by_image()
            .filter(img_id)
            .next()
            .is_some();
    !referenced && ctx.db.image().img_id().delete(img_id)
}

#[cfg(test)]
mod tests {
    use super::is_content_addressed_r2_url;

    const HASH: &str = "803b88bf05a35dff4ca0d0666b38953c19e3d4304c6617ca5c9c4ba2cb9b57fc";

    #[test]
    fn 只接受与哈希一致的_r2_内容寻址地址() {
        assert!(is_content_addressed_r2_url(
            &format!("https://r2.example.com/yizu/salary-images/{HASH}.jpg"),
            HASH
        ));
        assert!(!is_content_addressed_r2_url(
            "https://r2.example.com/yizu/salary-images/other.jpg",
            HASH
        ));
        assert!(!is_content_addressed_r2_url(
            &format!("http://r2.example.com/yizu/salary-images/{HASH}.jpg"),
            HASH
        ));
    }
}
