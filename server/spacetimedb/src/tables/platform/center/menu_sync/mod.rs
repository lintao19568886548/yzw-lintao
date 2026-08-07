//! 菜单模板同步任务及目标日志表。

#[path = "job.rs"]
mod job_table;
#[path = "log.rs"]
mod log_table;

pub use job_table::*;
pub use log_table::*;
