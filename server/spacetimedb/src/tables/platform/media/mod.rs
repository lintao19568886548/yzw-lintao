//! 共享图片及资产图片关系表。

#[path = "dormitory_image.rs"]
mod dormitory_image_table;
#[path = "floor_image.rs"]
mod floor_image_table;
#[path = "image_binding.rs"]
mod image_binding_table;
#[path = "image.rs"]
mod image_table;
#[path = "park_image.rs"]
mod park_image_table;
#[path = "salary_image.rs"]
mod salary_image_table;
#[path = "tenant_image.rs"]
mod tenant_image_table;

pub use dormitory_image_table::*;
pub use floor_image_table::*;
pub use image_binding_table::*;
pub use image_table::*;
pub use park_image_table::*;
pub use salary_image_table::*;
pub use tenant_image_table::*;
