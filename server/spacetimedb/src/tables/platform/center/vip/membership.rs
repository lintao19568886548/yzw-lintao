//! 租户 VIP 会员汇总表。

use spacetimedb::Timestamp;

/// 对应中心库 `vip_membership` 表。
#[spacetimedb::table(
    accessor = vip_membership,
    index(accessor = vip_membership_by_scope, btree(columns = [center_scope])),
    index(accessor = vip_membership_by_last_payer, btree(columns = [last_payer_center_user_id]))
)]
pub struct VipMembership {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub center_scope: u8,
    #[unique]
    pub customer_id: String,
    pub status: String,
    pub expire_at: Timestamp,
    pub last_payer_center_user_id: Option<u64>,
    pub last_out_trade_no: Option<String>,
    pub last_transaction_id: Option<String>,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}
