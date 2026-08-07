//! 门禁管理三个业务页面。

mod car;
mod car_form;
mod delete_dialog;
mod model;
mod navigation;
mod register;
mod visitor;
mod visitor_form;

pub use car::AccessCarPage;
pub use register::AccessVisitorRegisterPage;
pub use visitor::AccessVisitorPage;
