//! 组织租户开通任务及角色快照表。

#[path = "job.rs"]
mod job_table;
#[path = "role_snapshot.rs"]
mod role_snapshot_table;

pub use job_table::*;
pub use role_snapshot_table::*;
