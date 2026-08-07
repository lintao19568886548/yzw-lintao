//! 当前中心用户已经加入的租户列表。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_center_user_id};

#[spacetimedb::view(accessor = my_tenants, public)]
pub fn my_tenants(ctx: &ViewContext) -> Vec<Customer> {
    let Some(center_user_id) = current_center_user_id(ctx) else {
        return vec![];
    };
    let mut customers = ctx
        .db
        .user_tenant_mapping()
        .mapping_by_center_user()
        .filter(center_user_id)
        .filter_map(|mapping| ctx.db.customer().customer_id().find(mapping.customer_id))
        .filter(|customer| customer.status == 1)
        .collect::<Vec<_>>();
    customers.sort_by(|left, right| left.customer_id.cmp(&right.customer_id));
    customers.dedup_by(|left, right| left.customer_id == right.customer_id);
    customers
}
