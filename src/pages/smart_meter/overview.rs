//! 合众电表与 YMSINO 水表实时工作台。

use std::collections::BTreeSet;

use dioxus::prelude::*;

use super::{
    binding::MeterBindingSection,
    date::{display_date, shift_date, today},
    device_tree::MeterDeviceTree,
    reading_table::MeterReadingList,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        progress::Progress,
        total_pages, Pager,
    },
    router::Route,
    services::{
        load_saved_token, load_smart_meter_catalog, load_smart_meter_readings, MeterKind,
        SmartMeterCatalog, SmartMeterReading,
    },
};

const PAGE_SIZE: usize = 20;

#[component]
pub fn SmartElectricMeterPage() -> Element {
    rsx! { SmartMeterPage { kind: MeterKind::Electric } }
}

#[component]
pub fn SmartWaterMeterPage() -> Element {
    rsx! { SmartMeterPage { kind: MeterKind::Water } }
}

#[component]
fn SmartMeterPage(kind: MeterKind) -> Element {
    let mut selected_date = use_signal(today);
    let mut catalog = use_signal(|| None::<SmartMeterCatalog>);
    let mut readings = use_signal(Vec::<SmartMeterReading>::new);
    let mut catalog_loading = use_signal(|| true);
    let mut reading_loading = use_signal(|| false);
    let mut catalog_error = use_signal(|| None::<String>);
    let mut reading_error = use_signal(|| None::<String>);
    let mut catalog_refresh_version = use_signal(|| 0u64);
    let mut reading_refresh_version = use_signal(|| 0u64);
    let mut tree_search = use_signal(String::new);
    let mut selected_tree = use_signal(String::new);
    let mut reading_search = use_signal(String::new);
    let mut page = use_signal(|| 1usize);

    use_effect(move || {
        let request_version = catalog_refresh_version();
        catalog_loading.set(true);
        catalog_error.set(None);
        let Some(token) = load_saved_token() else {
            catalog_loading.set(false);
            catalog_error.set(Some("登录凭证不存在，请重新登录".into()));
            return;
        };
        spawn(async move {
            let result = load_smart_meter_catalog(token, kind).await;
            if catalog_refresh_version() != request_version {
                return;
            }
            catalog_loading.set(false);
            match result {
                Ok(value) => catalog.set(Some(value)),
                Err(request_error) => catalog_error.set(Some(format!("{request_error}"))),
            }
        });
    });

    use_effect(move || {
        let request_date = selected_date();
        let request_version = reading_refresh_version();
        // 与原页面一致：设备目录成功后才读取表格，目录先展示，不阻塞左侧区域。
        if catalog().is_none() {
            return;
        }
        reading_loading.set(true);
        reading_error.set(None);
        let Some(token) = load_saved_token() else {
            reading_loading.set(false);
            reading_error.set(Some("登录凭证不存在，请重新登录".into()));
            return;
        };
        spawn(async move {
            let result = load_smart_meter_readings(token, kind, request_date.clone()).await;
            if selected_date() != request_date || reading_refresh_version() != request_version {
                return;
            }
            reading_loading.set(false);
            match result {
                Ok(value) => readings.set(value.readings),
                Err(request_error) => reading_error.set(Some(format!("{request_error}"))),
            }
        });
    });

    let catalog_value = catalog();
    let devices = catalog_value
        .as_ref()
        .map(|value| value.devices.clone())
        .unwrap_or_default();
    let reading_rows = readings();
    let reading_keyword = reading_search().trim().to_lowercase();
    let selected_value = selected_tree();
    let selected_addresses = devices
        .iter()
        .filter(|device| matches_device_selection(device, &selected_value))
        .map(|device| device.factory_no.clone())
        .collect::<BTreeSet<_>>();
    let filtered_rows = reading_rows
        .iter()
        .filter(|reading| {
            selected_value.is_empty() || selected_addresses.contains(&reading.com_address)
        })
        .filter(|reading| {
            reading_keyword.is_empty()
                || reading.room_name.to_lowercase().contains(&reading_keyword)
                || reading
                    .com_address
                    .to_lowercase()
                    .contains(&reading_keyword)
                || reading.device_id.to_lowercase().contains(&reading_keyword)
        })
        .cloned()
        .collect::<Vec<_>>();
    let page_count = total_pages(filtered_rows.len(), PAGE_SIZE);
    // 筛选或换日期会让页数变少，越界时回落到最后一页而不是在渲染中回写信号。
    let page_start = (page().clamp(1, page_count) - 1) * PAGE_SIZE;
    let visible_rows = filtered_rows
        .iter()
        .skip(page_start)
        .take(PAGE_SIZE)
        .cloned()
        .collect::<Vec<_>>();
    let reporting_devices = reading_rows
        .iter()
        .map(|reading| reading.com_address.as_str())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .len();
    let coverage = if devices.is_empty() {
        0
    } else {
        (reporting_devices * 100 / devices.len()).min(100)
    };
    let total_value = reading_rows
        .iter()
        .filter_map(|reading| reading.data_value.parse::<f64>().ok())
        .sum::<f64>();
    let protocol = catalog_value
        .as_ref()
        .map(|value| value.protocol.as_str())
        .unwrap_or("--");
    let park_name = catalog_value
        .as_ref()
        .map(|value| value.park_name.as_str())
        .unwrap_or(if kind == MeterKind::Electric {
            "项目 241"
        } else {
            "YZWL"
        });
    let provider_name = catalog_value
        .as_ref()
        .map(|value| value.provider_name.as_str())
        .unwrap_or(if kind == MeterKind::Electric {
            "合众"
        } else {
            "YMSINO"
        });

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "智能水电表管理" }
                    p { class: "page-subtitle",
                        "按照原系统园区、楼栋与楼层关系读取实时日冻结数据，支持电表尖峰平谷与水表累计流量查询。"
                    }
                }
                div { class: "page-actions",
                    span { class: "live-pulse" }
                    span { class: "hint", "实时数据源 {provider_name} · {park_name}" }
                }
            }

            nav { class: "row", aria_label: "水电表类型",
                Link { to: Route::SmartElectricMeterPage {},
                    Button {
                        variant: if kind == MeterKind::Electric { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        "电表"
                    }
                }
                Link { to: Route::SmartWaterMeterPage {},
                    Button {
                        variant: if kind == MeterKind::Water { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        "水表"
                    }
                }
            }

            section { class: "grid-4",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "设备档案" }
                                strong { class: "stat-value", "{devices.len()}" }
                                span { class: "stat-caption", "{kind.label()}设备" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "当日回传" }
                                strong { class: "stat-value", "{reading_rows.len()}" }
                                span { class: "stat-caption", "日冻结记录" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "回传覆盖" }
                                strong { class: "stat-value", "{coverage}%" }
                                Progress { value: coverage as f64, max: 100.0 }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "累计读数" }
                                strong { class: "stat-value", "{total_value:.2}" }
                                span { class: "stat-caption", "单位 · {kind.unit()}" }
                            }
                        }
                    }
                }
            }

            section { class: "grid-aside",
                aside {
                    Card {
                        CardContent {
                            div { class: "stack",
                                div { class: "section-header",
                                    h3 { "建筑列表" }
                                    Badge { variant: BadgeVariant::Secondary, "{devices.len()} 台" }
                                }
                                Input {
                                    value: tree_search,
                                    placeholder: "搜索建筑或设备编号",
                                    oninput: move |event: FormEvent| tree_search.set(event.value()),
                                }
                                if let Some(message) = catalog_error() {
                                    div { class: "stack", role: "alert",
                                        p { class: "form-error", "{message}" }
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            onclick: move |_| *catalog_refresh_version.write() += 1,
                                            "重新读取建筑树"
                                        }
                                    }
                                } else if catalog_loading() && catalog_value.is_none() {
                                    p { class: "empty", "正在获取建筑与设备档案…" }
                                } else {
                                    MeterDeviceTree {
                                        devices: devices.clone(),
                                        search: tree_search(),
                                        selected: selected_tree(),
                                        on_select: move |value| {
                                            selected_tree.set(value);
                                            page.set(1);
                                        },
                                    }
                                }
                            }
                        }
                    }
                }

                section { class: "section",
                    Card {
                        CardContent {
                            div { class: "stack",
                                div { class: "section-header",
                                    h3 { "{kind.label()}日冻结数据" }
                                    Badge { variant: BadgeVariant::Outline, "{protocol}" }
                                }

                                div { class: "filters",
                                    div { class: "field",
                                        // 这一格是三个按钮组成的日期步进器，没有单一控件可挂 for
                                        span { class: "field-label", "冻结日期" }
                                        div { class: "row",
                                            Button {
                                                variant: ButtonVariant::Outline,
                                                aria_label: "前一天",
                                                onclick: move |_| {
                                                    selected_date.set(shift_date(&selected_date(), -1));
                                                    page.set(1);
                                                },
                                                "前一天"
                                            }
                                            Button {
                                                variant: ButtonVariant::Ghost,
                                                onclick: move |_| {
                                                    selected_date.set(today());
                                                    page.set(1);
                                                },
                                                "{display_date(&selected_date())}"
                                            }
                                            Button {
                                                variant: ButtonVariant::Outline,
                                                aria_label: "后一天",
                                                disabled: selected_date() >= today(),
                                                onclick: move |_| {
                                                    selected_date.set(shift_date(&selected_date(), 1));
                                                    page.set(1);
                                                },
                                                "后一天"
                                            }
                                        }
                                    }
                                    div { class: "field is-flexible",
                                        Label { html_for: "meter-reading-search", "设备检索" }
                                        Input {
                                            id: "meter-reading-search",
                                            value: reading_search,
                                            placeholder: "输入房间、设备编号",
                                            oninput: move |event: FormEvent| {
                                                reading_search.set(event.value());
                                                page.set(1);
                                            },
                                        }
                                    }
                                    div { class: "field",
                                        span { class: "field-label", "数据类型" }
                                        // 系统只提供日冻结一种数据，是只读取值而不是可选项，
                                        // 做成徽章会被当成可点的筛选标签。
                                        span { class: "field-static", "日冻结数据" }
                                    }
                                    div { class: "field is-action",
                                        Button {
                                            disabled: reading_loading() || catalog_value.is_none(),
                                            onclick: move |_| *reading_refresh_version.write() += 1,
                                            if reading_loading() {
                                                "同步中…"
                                            } else {
                                                "刷新数据"
                                            }
                                        }
                                    }
                                }

                                if let Some(message) = reading_error() {
                                    div { class: "stack", role: "alert",
                                        p { class: "form-error", "数据读取失败：{message}" }
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            onclick: move |_| *reading_refresh_version.write() += 1,
                                            "重新读取"
                                        }
                                    }
                                }

                                if reading_loading() && reading_rows.is_empty() {
                                    p { class: "empty",
                                        "正在获取 {display_date(&selected_date())} 日冻结记录，最长等待 20 秒…"
                                    }
                                } else {
                                    MeterReadingList { kind, rows: visible_rows }
                                    Pager {
                                        page,
                                        total_pages: page_count,
                                        total_count: filtered_rows.len(),
                                    }
                                }
                            }
                        }
                    }
                }
            }

            MeterBindingSection { kind, devices: devices.clone() }
        }
    }
}

fn matches_device_selection(device: &crate::services::SmartMeterDevice, selection: &str) -> bool {
    selection.is_empty()
        || selection
            .strip_prefix("park:")
            .is_some_and(|park| device.park_name == park)
        || selection
            .strip_prefix("building:")
            .is_some_and(|path| path == format!("{}/{}", device.park_name, device.building_name))
        || selection.strip_prefix("floor:").is_some_and(|path| {
            path == format!(
                "{}/{}/{}",
                device.park_name, device.building_name, device.floor_name
            )
        })
        || selection
            .strip_prefix("room:")
            .is_some_and(|room_id| device.room_id == room_id)
        || selection
            .strip_prefix("device:")
            .is_some_and(|address| device.factory_no == address)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> crate::services::SmartMeterDevice {
        crate::services::SmartMeterDevice {
            park_id: "241".into(),
            park_name: "十一高埗园区".into(),
            building_name: "A栋".into(),
            floor_name: "3楼".into(),
            room_id: "8303".into(),
            room_name: "8303".into(),
            device_id: "1831655".into(),
            factory_no: "250603033969".into(),
            protocol: "D.ZDG.FIWBM-GD04".into(),
            current_ratio: "1".into(),
            multiplier: 1.0,
        }
    }

    #[test]
    fn 建筑树选择支持各级路径和设备() {
        assert!(matches_device_selection(&device(), "park:十一高埗园区"));
        assert!(matches_device_selection(
            &device(),
            "building:十一高埗园区/A栋"
        ));
        assert!(matches_device_selection(
            &device(),
            "floor:十一高埗园区/A栋/3楼"
        ));
        assert!(matches_device_selection(&device(), "device:250603033969"));
        assert!(!matches_device_selection(&device(), "device:missing"));
    }
}
