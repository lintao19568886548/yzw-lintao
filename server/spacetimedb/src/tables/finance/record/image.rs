//! 财务流水附件表。

/// MySQL `finance_image` 直接保存 URL，不引用共享图片主表。
#[spacetimedb::table(
    accessor = finance_image,
    index(accessor = finance_image_by_customer, btree(columns = [customer_id])),
    index(accessor = finance_image_by_finance, btree(columns = [finance_id]))
)]
pub struct FinanceImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub finance_id: u64,
    pub url: String,
}
