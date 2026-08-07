//! 总览页面的可复用展示组件。

use dioxus::prelude::*;

use crate::{
    components::{
        badge::Badge,
        card::{Card, CardContent, CardHeader},
        Icon,
    },
    spacetime_bindings::dashboard_revenue_pulse_type::DashboardRevenuePulse,
};

#[component]
pub fn MetricCard(label: String, value: String, detail: String, icon: String) -> Element {
    rsx! {
        div { Card {
                CardContent {
                    div { class: "stat",
                        span { class: "stat-label",
                            Icon { name: icon }
                            "{label}"
                        }
                        strong { "{value}" }
                        small { "{detail}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn RevenuePulseChart(points: Vec<DashboardRevenuePulse>) -> Element {
    let maximum = points
        .iter()
        .map(|point| point.receivable_cents.max(point.received_cents))
        .max()
        .unwrap_or(0)
        .max(1) as f64;

    if points.is_empty() {
        return rsx! {
            p { class: "empty", "等待首笔经营数据 · 账单写入后经营脉搏会自动出现" }
        };
    }

    rsx! {
        div { class: "dash-chart",
            for point in points {
                {
                    let receivable_height = ((point.receivable_cents.max(0) as f64 / maximum) * 100.0).max(4.0);
                    let received_height = ((point.received_cents.max(0) as f64 / maximum) * 100.0).max(3.0);
                    rsx! {
                        div { class: "dash-chart-column",
                            div { class: "dash-chart-bars",
                                span {
                                    class: "dash-chart-bar is-receivable",
                                    style: "--bar-height: {receivable_height}%",
                                }
                                span {
                                    class: "dash-chart-bar is-received",
                                    style: "--bar-height: {received_height}%",
                                }
                            }
                            small { "{point.period}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn TodoItem(
    title: String,
    description: String,
    count: u64,
    path: String,
    icon: String,
) -> Element {
    // 计数为零的事项不需要强调，弱化后让真正待办的条目自然浮出来。
    let pending = count > 0;
    rsx! {
        Link { class: if pending { "list-item" } else { "list-item is-clear" }, to: path,
            span { class: "list-icon", Icon { name: icon } }
            span { class: "list-item-copy",
                strong { "{title}" }
                small { "{description}" }
            }
            if pending {
                Badge { "{count}" }
            } else {
                span { class: "hint", "—" }
            }
        }
    }
}

#[component]
pub fn AssetStat(label: String, value: String, caption: String) -> Element {
    rsx! {
        div { class: "stat",
            span { "{label}" }
            strong { "{value}" }
            small { "{caption}" }
        }
    }
}
