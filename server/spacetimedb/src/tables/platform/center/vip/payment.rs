//! VIP 会员支付快照表。

use spacetimedb::Timestamp;

/// 对应中心库 `vip_membership_payment` 表。
#[spacetimedb::table(
    accessor = vip_membership_payment,
    index(accessor = vip_payment_by_scope, btree(columns = [center_scope])),
    index(accessor = vip_payment_by_center_user, btree(columns = [center_user_id])),
    index(accessor = vip_payment_by_source_org, btree(columns = [source_org_id])),
    index(accessor = vip_payment_by_source_customer, btree(columns = [source_customer_id])),
    index(accessor = vip_payment_by_trade_state, btree(columns = [trade_state])),
    index(accessor = vip_payment_by_state_refund_check, btree(columns = [trade_state, refund_checked_at]))
)]
pub struct VipMembershipPayment {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub center_scope: u8,
    #[unique]
    pub out_trade_no: String,
    pub center_user_id: u64,
    pub source_org_id: Option<u64>,
    pub source_customer_id: String,
    pub target_customer_id: Option<String>,
    pub target_city: Option<String>,
    pub target_company_short_name: Option<String>,
    pub username: Option<String>,
    pub amount_total: u64,
    pub trade_state: String,
    pub transaction_id: Option<String>,
    pub paid_at: Option<Timestamp>,
    pub applied_at: Option<Timestamp>,
    pub refund_checked_at: Option<Timestamp>,
    pub raw_attach: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}
