//! 平台层的写入逻辑：多租户中心、菜单权限、图片、通知、系统配置、数据迁移。
//!
//! 侧边栏上没有对应的业务菜单——它们不是园区业务，而是撑起业务的底座。
//! 和业务域分开放，是为了让「改这里会影响所有租户」一眼看得出来。

pub(crate) mod center;
pub(crate) mod media;
#[path = "menu.rs"]
pub(crate) mod menu_reducer;
pub(crate) mod migration;
pub(crate) mod navigation;
pub(crate) mod notices;
pub(crate) mod permissions;
pub(crate) mod profile;
pub(crate) mod support;
pub(crate) mod system;
