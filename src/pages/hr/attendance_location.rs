//! 打卡点设置：老板在这里定「只准在哪儿打卡」。
//!
//! 在这个页面出现之前，打卡完全不限地点——服务端只检查经纬度是不是合法数字
//! （防定位失败），员工在家、在外地照样打卡成功。
//!
//! 坐标不要求手输。「取当前位置」直接读浏览器定位，老板站在门口点一下就完事；
//! 手抄经纬度既容易抄错，也没人记得住自己公司的坐标是多少。

use dioxus::prelude::*;

use super::navigation::HrmNavigation;
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        textarea::Textarea,
        ConfirmDialog,
    },
    permissions::can_manage_hr,
    services::{
        create_attendance_location_record, delete_attendance_location_record,
        update_attendance_location_record,
    },
    spacetime_bindings::{
        attendance_location_input_type::AttendanceLocationInput,
        attendance_location_type::AttendanceLocation,
    },
    state::WorkspaceState,
};

/// 坐标的定点倍数，与服务端一致。
const COORD_SCALE: f64 = 1e15;
/// 新建时的默认半径。城区 GPS 误差常有 20～100 米，给小了会把正常上班的人挡在门外。
const DEFAULT_RADIUS_METRES: i32 = 300;

fn to_degrees(value: i64) -> f64 {
    value as f64 / COORD_SCALE
}

fn parse_degrees(value: &str, label: &str) -> Result<i64, String> {
    value
        .trim()
        .parse::<f64>()
        .map(|degrees| (degrees * COORD_SCALE) as i64)
        .map_err(|_| format!("{label}必须是数字"))
}

/// 读浏览器定位，只取经纬度。
///
/// 打卡页那个 `browser_punch_context` 还要设备号和机型，用来做换机检测；设打卡
/// 点不需要那些，单独发一次更短的脚本。
#[cfg(target_arch = "wasm32")]
async fn current_position() -> Result<(f64, f64), String> {
    let script = r#"navigator.geolocation.getCurrentPosition((p)=>dioxus.send({longitude:p.coords.longitude,latitude:p.coords.latitude}),(e)=>dioxus.send({error:e.message}),{enableHighAccuracy:true,timeout:12000,maximumAge:0});"#;
    let mut eval = dioxus::document::eval(script);
    let value: serde_json::Value = eval
        .recv()
        .await
        .map_err(|error| format!("读取浏览器定位失败：{error}"))?;
    if let Some(message) = value.get("error").and_then(|item| item.as_str()) {
        return Err(format!("无法获取定位：{message}"));
    }
    let longitude = value
        .get("longitude")
        .and_then(|item| item.as_f64())
        .ok_or("定位数据里没有经度")?;
    let latitude = value
        .get("latitude")
        .and_then(|item| item.as_f64())
        .ok_or("定位数据里没有纬度")?;
    // 定位失败时有的浏览器会回 (0, 0)，那是几内亚湾，不是任何人的办公室。
    if longitude.abs() < 0.000_001 && latitude.abs() < 0.000_001 {
        return Err("定位结果无效，请确认已授予定位权限后重试".into());
    }
    Ok((longitude, latitude))
}

#[cfg(not(target_arch = "wasm32"))]
async fn current_position() -> Result<(f64, f64), String> {
    Err("取当前位置需要在浏览器中执行".into())
}

#[component]
pub fn HrmAttendanceLocationPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let locations = (state.attendance_locations)();
    let can_manage = can_manage_hr(&(state.roles)(), &(state.permission_codes)());

    let mut editing = use_signal(|| None::<Option<AttendanceLocation>>);
    let mut removing = use_signal(|| None::<AttendanceLocation>);
    let mut notice = use_signal(|| None::<String>);

    rsx! {
        main { class: "page",
            HrmNavigation { active: String::from("location") }
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "打卡点设置" }
                    p { class: "page-subtitle",
                        "员工必须站在其中任意一个打卡点的范围内才能打卡。一个点都没设时不限制地点。"
                    }
                }
                if can_manage {
                    div { class: "page-actions",
                        Button { onclick: move |_| editing.set(Some(None)), "新增打卡点" }
                    }
                }
            }
            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }
            Card {
                CardContent {
                    if locations.is_empty() {
                        p { class: "hint",
                            "还没有打卡点，当前不限制打卡地点——员工在任何地方都能打卡。点右上角新增一个，站在公司门口用「取当前位置」即可。"
                        }
                    } else {
                        div { class: "table-shell",
                            table { class: "table",
                                thead {
                                    tr {
                                        th { "名称" }
                                        th { "参考地址" }
                                        th { "坐标" }
                                        th { "允许半径" }
                                        th { "状态" }
                                        if can_manage { th { "操作" } }
                                    }
                                }
                                tbody {
                                    for row in locations.iter() {
                                        {
                                            let edit_row = row.clone();
                                            let delete_row = row.clone();
                                            rsx! {
                                                tr { key: "location-{row.location_id}",
                                                    td { strong { "{row.location_name}" } }
                                                    td { class: "is-wrap hint",
                                                        "{row.address.clone().unwrap_or_else(|| String::from(\"--\"))}"
                                                    }
                                                    td { class: "is-mono",
                                                        "{to_degrees(row.longitude_e_15):.6}, {to_degrees(row.latitude_e_15):.6}"
                                                    }
                                                    td { class: "is-mono", "{row.radius_metres} 米" }
                                                    td {
                                                        Badge {
                                                            variant: if row.is_enabled { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                                            if row.is_enabled { "启用" } else { "停用" }
                                                        }
                                                    }
                                                    if can_manage {
                                                        td {
                                                            div { class: "table-actions",
                                                                Button {
                                                                    variant: ButtonVariant::Outline,
                                                                    size: ButtonSize::Sm,
                                                                    onclick: move |_| editing.set(Some(Some(edit_row.clone()))),
                                                                    "编辑"
                                                                }
                                                                Button {
                                                                    variant: ButtonVariant::Outline,
                                                                    size: ButtonSize::Sm,
                                                                    onclick: move |_| removing.set(Some(delete_row.clone())),
                                                                    "删除"
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
        if let Some(target) = editing() {
            LocationFormDialog {
                location: target,
                on_close: move |_| editing.set(None),
                on_saved: move |_| {
                    editing.set(None);
                    notice.set(Some("打卡点已保存。".into()));
                },
            }
        }
        if let Some(row) = removing() {
            {
                let location_id = row.location_id;
                rsx! {
                    ConfirmDialog {
                        title: String::from("删除打卡点"),
                        description: format!(
                            "删除「{}」后，这个位置不再能打卡。已有的考勤记录不受影响。",
                            row.location_name,
                        ),
                        confirm_label: String::from("确认删除"),
                        on_cancel: move |_| removing.set(None),
                        on_confirm: move |_| {
                            spawn(async move {
                                match delete_attendance_location_record(location_id).await {
                                    Ok(()) => {
                                        removing.set(None);
                                        notice.set(Some("打卡点已删除。".into()));
                                    }
                                    Err(message) => notice.set(Some(message)),
                                }
                            });
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn LocationFormDialog(
    location: Option<AttendanceLocation>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let location_id = location.as_ref().map(|row| row.location_id);
    let mut name = use_signal_sync(|| {
        location
            .as_ref()
            .map(|row| row.location_name.clone())
            .unwrap_or_default()
    });
    let mut address = use_signal_sync(|| {
        location
            .as_ref()
            .and_then(|row| row.address.clone())
            .unwrap_or_default()
    });
    let mut longitude = use_signal_sync(|| {
        location
            .as_ref()
            .map(|row| format!("{:.6}", to_degrees(row.longitude_e_15)))
            .unwrap_or_default()
    });
    let mut latitude = use_signal_sync(|| {
        location
            .as_ref()
            .map(|row| format!("{:.6}", to_degrees(row.latitude_e_15)))
            .unwrap_or_default()
    });
    let mut radius = use_signal_sync(|| {
        location
            .as_ref()
            .map(|row| row.radius_metres.to_string())
            .unwrap_or_else(|| DEFAULT_RADIUS_METRES.to_string())
    });
    let mut enabled = use_signal_sync(|| location.as_ref().is_none_or(|row| row.is_enabled));
    let mut remark = use_signal_sync(|| {
        location
            .as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    let mut loading = use_signal_sync(|| false);
    let mut locating = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);

    let title = if location_id.is_some() {
        "编辑打卡点"
    } else {
        "新增打卡点"
    };

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !loading() && !locating() {
                    on_close.call(());
                }
            },
            DialogTitle { "{title}" }
            DialogDescription { "站在这个位置点「取当前位置」即可，不用手抄经纬度。" }
            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    if loading() {
                        return;
                    }
                    let name_value = name().trim().to_string();
                    if name_value.is_empty() {
                        error.set(Some("请填写打卡点名称".into()));
                        return;
                    }
                    let longitude_e15 = match parse_degrees(&longitude(), "经度") {
                        Ok(value) => value,
                        Err(message) => { error.set(Some(message)); return; }
                    };
                    let latitude_e15 = match parse_degrees(&latitude(), "纬度") {
                        Ok(value) => value,
                        Err(message) => { error.set(Some(message)); return; }
                    };
                    let radius_metres = match radius().trim().parse::<i32>() {
                        Ok(value) => value,
                        Err(_) => { error.set(Some("允许半径必须是整数米".into())); return; }
                    };
                    let input = AttendanceLocationInput {
                        location_name: name_value,
                        address: (!address().trim().is_empty()).then(|| address().trim().to_string()),
                        longitude_e_15: longitude_e15,
                        latitude_e_15: latitude_e15,
                        radius_metres,
                        is_enabled: enabled(),
                        remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                    };
                    loading.set(true);
                    error.set(None);
                    spawn(async move {
                        let saved = match location_id {
                            Some(id) => update_attendance_location_record(id, input).await,
                            None => create_attendance_location_record(input).await,
                        };
                        match saved {
                            Ok(()) => on_saved.call(()),
                            Err(message) => {
                                loading.set(false);
                                error.set(Some(message));
                            }
                        }
                    });
                },
                div { class: "form-grid",
                    div { class: "field is-wide",
                        Label { html_for: "location-name", "打卡点名称" }
                        Input {
                            id: "location-name",
                            value: name(),
                            placeholder: "如：总部办公室、周茂森园区门卫",
                            oninput: move |event: FormEvent| name.set(event.value()),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "location-address", "参考地址（选填）" }
                        Input {
                            id: "location-address",
                            value: address(),
                            placeholder: "只用于核对，判定不看它",
                            oninput: move |event: FormEvent| address.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "location-longitude", "经度" }
                        Input {
                            id: "location-longitude",
                            value: longitude(),
                            inputmode: "decimal",
                            placeholder: "113.123456",
                            oninput: move |event: FormEvent| longitude.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "location-latitude", "纬度" }
                        Input {
                            id: "location-latitude",
                            value: latitude(),
                            inputmode: "decimal",
                            placeholder: "23.123456",
                            oninput: move |event: FormEvent| latitude.set(event.value()),
                        }
                    }
                    div { class: "field is-wide",
                        Button {
                            r#type: "button",
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            disabled: locating(),
                            onclick: move |_| {
                                locating.set(true);
                                error.set(None);
                                spawn(async move {
                                    match current_position().await {
                                        Ok((lon, lat)) => {
                                            longitude.set(format!("{lon:.6}"));
                                            latitude.set(format!("{lat:.6}"));
                                        }
                                        Err(message) => error.set(Some(message)),
                                    }
                                    locating.set(false);
                                });
                            },
                            if locating() { "正在定位…" } else { "取当前位置" }
                        }
                        small { class: "hint", "站在要设为打卡点的地方点这里，浏览器会填好经纬度。" }
                    }
                    div { class: "field",
                        Label { html_for: "location-radius", "允许半径（米）" }
                        Input {
                            id: "location-radius",
                            value: radius(),
                            inputmode: "numeric",
                            oninput: move |event: FormEvent| radius.set(event.value()),
                        }
                        small { class: "hint",
                            "城区 GPS 误差常有 20～100 米，建议不低于 200 米，否则站在门口也可能打不上。"
                        }
                    }
                    div { class: "field",
                        Label { html_for: "location-enabled", "状态" }
                        label { class: "row",
                            input {
                                id: "location-enabled",
                                r#type: "checkbox",
                                checked: enabled(),
                                onchange: move |event: FormEvent| enabled.set(event.checked()),
                            }
                            span { "启用（停用后这个点不参与判定）" }
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "location-remark", "备注（选填）" }
                        Textarea {
                            id: "location-remark",
                            value: remark(),
                            rows: 2,
                            oninput: move |event: FormEvent| remark.set(event.value()),
                        }
                    }
                }
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", "{message}" }
                }
                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        disabled: loading(),
                        onclick: move |_| on_close.call(()),
                        "取消"
                    }
                    Button {
                        r#type: "submit",
                        disabled: loading() || locating(),
                        if loading() { "正在保存…" } else { "确认保存" }
                    }
                }
            }
        }
    }
}
