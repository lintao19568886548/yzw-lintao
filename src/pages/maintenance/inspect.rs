//! 手机扫码巡检页。
//!
//! 设备二维码指向 `/maintenance/inspect/transformer/{id}`。二维码只是入口，
//! 不构成授权：未登录会先走登录，登录后要求持有 `maintenance:inspect`
//! 权限码（或 Super），服务端 Reducer 再做同口径校验与园区数据范围检查。
//! 页面对任何已登录账号放行（`permissions::routes` 有对应豁免），因此无权限
//! 时在这里给出明确提示，而不是把人挡在一个空白页外。

use dioxus::prelude::*;

use super::elevator::ElevatorInspectionForm;
use super::firefighting::{location_label, FirefightingInspectionForm};
use super::model::*;
use super::transformer::{can_submit_inspection, TransformerInspectionForm};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
    },
    state::WorkspaceState,
};

#[component]
pub fn MaintenanceInspectTransformerPage(id: u64) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut done = use_signal(|| false);
    let can_inspect = can_submit_inspection(&state);
    let asset = state
        .transformer_assets
        .read()
        .iter()
        .find(|row| row.asset_id == id)
        .cloned();
    let parks = state.parks.read().clone();
    let factories = state.factories.read().clone();
    let inspections = state.transformer_inspections.read().clone();

    rsx! {
        main { class: "page inspect-page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "设备巡检" }
                    p { class: "page-subtitle", "确认设备信息后填写本次巡检结果。" }
                }
            }
            if let Some(asset) = asset {
                section { class: "section",
                    Card { CardContent { div { class: "stack-tight",
                        div { class: "row",
                            strong { "{asset.transformer_name}" }
                            small { class: "hint is-mono", "POWER-{asset.asset_id:06}" }
                        }
                        small { class: "hint",
                            {
                                let factory = if asset.factory_id == 0 {
                                    "园区公共区域".to_string()
                                } else {
                                    factory_name(&factories, Some(asset.factory_id))
                                };
                                format!("{} · {}", park_name(&parks, asset.park_id), factory)
                            }
                        }
                        small { class: "hint", "{asset.location}" }
                        small { class: "hint", "规格 {asset.specifications} · 容量 {format_capacity_kw(asset.capacity_centi_kw)}" }
                        if let Some(latest) = latest_inspection(&inspections, asset.asset_id) {
                            div { class: "row",
                                span { class: "hint", "上次巡检" }
                                Badge { variant: status_variant(&latest.status), "{latest.status}" }
                                small { class: "hint is-mono", "{format_datetime(latest.check_time)}" }
                                small { class: "hint", "{latest.inspector_name}" }
                            }
                        } else {
                            small { class: "hint", "这台设备还没有巡检记录，本次是第一条。" }
                        }
                    } } }
                }
                if done() {
                    section { class: "section",
                        Card { CardContent { div { class: "stack",
                            Badge { variant: BadgeVariant::Secondary, "巡检已提交" }
                            p { "记录已写入台账，可以继续扫下一台设备。" }
                            Button {
                                variant: ButtonVariant::Outline,
                                r#type: "button",
                                onclick: move |_| done.set(false),
                                "再登记一次"
                            }
                        } } }
                    }
                } else if can_inspect {
                    section { class: "section",
                        Card { CardContent {
                            TransformerInspectionForm {
                                asset: asset.clone(),
                                on_saved: move |_| done.set(true),
                            }
                        } }
                    }
                } else {
                    section { class: "section",
                        Card { CardContent { div { class: "stack",
                            p { class: "form-error", "当前账号没有巡检权限。" }
                            p { class: "hint", "请联系管理员在权限管理中授予「设施巡检」权限码后重试。" }
                        } } }
                    }
                }
            } else {
                section { class: "section",
                    Card { CardContent { div { class: "stack",
                        p { class: "form-error", "设备不存在或已注销。" }
                        p { class: "hint", "如果设备台账刚刚变更，请稍候片刻等待数据同步，或联系管理员确认这台设备的状态。" }
                    } } }
                }
            }
        }
    }
}

#[component]
pub fn MaintenanceInspectElevatorPage(id: u64) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut done = use_signal(|| false);
    let can_inspect = can_submit_inspection(&state);
    let asset = state
        .elevator_assets
        .read()
        .iter()
        .find(|row| row.asset_id == id)
        .cloned();
    let parks = state.parks.read().clone();
    let factories = state.factories.read().clone();
    let inspections = state.elevator_inspections.read().clone();

    rsx! {
        main { class: "page inspect-page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "电梯巡检" }
                    p { class: "page-subtitle", "确认设备信息后填写本次巡检结果。" }
                }
            }
            if let Some(asset) = asset {
                section { class: "section",
                    Card { CardContent { div { class: "stack-tight",
                        div { class: "row",
                            strong { "{asset.elevator_name}" }
                            small { class: "hint is-mono", "LIFT-{asset.asset_id:06}" }
                        }
                        small { class: "hint",
                            "{park_name(&parks, asset.park_id)} · {factory_name(&factories, Some(asset.factory_id))}"
                        }
                        small { class: "hint", "{asset.location}" }
                        small { class: "hint",
                            {
                                let size = asset.size.clone().unwrap_or_else(|| "未记录".into());
                                format!("轿厢 {} · 承重 {}", size, format_load_kg(asset.load_capacity_centi_kg))
                            }
                        }
                        if let Some(latest) = latest_elevator_inspection(&inspections, asset.asset_id) {
                            div { class: "row",
                                span { class: "hint", "上次巡检" }
                                Badge { variant: status_variant(&latest.status), "{latest.status}" }
                                small { class: "hint is-mono", "{format_datetime(latest.check_time)}" }
                                small { class: "hint", "{latest.inspector_name}" }
                            }
                        } else {
                            small { class: "hint", "这台设备还没有巡检记录，本次是第一条。" }
                        }
                    } } }
                }
                if done() {
                    section { class: "section",
                        Card { CardContent { div { class: "stack",
                            Badge { variant: BadgeVariant::Secondary, "巡检已提交" }
                            p { "记录已写入台账，可以继续扫下一台设备。" }
                            Button {
                                variant: ButtonVariant::Outline,
                                r#type: "button",
                                onclick: move |_| done.set(false),
                                "再登记一次"
                            }
                        } } }
                    }
                } else if can_inspect {
                    section { class: "section",
                        Card { CardContent {
                            ElevatorInspectionForm {
                                asset: asset.clone(),
                                on_saved: move |_| done.set(true),
                            }
                        } }
                    }
                } else {
                    section { class: "section",
                        Card { CardContent { div { class: "stack",
                            p { class: "form-error", "当前账号没有巡检权限。" }
                            p { class: "hint", "请联系管理员在权限管理中授予「设施巡检」权限码后重试。" }
                        } } }
                    }
                }
            } else {
                section { class: "section",
                    Card { CardContent { div { class: "stack",
                        p { class: "form-error", "设备不存在或已注销。" }
                        p { class: "hint", "如果设备台账刚刚变更，请稍候片刻等待数据同步，或联系管理员确认这台设备的状态。" }
                    } } }
                }
            }
        }
    }
}

#[component]
pub fn MaintenanceInspectFirefightingPage(id: u64) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut done = use_signal(|| false);
    let can_inspect = can_submit_inspection(&state);
    let asset = state
        .firefighting_assets
        .read()
        .iter()
        .find(|row| row.asset_id == id)
        .cloned();
    let parks = state.parks.read().clone();
    let factories = state.factories.read().clone();
    let factory_floors = state.factory_floors.read().clone();
    let dormitories = state.dormitories.read().clone();
    let dormitory_floors = state.dormitory_floors.read().clone();
    let inspections = state.firefighting_inspections.read().clone();
    let today = today_date();

    rsx! {
        main { class: "page inspect-page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "消防设施巡检" }
                    p { class: "page-subtitle", "确认设施信息后填写本次巡检结果。" }
                }
            }
            if let Some(asset) = asset {
                section { class: "section",
                    Card { CardContent { div { class: "stack-tight",
                        div { class: "row",
                            Badge { variant: BadgeVariant::Outline, "{asset.facility_type}" }
                            strong { "{asset.facility_name}" }
                            small { class: "hint is-mono", "FIRE-{asset.asset_id:06}" }
                        }
                        small { class: "hint",
                            {location_label(
                                &asset,
                                &parks,
                                &factories,
                                &factory_floors,
                                &dormitories,
                                &dormitory_floors,
                            )}
                        }
                        small { class: "hint", "{asset.location}" }
                        if let Some(expiry) = &asset.expiry_on {
                            if expiry_days_left(expiry, &today).is_some_and(|days| days < 0) {
                                p { class: "form-error", "该设施已于 {expiry} 过期，请及时更换。" }
                            } else {
                                small { class: "hint", "有效期至 {expiry}" }
                            }
                        }
                        if let Some(latest) = latest_firefighting_inspection(&inspections, asset.asset_id) {
                            div { class: "row",
                                span { class: "hint", "上次巡检" }
                                Badge { variant: status_variant(&latest.status), "{latest.status}" }
                                small { class: "hint is-mono", "{format_datetime(latest.check_time)}" }
                                small { class: "hint", "{latest.inspector_name}" }
                            }
                        } else {
                            small { class: "hint", "这个设施还没有巡检记录，本次是第一条。" }
                        }
                    } } }
                }
                if done() {
                    section { class: "section",
                        Card { CardContent { div { class: "stack",
                            Badge { variant: BadgeVariant::Secondary, "巡检已提交" }
                            p { "记录已写入台账，可以继续扫下一个设施。" }
                            Button {
                                variant: ButtonVariant::Outline,
                                r#type: "button",
                                onclick: move |_| done.set(false),
                                "再登记一次"
                            }
                        } } }
                    }
                } else if can_inspect {
                    section { class: "section",
                        Card { CardContent {
                            FirefightingInspectionForm {
                                asset: asset.clone(),
                                on_saved: move |_| done.set(true),
                            }
                        } }
                    }
                } else {
                    section { class: "section",
                        Card { CardContent { div { class: "stack",
                            p { class: "form-error", "当前账号没有巡检权限。" }
                            p { class: "hint", "请联系管理员在权限管理中授予「设施巡检」权限码后重试。" }
                        } } }
                    }
                }
            } else {
                section { class: "section",
                    Card { CardContent { div { class: "stack",
                        p { class: "form-error", "设施不存在或已注销。" }
                        p { class: "hint", "如果设施台账刚刚变更，请稍候片刻等待数据同步，或联系管理员确认这个设施的状态。" }
                    } } }
                }
            }
        }
    }
}
