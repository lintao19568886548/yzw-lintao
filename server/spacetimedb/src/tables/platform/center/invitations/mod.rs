//! 组织与租户邀请码表。

#[path = "organization.rs"]
mod organization_invitation_table;
#[path = "tenant.rs"]
mod tenant_invitation_table;
#[path = "tenant_join_log.rs"]
mod tenant_join_log_table;

pub use organization_invitation_table::*;
pub use tenant_invitation_table::*;
pub use tenant_join_log_table::*;
