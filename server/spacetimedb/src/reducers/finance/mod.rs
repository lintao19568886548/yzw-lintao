//! 财务域的写入逻辑：财务流水、租金账单、报销审批。
//!
//! 对应侧边栏「财务」分组下的财务管理、账单管理、报销管理与报销审核。

pub(crate) mod approvals;
pub(crate) mod billing;
pub(crate) mod record;
