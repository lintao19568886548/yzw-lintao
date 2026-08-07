//! 设备总览：按传感器／执行终端两栏汇总园区里的全部联网设备。
//!
//! 两栏不是排版趣味，是运维节奏的区分：传感器坏了是少一份数据，可以按天处理；
//! 执行终端坏了是有人堵在门口，得按分钟。设计见 `docs/设备管理.md` §2.2。
//!
//! 这一页显示的是**接入率**而不是在线率——填了平台设备号只说明能取到状态，
//! 不代表此刻在线。真正的在线率要向厂商平台实时查，摄像头与门禁那条链路还没接。

use dioxus::prelude::*;

use super::model::*;
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
    },
    permissions::can_access_route,
    router::Route,
    state::WorkspaceState,
};

#[component]
pub fn DeviceOverviewPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let summary = summarize_devices(&state.device_assets.read(), &state.utility_meters.read());

    let roles = state.roles.read().clone();
    let menus = state.menus.read().clone();
    let route_for = |path: &str| -> Option<Route> {
        if !can_access_route(&roles, &menus, path) {
            return None;
        }
        match path {
            "/device/camera" => Some(Route::DeviceCameraPage {}),
            "/device/access" => Some(Route::DeviceAccessPage {}),
            "/smart-meter/meter" => Some(Route::SmartElectricMeterPage {}),
            _ => None,
        }
    };

    let sensors = summary
        .sensors
        .iter()
        .map(|row| (row.clone(), row.route.and_then(|path| route_for(path))))
        .collect::<Vec<_>>();
    let actuators = summary
        .actuators
        .iter()
        .map(|row| (row.clone(), row.route.and_then(|path| route_for(path))))
        .collect::<Vec<_>>();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "设备总览" }
                    p { class: "page-subtitle",
                        "园区内的联网设备按传感器与执行终端两类汇总。传感器故障影响的是数据采集，执行终端故障将直接影响通行，两者的处置时限不同。"
                    }
                }
            }

            section { class: "grid-4", aria_label: "设备总量",
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "设备总数" }
                    strong { class: "stat-value is-mono", "{summary.total()}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "传感器" }
                    strong { class: "stat-value is-mono", "{summary.sensor_total()}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "执行终端" }
                    strong { class: "stat-value is-mono", "{summary.actuator_total()}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "已接入平台" }
                    strong { class: "stat-value is-mono", "{summary.bound_total()} · {summary.bound_percent()}%" }
                } } } }
            }

            section { class: "grid-2",
                DeviceClassColumn {
                    title: "传感器",
                    caption: "用于读取物理世界的状态。离线可按日处理，在月度结账前恢复即可。",
                    rows: sensors,
                }
                DeviceClassColumn {
                    title: "执行终端",
                    caption: "用于接受指令并改变物理世界的状态。离线将立即影响现场通行，须按分钟级响应。",
                    rows: actuators,
                }
            }

            section { class: "section",
                Card { CardContent {
                    p { class: "hint",
                        "「已接入平台」统计的是已填写平台设备号的设备，表示其具备获取状态的条件，不代表当前在线。目前仅水电表已接通厂商接口（合众、YMSINO），实时回传情况请见智能水电表管理页的「回传覆盖」；摄像头与门禁的平台接口尚未接入，在线状态与离线告警需待其完成。"
                    }
                    p { class: "hint",
                        "水电表台账在园区管理中维护、在智能水电表管理页完成绑定，本页仅将其一并计入。设备管理不作为第二个编辑入口。"
                    }
                }}
            }
        }
    }
}

#[component]
fn DeviceClassColumn(
    title: String,
    caption: String,
    rows: Vec<(DeviceTypeRow, Option<Route>)>,
) -> Element {
    let total: usize = rows.iter().map(|(row, _)| row.total).sum();
    rsx! {
        div {
            Card { CardContent {
                div { class: "section-header",
                    h2 { "{title}" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 台" }
                }
                p { class: "hint", "{caption}" }
                if total == 0 {
                    p { class: "empty", "该类别暂无已登记设备。" }
                } else {
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "类型" }
                                    th { "台数" }
                                    th { "已接入 / 未接入" }
                                    th { "" }
                                }
                            }
                            tbody {
                                for (row , route) in rows.iter() {
                                    tr { key: "{row.label}",
                                        td { "{row.label}" }
                                        td { class: "is-mono", "{row.total}" }
                                        td {
                                            span { class: "is-mono", "{row.bound}" }
                                            " / "
                                            span { class: "is-mono", "{row.unbound()}" }
                                        }
                                        td {
                                            if let Some(route) = route.clone() {
                                                Link { to: route,
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        "查看"
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
            }}
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[component]
    fn DeviceOverviewRenderTestRoot() -> Element {
        let state = crate::app::use_workspace_state();
        use_context_provider(|| state);
        rsx! { DeviceOverviewPage {} }
    }

    #[test]
    fn 设备总览页可以完成首次渲染() {
        let html = dioxus_ssr::render_element(rsx! { DeviceOverviewRenderTestRoot {} });
        assert!(html.contains("设备总览"), "页面标题没有进入渲染结果");
        assert!(html.contains("传感器"), "传感器栏没有进入渲染结果");
        assert!(html.contains("执行终端"), "执行终端栏没有进入渲染结果");
    }
}
