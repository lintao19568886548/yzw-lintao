//! 当前用户有权访问的报销申请及附件关系。

use std::collections::BTreeSet;

use spacetimedb::{SpacetimeType, ViewContext};

use crate::{access::Duty, tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_reimbursements, public)]
pub fn my_reimbursements(ctx: &ViewContext) -> Vec<Reimbursement> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .reimbursement()
        .reimbursement_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted)
        // 本人的报销始终可见；有审批职能时还能看到数据范围内其他人的申请。
        .filter(|row| scope.owns_or_has_duty(row.user_id, row.park_id, Duty::ReimbursementAudit))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows
}

#[spacetimedb::view(accessor = my_reimbursement_images, public)]
pub fn my_reimbursement_images(ctx: &ViewContext) -> Vec<ReimbursementImage> {
    let ids = my_reimbursements(ctx)
        .into_iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for id in ids {
        rows.extend(
            ctx.db
                .reimbursement_image()
                .reimbursement_image_by_reimbursement()
                .filter(id),
        );
    }
    rows.sort_by_key(|row| row.id);
    rows
}

/// 报销页面直接使用的最小图片信息，避免订阅租户下全部图片。
#[derive(SpacetimeType)]
pub struct ReimbursementImagePreview {
    pub reimbursement_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_reimbursement_image_previews, public)]
pub fn my_reimbursement_image_previews(ctx: &ViewContext) -> Vec<ReimbursementImagePreview> {
    let mut previews = my_reimbursement_images(ctx)
        .into_iter()
        .filter_map(|link| {
            let image = ctx.db.image().img_id().find(link.img_id)?;
            Some(ReimbursementImagePreview {
                reimbursement_id: link.reimbursement_id,
                img_id: link.img_id,
                img_url: image.img_url,
            })
        })
        .collect::<Vec<_>>();
    previews.sort_by_key(|row| (row.reimbursement_id, row.img_id));
    previews
}
