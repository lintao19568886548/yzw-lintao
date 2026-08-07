//! 卫生检查记录表定义。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = hygiene_check,
    index(accessor = hygiene_check_by_customer, btree(columns = [customer_id])),
    index(accessor = hygiene_check_by_factory, btree(columns = [factory_id])),
    index(accessor = hygiene_check_by_park, btree(columns = [park_id]))
)]
pub struct HygieneCheck {
    #[primary_key]
    #[auto_inc]
    pub hygiene_check_id: u64,
    pub customer_id: String,
    pub check_items: String,
    pub checker: String,
    pub check_date: Timestamp,
    pub check_result: String,
    pub remark: Option<String>,
    pub factory_id: u64,
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
