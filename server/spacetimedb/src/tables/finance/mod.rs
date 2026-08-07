//! 财务域的表：财务流水、租金账单、报销审批。
//!
//! 对应侧边栏「财务」分组下的财务管理、账单管理、报销管理与报销审核。三者是一
//! 条链：账单生成流水，报销批准也生成流水，流水才是对账口径。

mod approvals;
mod billing;
mod record;

pub use approvals::*;
pub use billing::*;
pub use record::*;
