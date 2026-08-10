//! 客户端基础设施服务。

mod access_control;
mod contract;
mod credentials;
mod device;
mod finance;
mod hr;
mod maintenance;
mod miniapp;
mod park_ref;
mod profile;
mod permission_management;
mod reconnect;
mod reimbursement;
mod rental_management;
mod salary;
mod smart_meter;
mod spacetime;
mod storage;
mod tenant;

pub use contract::{
    analyze_contract_images, create_contract_with_images_record, delete_contract_record,
    send_contract_reminder_sms, update_contract_with_images_record, ContractAiImage,
};
pub use credentials::{load_saved_account, load_saved_token};
pub use device::{
    delete_device_asset_record, delete_edge_gateway_record,
    regenerate_edge_registration_code_record, save_device_asset_record, save_edge_gateway_record,
};
pub use finance::{
    create_finance_with_images_record, delete_finance_record, update_finance_with_images_record,
};
pub use hr::{
    audit_leave_record, create_attendance_location_record, create_employee_record,
    create_leave_record, delete_attendance_location_record, delete_employee_record,
    delete_leave_record, punch_in_record, punch_out_record, update_attendance_location_record,
    update_employee_record,
    update_leave_record,
};
pub use park_ref::{has_park, park_ref, NO_PARK};
pub use permission_management::{delete_role_policy, save_role_policy};
pub use reconnect::use_auto_reconnect;
pub use reimbursement::{
    audit_reimbursement_record, create_reimbursement_with_images_record,
    delete_reimbursement_record,
};
pub use rental_management::{
    create_dormitory_floor_record, create_dormitory_record, create_factory_floor_record,
    create_factory_with_floors_record, create_park_with_images_record, create_utility_meter_record,
    delete_dormitory_floor_record, delete_dormitory_record, delete_factory_floor_record,
    delete_factory_record, delete_park_record, delete_utility_meter_record,
    update_dormitory_floor_record, update_dormitory_with_images_record,
    update_factory_floor_with_images_record, update_factory_record, update_park_with_images_record,
    update_utility_meter_record,
};
pub use salary::{
    create_salary_with_images_record, delete_salary_record, update_salary_with_images_record,
};
pub use smart_meter::{
    load_smart_meter_catalog, load_smart_meter_readings, MeterKind, SmartMeterCatalog,
    SmartMeterDevice, SmartMeterReading,
};
pub use profile::{change_my_password_record, update_my_avatar_record, update_my_name_record};
pub use maintenance::{
    create_elevator_inspection_record, create_firefighting_inspection_record,
    create_transformer_inspection_record, delete_elevator_asset_record,
    delete_firefighting_asset_record, delete_transformer_asset_record,
    save_elevator_asset_record, save_firefighting_asset_record, save_transformer_asset_record,
};
pub use spacetime::{
    activate_data_scope, connect_workspace, delete_repair_order_record, login_workspace,
    login_workspace_with_sms, logout_workspace, save_repair_order, send_login_sms_code,
    transition_repair_order_record, ConnectionConfig,
};
pub use storage::{
    delete_business_images_from_r2, delete_salary_images_from_r2, upload_business_image,
    upload_salary_image, validate_business_image, validate_salary_image, StoredR2Image,
};
pub use tenant::{
    create_tenant_profile_record, delete_tenant_profile_record, sync_tenant_profiles_record,
    update_tenant_profile_record,
};
mod billing;
pub use access_control::{
    create_access_car_record, create_access_visitor_record, delete_access_car_record,
    delete_access_visitor_record, update_access_car_record, update_access_visitor_record,
};
pub use billing::{
    analyze_amount_bill_excel, confirm_bill_collected_record, confirm_bill_shortfall_record,
    confirm_carryover_batch_record, create_amount_bill_record, delete_amount_bill_record,
    discard_carryover_batch_record, download_generated_file, export_amount_bill_excel,
    open_carryover_batch_record, send_collection_sms, set_carryover_disposition_record,
    update_amount_bill_record, validate_amount_bill_excel, AiAmountBillDraft, BillExcelRow,
};
mod history;

pub use history::{
    query_billing_history, query_finance_history, query_salary_history, BillingPageQuery,
    FinancePageQuery, SalaryPageQuery,
};
