//! 服务端事务逻辑。
//!
//! # 分组
//!
//! 顶层是业务分组，与 `tables`、`views` 用同一套划分，也与侧边栏的菜单分组
//! 对应——三层形状一致，找一个域的读、写、表定义才不用各记一套路径。
//!
//! | 目录 | 侧边栏 |
//! |---|---|
//! | `rental` | 租赁：园区管理、待租厂房、租户管理、合同管理 |
//! | `hr` | 人事：人事管理、工资管理、角色管理 |
//! | `finance` | 财务：财务管理、账单管理、报销管理与审核 |
//! | `access_control` | 门禁管理 |
//! | `device` | 设备管理 |
//! | `maintenance` | 维护管理 |
//! | `investment` | 招商（服务端已完成，前端尚无页面） |
//! | `platform` | 无对应菜单：多租户中心、菜单权限、图片、通知、系统配置、迁移 |
//! | `shared` | 不属于任何域：身份权限、字段校验、园区外键、初始化 |

pub(crate) mod access_control;
pub(crate) mod device;
#[path = "finance/mod.rs"]
pub(crate) mod finance_reducers;
pub(crate) mod hr;
#[path = "investment/mod.rs"]
pub(crate) mod investment_reducers;
pub(crate) mod maintenance;
pub(crate) mod platform;
pub(crate) mod rental;
pub(crate) mod shared;

/// 跨域公用的四个模块继续从 `reducers::` 直接可达。
///
/// 它们的归属在 `shared/` 目录里写得很清楚，但被引用一百多处，让每个 `use` 都
/// 多写一层 `shared::` 只是把行拉长，并不会让人更容易找到它们。定义仍然只有
/// `shared/` 一处，这里只是门面。
pub(crate) use shared::{access, park_ref, validation};
