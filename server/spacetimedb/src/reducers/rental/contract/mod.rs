//! 租赁客户事务逻辑。

mod profile;
mod tenant;
/// 账单侧要按这里的约定重算费用，所以对整个 crate 可见。
pub(crate) mod tenant_fee;
mod tenant_floor;
mod tenant_meter;
mod tenant_media;
