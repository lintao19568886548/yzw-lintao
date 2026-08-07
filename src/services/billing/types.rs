//! 账单外部服务共用的序列化数据结构。

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratedBillingFile {
    pub file_name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BillExcelRow {
    pub park_name: String,
    pub project_name: String,
    pub tenant_name: String,
    pub ele_fee_cents: i64,
    pub water_fee_cents: i64,
    pub factory_rent_cents: i64,
    pub management_fee_cents: i64,
    pub service_fee_cents: i64,
    pub garbage_fee_cents: i64,
    pub invoice_tax_cents: i64,
    pub penalty_fee_cents: i64,
    pub receive_fee_cents: i64,
    pub total_fee_cents: i64,
    pub receipt_amount_cents: i64,
    pub remaining_amount_cents: i64,
    pub collection_status: String,
    pub receipt_date: String,
    pub remark: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AiAmountBillDraft {
    pub park_name: String,
    pub tenant_name: String,
    pub project_name: String,
    pub public_bank_account: String,
    pub private_bank_account: String,
    pub ele_fee_cents: i64,
    pub water_fee_cents: i64,
    pub factory_rent_cents: i64,
    pub management_fee_cents: i64,
    pub service_fee_cents: i64,
    pub garbage_fee_cents: i64,
    pub invoice_tax_cents: i64,
    pub penalty_fee_cents: i64,
    pub receive_fee_cents: i64,
    pub total_fee_cents: i64,
    pub receipt_amount_cents: i64,
    pub receipt_date: String,
    pub remark: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CollectionSmsResult {
    pub bill_id: u64,
    pub success: bool,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CollectionSmsBatchResult {
    pub results: Vec<CollectionSmsResult>,
}
