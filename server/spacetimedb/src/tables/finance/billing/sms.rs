//! 账单催缴短信审计日志。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = amount_bill_collection_sms_log,
    index(accessor = collection_log_by_customer, btree(columns = [customer_id])),
    index(accessor = collection_log_by_bill, btree(columns = [bill_id])),
    index(accessor = collection_log_by_type_time, btree(columns = [collection_type, sent_at]))
)]
pub struct AmountBillCollectionSmsLog {
    #[primary_key]
    #[auto_inc]
    pub log_id: u64,
    pub customer_id: String,
    pub bill_id: u64,
    pub collection_type: String,
    pub template_id: Option<String>,
    pub phone_number: Option<String>,
    pub tenant_name: Option<String>,
    pub project_name: Option<String>,
    pub remaining_amount_cents: Option<i64>,
    pub success: bool,
    pub error: Option<String>,
    /// 保留短信供应商返回的 JSON 文本。
    pub provider_result_json: Option<String>,
    pub sent_at: Timestamp,
    pub created_at: Timestamp,
}
