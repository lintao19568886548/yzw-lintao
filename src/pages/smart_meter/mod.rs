//! 智能水电表管理响应式页面模块。

mod binding;
pub(crate) mod date;

pub(crate) use date::{previous_month_same_day, today};
mod device_tree;
mod overview;
mod reading_table;

pub use overview::{SmartElectricMeterPage, SmartWaterMeterPage};
