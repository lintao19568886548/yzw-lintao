//! VIP 会员每笔订单权益表。

use spacetimedb::Timestamp;

/// 对应中心库 `vip_membership_entitlement` 表。
#[spacetimedb::table(
    accessor = vip_membership_entitlement,
    index(accessor = vip_entitlement_by_scope, btree(columns = [center_scope])),
    index(accessor = vip_entitlement_by_customer, btree(columns = [customer_id])),
    index(accessor = vip_entitlement_by_center_user, btree(columns = [center_user_id])),
    index(accessor = vip_entitlement_by_status, btree(columns = [status])),
    index(accessor = vip_entitlement_by_customer_status_start, btree(columns = [customer_id, status, start_at])),
    index(accessor = vip_entitlement_by_customer_status_end, btree(columns = [customer_id, status, end_at]))
)]
pub struct VipMembershipEntitlement {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub center_scope: u8,
    pub customer_id: String,
    pub center_user_id: Option<u64>,
    #[unique]
    pub out_trade_no: String,
    pub transaction_id: Option<String>,
    pub amount_total: u64,
    pub duration_months: u32,
    pub start_at: Timestamp,
    pub end_at: Timestamp,
    pub status: String,
    pub refunded_at: Option<Timestamp>,
    pub refunded_out_refund_no: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}
