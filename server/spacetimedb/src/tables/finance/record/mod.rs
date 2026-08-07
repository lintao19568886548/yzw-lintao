//! 财务流水及其附件表。

#[path = "image.rs"]
mod finance_image_table;
#[path = "record.rs"]
mod finance_record_table;

pub use finance_image_table::*;
pub use finance_record_table::*;
