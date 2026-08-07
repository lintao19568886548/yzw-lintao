//! 系统配置、版本和审计业务表。

#[path = "api_log.rs"]
mod api_log_table;
#[path = "app_version.rs"]
mod app_version_table;
#[path = "system_key.rs"]
mod system_key_table;

pub use api_log_table::*;
pub use app_version_table::*;
pub use system_key_table::*;
