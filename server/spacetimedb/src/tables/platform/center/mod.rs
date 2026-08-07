//! `magic_center` 中心库表定义。

mod auth;
#[path = "customer.rs"]
mod customer_table;
mod invitations;
mod menu_sync;
#[path = "organization.rs"]
mod organization_table;
mod provisioning;
mod system;
mod user;
mod vip;

pub use auth::*;
pub use customer_table::*;
pub use invitations::*;
pub use menu_sync::*;
pub use organization_table::*;
pub use provisioning::*;
pub use system::*;
pub use user::*;
pub use vip::*;
