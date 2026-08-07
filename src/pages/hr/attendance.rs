//! 浏览器定位考勤打卡页面。

use dioxus::prelude::*;
use serde::Deserialize;

use super::{
    model::{attendance_status, format_datetime, now_timestamp, today},
    navigation::HrmNavigation,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize},
        card::{Card, CardContent},
    },
    services::{punch_in_record, punch_out_record},
    spacetime_bindings::{
        attendance_device_input_type::AttendanceDeviceInput,
        attendance_punch_input_type::AttendancePunchInput, attendance_type::Attendance,
    },
    state::WorkspaceState,
};

#[derive(Deserialize)]
struct BrowserPunchContext {
    latitude: f64,
    longitude: f64,
    accuracy: Option<f64>,
    device_id: String,
    model: String,
    system: String,
}

#[cfg(target_arch = "wasm32")]
async fn browser_punch_context() -> Result<BrowserPunchContext, String> {
    let script = r#"const key='yizu_attendance_device_id';let id=localStorage.getItem(key);if(!id){id='web-'+crypto.randomUUID();localStorage.setItem(key,id);}navigator.geolocation.getCurrentPosition((p)=>dioxus.send({accuracy:p.coords.accuracy,latitude:p.coords.latitude,longitude:p.coords.longitude,device_id:id,model:navigator.userAgent,system:navigator.platform||'Web'}), (e)=>dioxus.send({error:e.message}), {enableHighAccuracy:true,timeout:12000,maximumAge:30000});"#;
    let mut eval = document::eval(script);
    let value: serde_json::Value = eval
        .recv()
        .await
        .map_err(|e| format!("读取浏览器定位失败：{e}"))?;
    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return Err(format!("无法获取定位：{error}"));
    }
    serde_json::from_value(value).map_err(|e| format!("定位数据格式错误：{e}"))
}

#[cfg(not(target_arch = "wasm32"))]
async fn browser_punch_context() -> Result<BrowserPunchContext, String> {
    Err("考勤打卡需要在浏览器中执行".into())
}

#[cfg(target_arch = "wasm32")]
fn is_valid_position(latitude: f64, longitude: f64) -> bool {
    latitude.is_finite()
        && longitude.is_finite()
        && latitude.abs() > 0.000_001
        && longitude.abs() > 0.000_001
        && (-90.0..=90.0).contains(&latitude)
        && (-180.0..=180.0).contains(&longitude)
}

#[cfg(not(target_arch = "wasm32"))]
fn is_valid_position(_latitude: f64, _longitude: f64) -> bool {
    true
}

fn to_micro(value: f64) -> i64 {
    (value * 1e15).round() as i64
}

fn to_degrees(value: i64) -> f64 {
    value as f64 / 1e15
}

fn is_today_record(row: &Attendance, today_label: &str) -> bool {
    format_datetime(Some(row.punch_in)).starts_with(today_label)
}

fn locate_today_record(records: &[Attendance], today_label: &str) -> Option<Attendance> {
    records
        .iter()
        .filter(|row| is_today_record(row, today_label))
        .max_by_key(|row| row.punch_in.to_micros_since_unix_epoch())
        .cloned()
}

fn is_punched(record: &Option<Attendance>) -> bool {
    record
        .as_ref()
        .is_some_and(|value| value.punch_out.is_none())
}

fn is_device_abnormal(message: &str) -> bool {
    message.contains("检测到考勤设备异常")
}

#[component]
pub fn HrmAttendancePunchPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let records = (state.attendances)();
    let abnormal_logs = (state.attendance_abnormal_logs)();
    let user = (state.business_user)();

    let today_label = today();
    let record = locate_today_record(&records, &today_label);
    let has_checked_today = record.is_some();
    let has_checked_out = record
        .as_ref()
        .is_some_and(|value| value.punch_out.is_some());
    let can_check_out = is_punched(&record);

    let (status_label, status_variant) = record
        .as_ref()
        .map(|row| attendance_status(row.status))
        .unwrap_or(("尚未打卡", BadgeVariant::Outline));

    let punch_in = record
        .as_ref()
        .map(|row| format_datetime(Some(row.punch_in)))
        .unwrap_or_else(|| "--".into());
    let punch_out = record
        .as_ref()
        .map(|row| format_datetime(row.punch_out))
        .unwrap_or_else(|| "--".into());
    let punch_position = record
        .as_ref()
        .map(|row| {
            format!(
                "{:.6}, {:.6}",
                to_degrees(row.longitude_e_15),
                to_degrees(row.latitude_e_15)
            )
        })
        .unwrap_or_else(|| "未打卡".into());

    let today_log = record
        .as_ref()
        .map(|row| {
            abnormal_logs
                .iter()
                .filter(|log| log.attendance_id == Some(row.attendance_id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let current_name = user
        .as_ref()
        .map(|person| person.real_name.clone())
        .unwrap_or_else(|| "当前员工".into());

    let button_text = if !has_checked_today {
        "上班打卡"
    } else if can_check_out {
        "下班打卡"
    } else {
        "今日考勤已完成"
    };
    let mut confirm_device_abnormal = use_signal(|| false);
    let mut require_confirmation_notice = use_signal(|| false);

    let mut loading = use_signal(|| false);
    let mut notice = use_signal(|| None::<String>);

    rsx! {
        main { class: "page",
            HrmNavigation { active: String::from("punch") }
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "考勤打卡" }
                    p { class: "page-subtitle", "使用当前设备与浏览器实时定位完成上下班打卡。" }
                }
                div { class: "page-actions",
                    span { class: "hint", "{current_name} · {today_label}" }
                }
            }
            {
                // 打卡点是员工要遵守的规则，不该等提交被服务端拒了才知道。
                let locations = (state.attendance_locations)();
                let enabled = locations
                    .iter()
                    .filter(|row| row.is_enabled)
                    .collect::<Vec<_>>();
                rsx! {
                    if enabled.is_empty() {
                        p { class: "hint", "当前未限制打卡地点。" }
                    } else {
                        p { class: "hint",
                            "只能在这些地点打卡："
                            for row in enabled.iter() {
                                span { key: "punch-location-{row.location_id}",
                                    "{row.location_name}（{row.radius_metres} 米内）　"
                                }
                            }
                        }
                    }
                }
            }
            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }
            Card {
                CardContent {
                div { class: "stack",
                div { class: "grid-3",
                    div { class: "panel is-tight is-plain",
                        span { class: "stat-label", "今日状态" }
                        div { class: "row",
                            Badge { variant: status_variant, "{status_label}" }
                        }
                    }
                    div { class: "panel is-tight is-plain",
                        span { class: "stat-label", "上班打卡" }
                        strong { class: "is-mono", "{punch_in}" }
                    }
                    div { class: "panel is-tight is-plain",
                        span { class: "stat-label", "下班打卡" }
                        strong { class: "is-mono", "{punch_out}" }
                    }
                }
                Button {
                    disabled: loading() || has_checked_out,
                    onclick: move |_| {
                        if loading() {
                            return;
                        }

                        let pending_record = record.clone();
                        loading.set(true);
                        notice.set(Some("正在获取高精度定位…".into()));

                        spawn(async move {
                            let context = match browser_punch_context().await {
                                Ok(value) => value,
                                Err(message) => {
                                    loading.set(false);
                                    notice.set(Some(message));
                                    return;
                                }
                            };

                            if !is_valid_position(context.latitude, context.longitude) {
                                loading.set(false);
                                notice.set(Some("定位坐标无效，请重试".into()));
                                return;
                            }

                            if let Some(accuracy) = context.accuracy {
                                if !accuracy.is_finite() || accuracy <= 0.0 {
                                    loading.set(false);
                                    notice.set(Some("定位精度异常，重试后继续".into()));
                                    return;
                                }
                            }

                            let input = AttendancePunchInput {
                                punch_time: now_timestamp(),
                                longitude_e_15: to_micro(context.longitude),
                                latitude_e_15: to_micro(context.latitude),
                                device: AttendanceDeviceInput {
                                    device_id: Some(context.device_id),
                                    device_model: Some(context.model),
                                    device_system: Some(context.system),
                                    bind_current_device: true,
                                    confirm_device_abnormal: confirm_device_abnormal(),
                                },
                            };

                            let result = if let Some(row) = pending_record {
                                punch_out_record(row.attendance_id, input).await
                            } else {
                                punch_in_record(input).await
                            };

                            loading.set(false);
                            match result {
                                Ok(()) => {
                                    confirm_device_abnormal.set(false);
                                    require_confirmation_notice.set(false);
                                    notice.set(Some("打卡成功，记录已同步".into()));
                                }
                                Err(message) => {
                                    if is_device_abnormal(&message) {
                                        require_confirmation_notice.set(true);
                                    } else {
                                        require_confirmation_notice.set(false);
                                    }
                                    notice.set(Some(message));
                                }
                            }
                        });
                    },
                    if loading() {
                        "定位并打卡中…"
                    } else {
                        "{button_text}"
                    }
                }
                if require_confirmation_notice() {
                    label { class: "row",
                        input {
                            r#type: "checkbox",
                            checked: confirm_device_abnormal(),
                            onchange: move |event| confirm_device_abnormal.set(event.checked()),
                        }
                        span { "检测到设备异常，确认后继续打卡" }
                    }
                }

                dl { class: "facts",
                    div {
                        dt { "打卡坐标" }
                        dd { class: "is-mono", "{punch_position}" }
                    }
                    div {
                        dt { "下班状态" }
                        dd {
                            if can_check_out {
                                "待下班"
                            } else if has_checked_today {
                                "已完成"
                            } else {
                                "未打卡"
                            }
                        }
                    }
                }

                p { class: "hint",
                    "定位仅随本次考勤写入 SpacetimeDB；设备标识保存在当前浏览器，用于识别异常换机。"
                }
                }
                }
            }

            if !today_log.is_empty() {
                section { class: "section",
                    div { class: "section-header",
                        h2 { "设备异常日志" }
                        Badge { variant: BadgeVariant::Destructive, "{today_log.len()} 条" }
                    }
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "设备标识" }
                                    th { "异常类型" }
                                    th { "动作" }
                                }
                            }
                            tbody {
                                for log in today_log {
                                    tr { key: "attendance-log-{log.id}",
                                        td { class: "is-mono", "{log.current_device_id}" }
                                        td { "{log.abnormal_type}" }
                                        td { "{log.action}" }
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
