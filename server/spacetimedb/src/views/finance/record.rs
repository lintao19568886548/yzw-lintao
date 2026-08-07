//! 当前用户有权访问的财务流水和附件。

use spacetimedb::ViewContext;

use crate::views::shared::identity::current_read_scope;
use crate::tables::*;

#[spacetimedb::view(accessor = my_finances, public)]
pub fn my_finances(ctx: &ViewContext) -> Vec<Finance> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut finances = ctx
        .db
        .finance()
        .finance_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|finance| !finance.is_deleted && scope.allows_park(finance.park_id))
        .collect::<Vec<_>>();
    finances.sort_by_key(|finance| finance.finance_id);
    finances
}

#[spacetimedb::view(accessor = my_finance_images, public)]
pub fn my_finance_images(ctx: &ViewContext) -> Vec<FinanceImage> {
    let mut images = Vec::new();
    for finance in my_finances(ctx) {
        images.extend(
            ctx.db
                .finance_image()
                .finance_image_by_finance()
                .filter(finance.finance_id),
        );
    }
    images.sort_by_key(|image| image.id);
    images
}
