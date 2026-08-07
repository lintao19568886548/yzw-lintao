//! 智能水电表管理服务模块。

#[cfg(feature = "server")]
mod common;
#[cfg(feature = "server")]
mod hezhong;
#[cfg(feature = "server")]
mod provider;
mod server;
mod types;

pub use server::{load_smart_meter_catalog, load_smart_meter_readings};
pub use types::{MeterKind, SmartMeterCatalog, SmartMeterDevice, SmartMeterReading};
