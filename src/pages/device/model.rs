//! 设备管理页面的函数式核心：分类、类型文案与总览聚合。

use crate::spacetime_bindings::{
    device_asset_type::DeviceAsset, factory_type::Factory, park_type::Park,
    utility_meter_type::UtilityMeter,
};

/// 健康等级，与服务端 `HEALTH_*` 常量一一对应。
pub(super) const HEALTH_NORMAL: &str = "normal";
pub(super) const HEALTH_WARNING: &str = "warning";
pub(super) const HEALTH_CRITICAL: &str = "critical";

/// 健康等级中文名。
#[pure_function::pure]
pub(super) fn health_label(level: &str) -> &'static str {
    match level {
        HEALTH_NORMAL => "正常",
        HEALTH_WARNING => "需要留意",
        HEALTH_CRITICAL => "已影响工作",
        _ => "尚未上报",
    }
}

/// 健康等级到徽章样式。
///
/// 「尚未上报」用 Outline 而不是 Destructive：边缘设备刚建档还没装机时就是这个
/// 状态，把它标红只会让人以为出了故障。
#[pure_function::pure]
pub(super) fn health_variant(level: &str) -> crate::components::badge::BadgeVariant {
    use crate::components::badge::BadgeVariant;
    match level {
        HEALTH_NORMAL => BadgeVariant::Secondary,
        HEALTH_CRITICAL => BadgeVariant::Destructive,
        _ => BadgeVariant::Outline,
    }
}

/// 这条健康等级是否需要有人处理。
#[pure_function::pure]
pub(super) fn health_needs_attention(level: &str) -> bool {
    matches!(level, HEALTH_WARNING | HEALTH_CRITICAL)
}

/// 「在线 3 天」「离线 2 小时」这样的相对时长。
///
/// 传入当前时刻而不是自己读时钟——这一层要能脱离运行环境单测。
#[pure_function::pure]
pub(super) fn elapsed_label(since_micros: i64, now_micros: i64) -> String {
    let seconds = (now_micros - since_micros).max(0) / 1_000_000;
    if seconds < 60 {
        return "刚刚".into();
    }
    if seconds < 3_600 {
        return format!("{} 分钟", seconds / 60);
    }
    if seconds < 86_400 {
        return format!("{} 小时", seconds / 3_600);
    }
    format!("{} 天", seconds / 86_400)
}

/// 传感器：只读物理世界的状态。与服务端 `DEVICE_CLASS_SENSOR` 保持一致。
pub(super) const CLASS_SENSOR: &str = "sensor";
/// 执行终端：接受指令改变物理世界的状态。与服务端 `DEVICE_CLASS_ACTUATOR` 一致。
pub(super) const CLASS_ACTUATOR: &str = "actuator";

/// 摄像头页可登记的类型；对应服务端归为传感器的那一组。
pub(super) const SENSOR_TYPES: [&str; 1] = ["camera"];
/// 门禁设备页可登记的类型；对应服务端归为执行终端的那一组。
pub(super) const ACTUATOR_TYPES: [&str; 3] = ["access_controller", "barrier", "turnstile"];

/// 设备分类中文名。
#[pure_function::pure]
pub(super) fn class_label(device_class: &str) -> &'static str {
    match device_class {
        CLASS_SENSOR => "传感器",
        CLASS_ACTUATOR => "执行终端",
        _ => "未分类",
    }
}

/// 设备类型中文名。库里存的是英文键，界面上一律显示中文。
#[pure_function::pure]
pub(super) fn device_type_label(device_type: &str) -> &'static str {
    match device_type {
        "camera" => "摄像头",
        "access_controller" => "门禁一体机",
        "barrier" => "车辆道闸",
        "turnstile" => "人行闸机",
        _ => "未知类型",
    }
}

/// 厂房展示名；`0` 表示园区公共区域。
#[pure_function::pure]
pub(super) fn factory_label(factories: &[Factory], factory_id: u64) -> String {
    if factory_id == 0 {
        return "园区公共区域".into();
    }
    factories
        .iter()
        .find(|row| row.factory_id == factory_id)
        .map(|row| row.factory_name.clone())
        .unwrap_or_else(|| format!("厂房 #{factory_id}"))
}

#[pure_function::pure]
pub(super) fn park_label(parks: &[Park], park_id: u64) -> String {
    parks
        .iter()
        .find(|row| row.park_id == park_id)
        .map(|row| row.park_name.clone())
        .unwrap_or_else(|| format!("园区 #{park_id}"))
}

/// 总览里的一行：某个设备类型的台数与接入情况。
#[derive(Clone, PartialEq, Debug)]
pub(super) struct DeviceTypeRow {
    pub label: &'static str,
    pub total: usize,
    /// 已填平台设备号的台数。没填就只是一条静态台账，拿不到任何实时状态。
    pub bound: usize,
    /// 这一类设备归哪一页管；`None` 表示台账不在本模块（水电表在园区档案里维护）。
    pub route: Option<&'static str>,
}

impl DeviceTypeRow {
    #[pure_function::pure]
    pub fn unbound(&self) -> usize {
        self.total.saturating_sub(self.bound)
    }
}

/// 两栏总览的数据。
#[derive(Clone, PartialEq, Debug, Default)]
pub(super) struct DeviceSummary {
    pub sensors: Vec<DeviceTypeRow>,
    pub actuators: Vec<DeviceTypeRow>,
}

impl DeviceSummary {
    #[pure_function::pure]
    pub fn sensor_total(&self) -> usize {
        self.sensors.iter().map(|row| row.total).sum()
    }

    #[pure_function::pure]
    pub fn actuator_total(&self) -> usize {
        self.actuators.iter().map(|row| row.total).sum()
    }

    #[pure_function::pure]
    pub fn total(&self) -> usize {
        self.sensor_total() + self.actuator_total()
    }

    #[pure_function::pure]
    pub fn bound_total(&self) -> usize {
        self.sensors
            .iter()
            .chain(self.actuators.iter())
            .map(|row| row.bound)
            .sum()
    }

    /// 接入率百分比（整数，向下取整）。一台设备都没有时返回 0。
    ///
    /// 刻意不叫「在线率」：填了平台设备号只说明**能**取到状态，不代表此刻在线。
    /// 真正的在线率要向厂商平台实时查，目前只有水电表那条链路通着。
    #[pure_function::pure]
    pub fn bound_percent(&self) -> usize {
        let total = self.total();
        if total == 0 {
            return 0;
        }
        self.bound_total() * 100 / total
    }
}

/// 是否填了平台设备号。空白串等于没填——表单会把空输入交上来。
#[pure_function::pure]
pub(super) fn is_platform_bound(external_device_id: Option<&str>) -> bool {
    external_device_id.is_some_and(|value| !value.trim().is_empty())
}

/// 按传感器／执行终端两栏汇总设备台账与水电表台账。
///
/// 水电表来自 `utility_meter` 而不是 `device_asset`——两张表在这里合流，
/// 但各自的写入通道不变（`docs/设备管理.md` 第四章）。
#[pure_function::pure]
pub(super) fn summarize_devices(devices: &[DeviceAsset], meters: &[UtilityMeter]) -> DeviceSummary {
    let type_row = |device_type: &'static str, route: Option<&'static str>| {
        let mut total = 0usize;
        let mut bound = 0usize;
        for row in devices.iter().filter(|row| row.device_type == device_type) {
            total += 1;
            bound += usize::from(is_platform_bound(row.external_device_id.as_deref()));
        }
        DeviceTypeRow {
            label: device_type_label(device_type),
            total,
            bound,
            route,
        }
    };
    let meter_row = |label: &'static str, is_electric: bool| {
        let mut total = 0usize;
        let mut bound = 0usize;
        for row in meters.iter().filter(|row| row.is_electric == is_electric) {
            total += 1;
            bound += usize::from(is_platform_bound(row.external_device_id.as_deref()));
        }
        DeviceTypeRow {
            label,
            total,
            bound,
            // 水电表台账在园区档案里维护，绑定在智能水电表管理页做，
            // 所以这里指向后者——总览是入口，不是第二个编辑面。
            route: Some("/smart-meter/meter"),
        }
    };
    DeviceSummary {
        sensors: vec![
            meter_row("电表", true),
            meter_row("水表", false),
            type_row("camera", Some("/device/camera")),
        ],
        actuators: ACTUATOR_TYPES
            .iter()
            .map(|device_type| type_row(device_type, Some("/device/access")))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(device_type: &str, device_class: &str, external: Option<&str>) -> DeviceAsset {
        DeviceAsset {
            asset_id: 1,
            customer_id: "c".into(),
            park_id: 1,
            factory_id: 0,
            device_class: device_class.into(),
            device_type: device_type.into(),
            device_name: "设备".into(),
            device_code: "D-1".into(),
            location: "北大门".into(),
            vendor: None,
            model: None,
            external_device_id: external.map(str::to_string),
            commissioned_on: None,
            remark: None,
            is_deleted: false,
            created_at: spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(0),
            updated_at: None,
            gateway_id: 0,
            channel_no: 0,
        }
    }

    fn meter(is_electric: bool, external: Option<&str>) -> UtilityMeter {
        UtilityMeter {
            meter_id: 1,
            customer_id: "c".into(),
            park_id: 1,
            meter_code: "M-1".into(),
            is_electric,
            factory_floor_id: 0,
            dormitory_floor_id: 0,
            multiplier_centi: 100,
            is_time_of_use: false,
            external_device_id: external.map(str::to_string),
            remark: None,
            is_deleted: false,
            created_at: spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(0),
            updated_at: None,
        }
    }

    #[test]
    fn 摄像头归传感器栏门禁归执行终端栏() {
        let summary = summarize_devices(
            &[
                device("camera", CLASS_SENSOR, None),
                device("barrier", CLASS_ACTUATOR, None),
            ],
            &[],
        );
        assert_eq!(summary.sensor_total(), 1);
        assert_eq!(summary.actuator_total(), 1);
    }

    #[test]
    fn 水电表并入传感器栏但不进设备表() {
        let summary = summarize_devices(&[], &[meter(true, None), meter(false, None)]);
        assert_eq!(summary.sensor_total(), 2, "电表水表都该算进传感器");
        assert_eq!(summary.actuator_total(), 0);
    }

    #[test]
    fn 接入率只数填了平台设备号的() {
        let summary = summarize_devices(
            &[
                device("camera", CLASS_SENSOR, Some("A1")),
                device("camera", CLASS_SENSOR, None),
                // 空白串等于没填，不能算接入。
                device("barrier", CLASS_ACTUATOR, Some("   ")),
                device("barrier", CLASS_ACTUATOR, Some("B1")),
            ],
            &[],
        );
        assert_eq!(summary.total(), 4);
        assert_eq!(summary.bound_total(), 2);
        assert_eq!(summary.bound_percent(), 50);
    }

    #[test]
    fn 一台设备都没有时接入率是零而不是崩溃() {
        let summary = summarize_devices(&[], &[]);
        assert_eq!(summary.total(), 0);
        assert_eq!(summary.bound_percent(), 0);
    }

    #[test]
    fn 未接入台数由总数减去已接入() {
        let row = DeviceTypeRow {
            label: "摄像头",
            total: 5,
            bound: 2,
            route: None,
        };
        assert_eq!(row.unbound(), 3);
    }

    #[test]
    fn 分类与类型都有中文名() {
        assert_eq!(class_label(CLASS_SENSOR), "传感器");
        assert_eq!(class_label(CLASS_ACTUATOR), "执行终端");
        assert_eq!(device_type_label("camera"), "摄像头");
        assert_eq!(device_type_label("turnstile"), "人行闸机");
        // 库里出现没见过的键时不能显示成空白，否则一行看上去像是数据丢了。
        assert_eq!(device_type_label("drone"), "未知类型");
        assert_eq!(class_label("说不清"), "未分类");
    }

    #[test]
    fn 公共区域的设备显示为园区公共区域() {
        assert_eq!(factory_label(&[], 0), "园区公共区域");
        assert_eq!(factory_label(&[], 7), "厂房 #7");
    }

    #[test]
    fn 健康等级有中文名且未上报不标红() {
        use crate::components::badge::BadgeVariant;
        assert_eq!(health_label(HEALTH_NORMAL), "正常");
        assert_eq!(health_label(HEALTH_CRITICAL), "已影响工作");
        // 刚建档还没装机就是这个状态，标红会让人以为出了故障。
        assert_eq!(health_label("unknown"), "尚未上报");
        // BadgeVariant 没有 Debug，用 matches! 而不是 assert_eq!——
        // 为一条测试去改组件库的类型定义不值当。
        assert!(matches!(health_variant("unknown"), BadgeVariant::Outline));
        assert!(matches!(
            health_variant(HEALTH_CRITICAL),
            BadgeVariant::Destructive
        ));
    }

    #[test]
    fn 只有警告和严重才需要有人处理() {
        assert!(!health_needs_attention(HEALTH_NORMAL));
        assert!(!health_needs_attention("unknown"));
        assert!(health_needs_attention(HEALTH_WARNING));
        assert!(health_needs_attention(HEALTH_CRITICAL));
    }

    #[test]
    fn 相对时长按量级切换单位() {
        let now = 1_000_000_000_i64;
        assert_eq!(elapsed_label(now, now), "刚刚");
        assert_eq!(elapsed_label(now - 90 * 1_000_000, now), "1 分钟");
        assert_eq!(elapsed_label(now - 7_200 * 1_000_000, now), "2 小时");
        assert_eq!(elapsed_label(now - 3 * 86_400 * 1_000_000, now), "3 天");
        // 时钟回拨或跨机器时差会让差值为负，不能显示成一个荒唐的大数字。
        assert_eq!(elapsed_label(now + 5_000_000, now), "刚刚");
    }
}
