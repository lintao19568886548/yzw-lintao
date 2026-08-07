//! 租赁域的表：园区、资产台账、租赁合同。
//!
//! 对应侧边栏「租赁」分组下的园区管理、待租厂房、租户管理、合同管理。这三块在
//! 产品上是一件事——园区里有厂房和楼层，楼层上装着水电表，合同租的就是这些楼层
//! 并按这些表结算——所以放在同一个组里。

mod assets;
mod contract;
#[path = "park.rs"]
mod park_table;

pub use assets::*;
pub use contract::*;
pub use park_table::*;
