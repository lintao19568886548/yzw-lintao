//! 合同管理客户端服务。

mod ai;
mod reducer;
mod sms;

pub use ai::{analyze_contract_images, ContractAiImage};
pub use reducer::{
    create_contract_with_images_record, delete_contract_record, update_contract_with_images_record,
};
pub use sms::send_contract_reminder_sms;
