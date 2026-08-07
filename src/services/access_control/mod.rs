//! 门禁管理客户端服务。

mod car;
mod visitor;

pub use car::{create_access_car_record, delete_access_car_record, update_access_car_record};
pub use visitor::{
    create_access_visitor_record, delete_access_visitor_record, update_access_visitor_record,
};
