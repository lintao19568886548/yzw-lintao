//! 面向 Dioxus 客户端的只读订阅视图。
//!
//! 原始表保持私有，客户端只能订阅当前身份有权访问的数据。
//!
//! 目录分组与 `tables` 一致，对应侧边栏的菜单分组——三层形状一样，找一个域的
//! 读、写、表定义才不用各记一套路径。

pub(crate) mod access_control;
pub(crate) mod device;
#[path = "finance/mod.rs"]
pub(crate) mod finance_views;
pub(crate) mod hr;
pub(crate) mod investment;
pub(crate) mod maintenance;
pub(crate) mod platform;
pub(crate) mod rental;
pub(crate) mod shared;
pub(crate) mod workbench;
