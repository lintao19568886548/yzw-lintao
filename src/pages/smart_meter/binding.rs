//! 合众设备与水电表台账的绑定。
//!
//! 台账只有一份（`utility_meter`，属于园区资产），绑定关系也只有一列
//! （`external_device_id`）。这一页不再维护第二份表档案，只负责把供应商的
//! 设备接到台账上——因为供应商的设备列表在这里，在园区详情页凭记忆手敲设备
//! 号本来就是反的：敲错一位不会报错，只会让读数永远拉不到。

use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent, CardHeader, CardTitle},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        ConfirmDialog,
    },
    pages::{
        decode_location, encode_location, meter_location_label, meter_location_options, PUBLIC_AREA,
    },
    permissions::can_manage_rental,
    services::{
        create_utility_meter_record, update_utility_meter_record, MeterKind, SmartMeterDevice,
    },
    spacetime_bindings::{
        utility_meter_input_type::UtilityMeterInput, utility_meter_type::UtilityMeter,
    },
    state::WorkspaceState,
};

/// 设备号 → 已绑定的台账表。
///
/// 已注销的表不算绑定——否则删掉一块表之后，它占着的设备号永远显示"已绑定"，
/// 而服务端的唯一性守卫只看未注销的行，两边判断会打架。
pub(super) fn binding_index(meters: &[UtilityMeter]) -> BTreeMap<String, UtilityMeter> {
    meters
        .iter()
        .filter(|meter| !meter.is_deleted)
        .filter_map(|meter| {
            meter
                .external_device_id
                .as_deref()
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(|id| (id.to_string(), meter.clone()))
        })
        .collect()
}

/// 供应商倍率转成台账用的百分之一单位。
///
/// 合众返回的是浮点数（`current_ratio` 是 "1"、"200" 这样的字符串，
/// `multiplier` 是解析后的 f64）。四舍五入而不是截断：倍率 1.5 截成 1 会让
/// 整块表的用量少算三分之一。
pub(super) fn multiplier_centi(multiplier: f64) -> i64 {
    if !multiplier.is_finite() || multiplier <= 0.0 {
        // 供应商没给或给了非法值时退回 1 倍，而不是 0——0 倍会让用量恒为零。
        return 100;
    }
    (multiplier * 100.0).round() as i64
}

/// 设备在供应商侧的位置描述，用于让人认出这是哪一台。
pub(super) fn vendor_location(device: &SmartMeterDevice) -> String {
    [
        device.building_name.trim(),
        device.floor_name.trim(),
        device.room_name.trim(),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

#[component]
pub(super) fn MeterBindingSection(kind: MeterKind, devices: Vec<SmartMeterDevice>) -> Element {
    let state = use_context::<WorkspaceState>();
    let can_manage = can_manage_rental(&(state.roles)(), &(state.menus)());
    let mut importing = use_signal(|| None::<SmartMeterDevice>);
    let mut unbinding = use_signal(|| None::<UtilityMeter>);
    let mut notice = use_signal(|| None::<String>);

    let meters = (state.utility_meters)();
    let index = binding_index(&meters);
    let bound_count = devices
        .iter()
        .filter(|device| index.contains_key(device.device_id.trim()))
        .count();
    let parks = (state.parks)();
    let factories = (state.factories)();
    let factory_floors = (state.factory_floors)();
    let dormitories = (state.dormitories)();
    let dormitory_floors = (state.dormitory_floors)();

    rsx! {
        div { class: "section",
            Card {
                CardHeader {
                    div { class: "section-header",
                        CardTitle { "设备绑定台账" }
                        Badge {
                            variant: if bound_count == devices.len() && !devices.is_empty() {
                                BadgeVariant::Primary
                            } else {
                                BadgeVariant::Outline
                            },
                            "已绑定 {bound_count} / {devices.len()}"
                        }
                    }
                }
                CardContent {
                    p { class: "hint",
                        "绑定之后，这台设备的读数才能在账单里按合同单价自动带出。表号、倍率、分时都从合众带过来，你只需要指定园区和安装位置。"
                    }
                    if let Some(message) = notice() {
                        p { class: "notice", role: "status", "{message}" }
                    }
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "供应商位置" }
                                    th { "设备号" }
                                    th { "出厂号" }
                                    th { "倍率" }
                                    th { "台账绑定" }
                                    if can_manage { th { "操作" } }
                                }
                            }
                            tbody {
                                if devices.is_empty() {
                                    tr {
                                        td { class: "table-empty", colspan: if can_manage { 6 } else { 5 },
                                            "尚未读取到设备档案"
                                        }
                                    }
                                } else {
                                    for device in devices.iter() {
                                        {
                                            let device_id = device.device_id.trim().to_string();
                                            let bound = index.get(&device_id).cloned();
                                            let location = vendor_location(device);
                                            let import_device = device.clone();
                                            let unbind_target = bound.clone();
                                            rsx! {
                                                tr { key: "bind-{device.device_id}-{device.factory_no}",
                                                    td {
                                                        div { class: "stack-tight",
                                                            strong { "{location}" }
                                                            small { class: "hint", "{device.park_name}" }
                                                        }
                                                    }
                                                    td { class: "is-mono", "{device.device_id}" }
                                                    td { class: "is-mono", "{device.factory_no}" }
                                                    td { class: "is-mono", "{device.current_ratio}" }
                                                    td {
                                                        if let Some(meter) = bound.as_ref() {
                                                            {
                                                                let park_name = parks
                                                                    .iter()
                                                                    .find(|park| park.park_id == meter.park_id)
                                                                    .map(|park| park.park_name.clone())
                                                                    .unwrap_or_else(|| "未知园区".into());
                                                                let install = meter_location_label(
                                                                    meter,
                                                                    &factories,
                                                                    &factory_floors,
                                                                    &dormitories,
                                                                    &dormitory_floors,
                                                                );
                                                                rsx! {
                                                                    div { class: "stack-tight",
                                                                        strong { "{meter.meter_code}" }
                                                                        small { class: "hint", "{park_name} · {install}" }
                                                                    }
                                                                }
                                                            }
                                                        } else {
                                                            Badge { variant: BadgeVariant::Outline, "未绑定" }
                                                        }
                                                    }
                                                    if can_manage {
                                                        td {
                                                            if bound.is_some() {
                                                                Button {
                                                                    size: ButtonSize::Sm,
                                                                    variant: ButtonVariant::Outline,
                                                                    r#type: "button",
                                                                    onclick: move |_| unbinding.set(unbind_target.clone()),
                                                                    "解绑"
                                                                }
                                                            } else {
                                                                Button {
                                                                    size: ButtonSize::Sm,
                                                                    r#type: "button",
                                                                    onclick: move |_| importing.set(Some(import_device.clone())),
                                                                    "导入到园区"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(device) = importing() {
            DeviceImportDialog {
                kind,
                device,
                on_close: move |_| importing.set(None),
                on_saved: move |message: String| {
                    importing.set(None);
                    notice.set(Some(message));
                },
            }
        }

        if let Some(meter) = unbinding() {
            UnbindDialog {
                meter,
                on_close: move |_| unbinding.set(None),
                on_saved: move |message: String| {
                    unbinding.set(None);
                    notice.set(Some(message));
                },
            }
        }
    }
}

/// 把一台合众设备导入成台账里的表。
///
/// 除了园区和安装位置，其余字段全部从供应商数据预填——手敲表号和倍率是这条
/// 链路上最容易出错、也最不必要的一步。
#[component]
fn DeviceImportDialog(
    kind: MeterKind,
    device: SmartMeterDevice,
    on_close: EventHandler<()>,
    on_saved: EventHandler<String>,
) -> Element {
    let state = use_context::<WorkspaceState>();
    let is_electric = kind == MeterKind::Electric;
    let device_id = device.device_id.trim().to_string();
    let mut code = use_signal(|| {
        // 出厂号是印在表壳上的编号，抄表对账时认的就是它；缺失才退回设备号。
        let factory_no = device.factory_no.trim();
        if factory_no.is_empty() {
            device.device_id.trim().to_string()
        } else {
            factory_no.to_string()
        }
    });
    let parks = (state.parks)()
        .into_iter()
        .filter(|park| !park.is_deleted)
        .collect::<Vec<_>>();
    let mut park_id = use_signal(|| parks.first().map(|park| park.park_id).unwrap_or(0));
    let mut location = use_signal(|| PUBLIC_AREA.to_string());
    // 电表默认按分时计量：合众的读数本来就分尖峰平谷四段回传。
    let mut time_of_use = use_signal(|| is_electric);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let selected_park = park_id();
    let options = meter_location_options(
        selected_park,
        &(state.factories)(),
        &(state.factory_floors)(),
        &(state.dormitories)(),
        &(state.dormitory_floors)(),
    );
    let park_value: ReadSignal<Option<String>> =
        use_memo(move || Some(park_id().to_string())).into();
    let location_value: ReadSignal<Option<String>> = use_memo(move || Some(location())).into();
    let multiplier = multiplier_centi(device.multiplier);
    let vendor_place = vendor_location(&device);
    let device_id_for_submit = device_id.clone();
    let kind_label = kind.label();

    rsx! {
        Dialog {
            open: true,
            is_modal: true,
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { "导入{kind_label}到园区台账" }
            DialogDescription {
                "合众设备 {device_id}（{vendor_place}）。表号和倍率已从设备档案带出，指定它装在哪个园区的哪一层即可。"
            }
            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    if loading() { return; }
                    let code_value = code().trim().to_string();
                    if code_value.is_empty() { error.set(Some("请填写表号".into())); return; }
                    if park_id() == 0 { error.set(Some("请选择所属园区".into())); return; }
                    // 设备号是读数的唯一线索，供应商没给就绑不出有效关系——
                    // 绑一个空字符串只会得到一块永远拉不到数的"智能表"。
                    if device_id_for_submit.trim().is_empty() {
                        error.set(Some("这台设备没有返回设备号，无法绑定".into()));
                        return;
                    }
                    let (factory_floor_id, dormitory_floor_id) = decode_location(&location());
                    let input = UtilityMeterInput {
                        park_id: park_id(),
                        meter_code: code_value.clone(),
                        is_electric,
                        factory_floor_id,
                        dormitory_floor_id,
                        multiplier_centi: multiplier,
                        is_time_of_use: is_electric && time_of_use(),
                        external_device_id: Some(device_id_for_submit.clone()),
                        remark: None,
                    };
                    error.set(None);
                    loading.set(true);
                    spawn(async move {
                        match create_utility_meter_record(input).await {
                            Ok(()) => {
                                loading.set(false);
                                on_saved.call(format!("设备已绑定到表「{code_value}」"));
                            }
                            Err(message) => {
                                loading.set(false);
                                error.set(Some(message));
                            }
                        }
                    });
                },
                div { class: "form-grid",
                    div { class: "field",
                        Label { html_for: "import-meter-code", "表号" }
                        Input {
                            id: "import-meter-code",
                            value: code(),
                            maxlength: 60,
                            oninput: move |event: FormEvent| code.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "import-meter-park", "所属园区" }
                        Select {
                            id: "import-meter-park",
                            value: Some(park_value),
                            on_value_change: move |value: Option<String>| {
                                if let Some(id) = value.and_then(|value| value.parse::<u64>().ok())
                                {
                                    park_id.set(id);
                                    // 换园区之后原来的楼层不再属于这个园区，退回公共区域。
                                    location.set(PUBLIC_AREA.to_string());
                                }
                            },
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    value: park.park_id.to_string(),
                                    index,
                                    text_value: park.park_name.clone(),
                                    "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "import-meter-location", "安装位置" }
                        Select {
                            id: "import-meter-location",
                            value: Some(location_value),
                            on_value_change: move |value: Option<String>| {
                                location.set(value.unwrap_or_else(|| PUBLIC_AREA.to_string()));
                            },
                            for (index , (value , label)) in options.iter().enumerate() {
                                SelectOption::<String> {
                                    value: value.clone(),
                                    index,
                                    text_value: label.clone(),
                                    "{label}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "倍率" }
                        p { class: "is-mono", "{device.current_ratio}" }
                        small { class: "hint", "由合众设备档案给出，导入后可在园区详情里修改" }
                    }
                    if is_electric {
                        div { class: "field",
                            label { class: "checkbox-row",
                                input {
                                    r#type: "checkbox",
                                    checked: time_of_use(),
                                    onchange: move |event: FormEvent| {
                                        time_of_use.set(event.checked());
                                    },
                                }
                                "这块表分时计量（尖 / 峰 / 平 / 谷）"
                            }
                            small { class: "hint", "合众按四段回传读数，分时表在账单里会拆成四行" }
                        }
                    }
                }
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", "{message}" }
                }
                footer { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        disabled: loading(),
                        onclick: move |_| on_close.call(()),
                        "取消"
                    }
                    Button { r#type: "submit", disabled: loading(),
                        if loading() { "绑定中…" } else { "确认导入" }
                    }
                }
            }
        }
    }
}

/// 解绑：只清掉设备号，表本身留在台账里。
#[component]
fn UnbindDialog(
    meter: UtilityMeter,
    on_close: EventHandler<()>,
    on_saved: EventHandler<String>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let meter_code = meter.meter_code.clone();
    let device_id = meter.external_device_id.clone().unwrap_or_default();

    rsx! {
        ConfirmDialog {
            title: "解除设备绑定",
            description: format!(
                "表「{meter_code}」将不再关联合众设备 {device_id}，读数需要手工抄录。表和它的合同报价都会保留。",
            ),
            confirm_label: "确认解绑",
            busy: loading(),
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() { return; }
                loading.set(true);
                error.set(None);
                // 解绑就是把设备号清空，其余字段原样回填——这里不能少填任何一项，
                // 更新是整行替换而不是打补丁。
                let input = UtilityMeterInput {
                    park_id: meter.park_id,
                    meter_code: meter.meter_code.clone(),
                    is_electric: meter.is_electric,
                    factory_floor_id: meter.factory_floor_id,
                    dormitory_floor_id: meter.dormitory_floor_id,
                    multiplier_centi: meter.multiplier_centi,
                    is_time_of_use: meter.is_time_of_use,
                    external_device_id: None,
                    remark: meter.remark.clone(),
                };
                let meter_id = meter.meter_id;
                let code = meter.meter_code.clone();
                spawn(async move {
                    match update_utility_meter_record(meter_id, input).await {
                        Ok(()) => {
                            loading.set(false);
                            on_saved.call(format!("表「{code}」已解除设备绑定"));
                        }
                        Err(message) => {
                            loading.set(false);
                            error.set(Some(message));
                        }
                    }
                });
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacetimedb_sdk::Timestamp;

    fn meter(meter_id: u64, device_id: Option<&str>, is_deleted: bool) -> UtilityMeter {
        UtilityMeter {
            meter_id,
            customer_id: "c1".into(),
            park_id: 1,
            meter_code: format!("A{meter_id}"),
            is_electric: true,
            factory_floor_id: 0,
            dormitory_floor_id: 0,
            multiplier_centi: 100,
            is_time_of_use: true,
            external_device_id: device_id.map(str::to_string),
            remark: None,
            is_deleted,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 只有未注销且填了设备号的表算已绑定() {
        let rows = vec![
            meter(1, Some("00998"), false),
            meter(2, None, false),
            meter(3, Some("00999"), true),
            // 空白字符串不算绑定，否则界面会显示一个绑到空设备的表。
            meter(4, Some("   "), false),
        ];
        let index = binding_index(&rows);
        assert_eq!(index.len(), 1);
        assert_eq!(index.get("00998").map(|m| m.meter_id), Some(1));
        assert!(index.get("00999").is_none(), "已注销的表不占设备号");
    }

    #[test]
    fn 设备号两侧空白会被裁掉后建索引() {
        let index = binding_index(&[meter(1, Some(" 00998 "), false)]);
        assert!(index.contains_key("00998"));
    }

    #[test]
    fn 倍率四舍五入且非法值退回一倍() {
        assert_eq!(multiplier_centi(1.0), 100);
        assert_eq!(multiplier_centi(200.0), 20_000);
        // 1.5 倍截断成 1 会让整块表的用量少算三分之一。
        assert_eq!(multiplier_centi(1.5), 150);
        assert_eq!(multiplier_centi(1.005), 100);
        assert_eq!(multiplier_centi(0.0), 100);
        assert_eq!(multiplier_centi(-3.0), 100);
        assert_eq!(multiplier_centi(f64::NAN), 100);
    }

    #[test]
    fn 供应商位置按楼栋楼层房号拼接并跳过空段() {
        let mut device = SmartMeterDevice {
            park_id: "241".into(),
            park_name: "周茂森园区".into(),
            building_name: "A栋".into(),
            floor_name: "一楼".into(),
            room_id: "1".into(),
            room_name: "101".into(),
            device_id: "00998".into(),
            factory_no: "8303".into(),
            protocol: "DLT645".into(),
            current_ratio: "1".into(),
            multiplier: 1.0,
        };
        assert_eq!(vendor_location(&device), "A栋 · 一楼 · 101");
        device.floor_name = String::new();
        assert_eq!(vendor_location(&device), "A栋 · 101");
    }

    #[test]
    fn 编码解码与共用的安装位置候选一致() {
        // 绑定对话框和园区详情共用同一套编码，这里钉住它不会各走一套。
        assert_eq!(decode_location(&encode_location(9, 0)), (9, 0));
        assert_eq!(decode_location(PUBLIC_AREA), (0, 0));
    }
}
