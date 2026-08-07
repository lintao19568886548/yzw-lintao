//! 易租应用的 SpacetimeDB 服务端模块。
//!
//! MySQL 在迁移阶段仍是表结构和隐式关系的事实来源；服务端表、事务逻辑和
//! 客户端视图按经过验证的业务批次逐步迁移。

pub mod access;
mod lifecycle;
mod procedures;
pub mod reducers;
mod sms;
pub mod tables;
mod views;
