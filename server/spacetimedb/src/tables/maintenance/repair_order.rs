//! 园区报修工单及其业务流转时间。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = repair_order,
    index(accessor = repair_order_by_customer, btree(columns = [customer_id])),
    index(accessor = repair_order_by_park, btree(columns = [park_id])),
    index(accessor = repair_order_by_status, btree(columns = [status]))
)]
pub struct RepairOrder {
    #[primary_key]
    #[auto_inc]
    pub repair_order_id: u64,
    #[unique]
    pub order_no: String,
    pub customer_id: String,
    pub source: String,
    pub tenant_name: Option<String>,
    pub tenant_phone: Option<String>,
    pub repair_type: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub assignee_phone: Option<String>,
    pub process_remark: Option<String>,
    pub accept_time: Option<Timestamp>,
    pub finish_time: Option<Timestamp>,
    pub confirm_time: Option<Timestamp>,
    pub factory_id: Option<u64>,
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
