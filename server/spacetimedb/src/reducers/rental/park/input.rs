//! 园区租赁主档输入类型。

use spacetimedb::SpacetimeType;

/// 对齐原系统园区表单的可编辑字段，面积按百分之一平方米保存。
#[derive(SpacetimeType)]
pub struct ParkInput {
    pub park_name: String,
    pub address: String,
    pub manager: Option<String>,
    pub contact: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
}
