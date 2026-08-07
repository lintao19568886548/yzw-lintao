//! VIP 会员退款记录表。

use spacetimedb::Timestamp;

/// 对应中心库 `vip_membership_refund` 表。
#[spacetimedb::table(
    accessor = vip_membership_refund,
    index(accessor = vip_refund_by_scope, btree(columns = [center_scope])),
    index(accessor = vip_refund_by_trade_no, btree(columns = [out_trade_no])),
    index(accessor = vip_refund_by_customer, btree(columns = [customer_id])),
    index(accessor = vip_refund_by_status_next_check, btree(columns = [status, next_check_at]))
)]
pub struct VipMembershipRefund {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub center_scope: u8,
    #[unique]
    pub out_refund_no: String,
    pub refund_id: Option<String>,
    pub out_trade_no: String,
    pub transaction_id: Option<String>,
    pub center_user_id: u64,
    pub customer_id: String,
    pub amount_total: u64,
    pub refund_amount: u64,
    pub status: String,
    pub reason: Option<String>,
    pub channel: String,
    pub notify_event_id: Option<String>,
    pub provider_raw: Option<String>,
    pub requested_at: Option<Timestamp>,
    pub success_at: Option<Timestamp>,
    pub last_checked_at: Option<Timestamp>,
    pub next_check_at: Option<Timestamp>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}
