//! 智能水电表管理的 Dioxus Server Function 入口。

use dioxus::prelude::*;

use super::types::{MeterKind, SmartMeterCatalog, SmartMeterReadingBatch, SmartMeterSnapshot};

#[post("/api/smart-meter/catalog")]
pub async fn load_smart_meter_catalog(
    spacetime_token: String,
    kind: MeterKind,
) -> Result<SmartMeterCatalog, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let result = match kind {
            MeterKind::Electric => super::hezhong::load_electric_catalog(&spacetime_token).await,
            MeterKind::Water => super::provider::load_catalog(&spacetime_token, kind).await,
        };
        result.map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (spacetime_token, kind);
        Err(ServerFnError::new("智能水电表设备接口只能在服务端执行"))
    }
}

#[post("/api/smart-meter/readings")]
pub async fn load_smart_meter_readings(
    spacetime_token: String,
    kind: MeterKind,
    date: String,
) -> Result<SmartMeterReadingBatch, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let result = match kind {
            MeterKind::Electric => {
                super::hezhong::load_electric_readings(&spacetime_token, &date).await
            }
            MeterKind::Water => super::provider::load_readings(&spacetime_token, kind, &date).await,
        };
        result.map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (spacetime_token, kind, date);
        Err(ServerFnError::new("智能水电表读数接口只能在服务端执行"))
    }
}

#[post("/api/smart-meter/snapshot")]
pub async fn load_smart_meter_snapshot(
    spacetime_token: String,
    kind: MeterKind,
    date: String,
) -> Result<SmartMeterSnapshot, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let result = match kind {
            // 正式系统的电表树和冻结读数来自合众项目 241。
            MeterKind::Electric => {
                super::hezhong::load_electric_snapshot(&spacetime_token, &date).await
            }
            // 水表暂时保留现有 YMSINO 接入，避免改变尚未核实的生产协议。
            MeterKind::Water => super::provider::load_snapshot(&spacetime_token, kind, &date).await,
        };
        result.map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (spacetime_token, kind, date);
        Err(ServerFnError::new("智能水电表接口只能在服务端执行"))
    }
}
