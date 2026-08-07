//! 设备管理：摄像头、门禁一类联网设备的台账与总览。

mod gateway;
mod ledger;
mod model;
mod overview;

pub use gateway::DeviceGatewayPage;
pub use ledger::{DeviceAccessPage, DeviceCameraPage};
pub use overview::DeviceOverviewPage;
