//! 当前用户可见的 VIP 会员、支付、权益和退款记录。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::{
    tables::*,
    views::shared::identity::{current_center_user, current_principal},
};

/// 中心侧管理台仅区分「系统管理员 / 本人」，不涉及园区维度。
fn is_system_admin(ctx: &ViewContext) -> bool {
    current_principal(ctx).is_some_and(|principal| principal.is_admin())
}

#[spacetimedb::view(accessor = my_vip_membership_payments, public)]
pub fn my_vip_membership_payments(ctx: &ViewContext) -> Vec<VipMembershipPayment> {
    let Some(user) = current_center_user(ctx) else {
        return vec![];
    };
    let admin = is_system_admin(ctx);
    let mut rows = ctx
        .db
        .vip_membership_payment()
        .vip_payment_by_scope()
        .filter(0u8)
        .filter(|row| admin || row.center_user_id == user.id)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows.reverse();
    rows
}

#[spacetimedb::view(accessor = my_vip_memberships, public)]
pub fn my_vip_memberships(ctx: &ViewContext) -> Vec<VipMembership> {
    let Some(user) = current_center_user(ctx) else {
        return vec![];
    };
    let admin = is_system_admin(ctx);
    let customer_ids = my_vip_membership_payments(ctx)
        .into_iter()
        .filter_map(|row| row.target_customer_id)
        .chain(user.customer_type)
        .collect::<BTreeSet<_>>();
    let mut rows = ctx
        .db
        .vip_membership()
        .vip_membership_by_scope()
        .filter(0u8)
        .filter(|row| admin || customer_ids.contains(&row.customer_id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows.reverse();
    rows
}

#[spacetimedb::view(accessor = my_vip_membership_entitlements, public)]
pub fn my_vip_membership_entitlements(ctx: &ViewContext) -> Vec<VipMembershipEntitlement> {
    let Some(user) = current_center_user(ctx) else {
        return vec![];
    };
    let admin = is_system_admin(ctx);
    let mut rows = ctx
        .db
        .vip_membership_entitlement()
        .vip_entitlement_by_scope()
        .filter(0u8)
        .filter(|row| admin || row.center_user_id == Some(user.id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows.reverse();
    rows
}

#[spacetimedb::view(accessor = my_vip_membership_refunds, public)]
pub fn my_vip_membership_refunds(ctx: &ViewContext) -> Vec<VipMembershipRefund> {
    let Some(user) = current_center_user(ctx) else {
        return vec![];
    };
    let admin = is_system_admin(ctx);
    let mut rows = ctx
        .db
        .vip_membership_refund()
        .vip_refund_by_scope()
        .filter(0u8)
        .filter(|row| admin || row.center_user_id == user.id)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.id);
    rows.reverse();
    rows
}
