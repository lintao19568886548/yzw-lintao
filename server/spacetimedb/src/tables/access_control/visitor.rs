//! 访客出入登记表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `access_visitor` 表。
#[spacetimedb::table(
    accessor = access_visitor,
    index(accessor = access_visitor_by_customer, btree(columns = [customer_id])),
    index(accessor = access_visitor_by_park, btree(columns = [park_id])),
    index(accessor = access_visitor_by_status_time, btree(columns = [status, register_time]))
)]
pub struct AccessVisitor {
    #[primary_key]
    #[auto_inc]
    pub visitor_id: u64,
    pub customer_id: String,
    pub visitor_name: String,
    pub car_num: Option<String>,
    pub phone_number: String,
    /// `0` 表示进入，`1` 表示离开。
    pub status: i8,
    pub register_time: Timestamp,
    pub remark: Option<String>,
    /// 园区外键。`0` 表示不绑定园区，见 `reducers::shared::park_ref`。
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
