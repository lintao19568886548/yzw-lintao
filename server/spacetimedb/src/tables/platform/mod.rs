//! 平台层的表：多租户中心、菜单权限、图片、通知、系统配置。
//!
//! 这一组在侧边栏上**没有对应的业务菜单**，因为它们不是园区业务，而是撑起业务
//! 的底座：`center` 是多租户 SaaS 那一套（中心账号、租户开通、VIP、邀请），
//! 其余是菜单导航、权限码、图片元数据、通知和系统配置。
//!
//! 和业务域分开放，是为了让「改这里会影响所有租户」这件事一眼看得出来。

mod center;
mod media;
#[path = "menu.rs"]
mod menu_table;
mod navigation;
mod notices;
mod permissions;
mod support;
mod system;

pub use center::*;
pub use media::*;
pub use menu_table::*;
pub use navigation::*;
pub use notices::*;
pub use permissions::*;
pub use support::*;
pub use system::*;
