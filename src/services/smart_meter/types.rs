//! 智能水电表管理页面与 Dioxus Server 之间共享的数据类型。

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeterKind {
    Electric,
    Water,
}

impl MeterKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Electric => "电表",
            Self::Water => "水表",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::Electric => "度",
            Self::Water => "吨",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SmartMeterDevice {
    pub park_id: String,
    pub park_name: String,
    pub building_name: String,
    pub floor_name: String,
    pub room_id: String,
    pub room_name: String,
    pub device_id: String,
    pub factory_no: String,
    pub protocol: String,
    pub current_ratio: String,
    pub multiplier: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SmartMeterReading {
    pub room_id: String,
    pub room_name: String,
    pub device_id: String,
    pub com_address: String,
    pub data_item_name: String,
    pub data_value: String,
    pub data_value_tip: String,
    pub data_value_peak: String,
    pub data_value_flat: String,
    pub data_value_valley: String,
    pub freeze_time: String,
    pub write_time: String,
}

/// 左侧建筑树所需的设备目录；与读数分开加载，避免慢查询阻塞整页。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SmartMeterCatalog {
    pub kind: MeterKind,
    pub provider_name: String,
    pub park_name: String,
    pub protocol: String,
    pub devices: Vec<SmartMeterDevice>,
}

/// 指定日期的冻结读数批次。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SmartMeterReadingBatch {
    pub kind: MeterKind,
    pub requested_date: String,
    pub readings: Vec<SmartMeterReading>,
}

/// 兼容旧版单请求端点；新页面已改用目录与读数分阶段接口。
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SmartMeterSnapshot {
    pub kind: MeterKind,
    pub requested_date: String,
    pub provider_name: String,
    pub park_name: String,
    pub protocol: String,
    pub devices: Vec<SmartMeterDevice>,
    pub readings: Vec<SmartMeterReading>,
}
