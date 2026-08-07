//! 报销申请与 R2 凭证元数据的一体化事务。

use spacetimedb::{ReducerContext, SpacetimeType};

use super::reimbursement::{ReimbursementInput, create_reimbursement_record};
use crate::reducers::{access::current_customer_id, platform::media::image_reducer::ensure_image};

#[derive(SpacetimeType)]
pub struct UploadedReimbursementImageInput {
    pub img_url: String,
    pub hash: String,
}

/// 用户点击确认后，客户端先上传 R2；本 Reducer 再原子登记图片并创建报销关系。
#[spacetimedb::reducer]
pub fn create_reimbursement_with_images(
    ctx: &ReducerContext,
    mut input: ReimbursementInput,
    uploads: Vec<UploadedReimbursementImageInput>,
) -> Result<(), String> {
    if input.image_ids.len() + uploads.len() > 8 {
        return Err("每份报销申请最多上传8张凭证".into());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    for upload in uploads {
        input.image_ids.push(ensure_image(
            ctx,
            customer_id.clone(),
            upload.img_url,
            upload.hash,
        )?);
    }
    create_reimbursement_record(ctx, input).map(|_| ())
}
