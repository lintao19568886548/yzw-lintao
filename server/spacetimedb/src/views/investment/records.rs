//! 当前用户有权访问的招商记录、图片关系和租户收支。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_investments, public)]
pub fn my_investments(ctx: &ViewContext) -> Vec<Investment> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .investment()
        .investment_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| scope.allows_park(row.park_id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.created_at);
    rows
}

#[spacetimedb::view(accessor = my_investment_images, public)]
pub fn my_investment_images(ctx: &ViewContext) -> Vec<InvestmentImage> {
    let owner_ids = my_investments(ctx)
        .into_iter()
        .map(|row| row.investment_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for investment_id in owner_ids {
        rows.extend(
            ctx.db
                .investment_image()
                .investment_image_by_investment()
                .filter(investment_id),
        );
    }
    rows.sort_by_key(|row| row.id);
    rows
}

#[spacetimedb::view(accessor = my_investment_tenants, public)]
pub fn my_investment_tenants(ctx: &ViewContext) -> Vec<InvestmentTenant> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    // 整表仅对系统管理员开放：这张表没有园区列，可见性只由「是否不受限」决定。
    if !scope.is_unrestricted() {
        return vec![];
    }
    let mut rows = ctx
        .db
        .investment_tenant()
        .investment_tenant_by_customer()
        .filter(scope.customer_id.as_str())
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.transaction_time);
    rows
}

#[spacetimedb::view(accessor = my_enterprise_profiles, public)]
pub fn my_enterprise_profiles(ctx: &ViewContext) -> Vec<EnterpriseProfile> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .enterprise_profile()
        .enterprise_profile_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.updated_at);
    rows
}

#[spacetimedb::view(accessor = my_enterprise_tags, public)]
pub fn my_enterprise_tags(ctx: &ViewContext) -> Vec<EnterpriseTag> {
    let profile_ids = my_enterprise_profiles(ctx)
        .into_iter()
        .map(|row| row.profile_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for profile_id in profile_ids {
        rows.extend(
            ctx.db
                .enterprise_tag()
                .enterprise_tag_by_profile()
                .filter(profile_id)
                .filter(|row| !row.is_deleted),
        );
    }
    rows.sort_by_key(|row| row.updated_at);
    rows
}
