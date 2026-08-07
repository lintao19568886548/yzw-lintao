//! 考勤定位轨迹列表。

use dioxus::prelude::*;

use super::{model::format_datetime, navigation::HrmNavigation};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        total_pages, Pager,
    },
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 30;

#[component]
pub fn HrmTrajectoryPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let rows = (state.localizations)();
    let attendances = (state.attendances)();
    let mut keyword = use_signal(String::new);
    let mut page = use_signal(|| 1usize);
    let query = keyword().to_lowercase();

    let mut points = rows
        .iter()
        .map(|row| {
            (
                row.user_name.clone(),
                row.punch_time,
                row.latitude_e_15,
                row.longitude_e_15,
                "定位上报",
            )
        })
        .collect::<Vec<_>>();
    points.extend(attendances.iter().map(|row| {
        (
            row.username.clone(),
            row.punch_in,
            row.latitude_e_15,
            row.longitude_e_15,
            "考勤打卡",
        )
    }));
    points.sort_by_key(|row| std::cmp::Reverse(row.1.to_micros_since_unix_epoch()));
    let filtered = points
        .iter()
        .filter(|row| query.is_empty() || row.0.to_lowercase().contains(&query))
        .collect::<Vec<_>>();
    let total = filtered.len();
    let page_count = total_pages(total, PAGE_SIZE);
    // 轨迹点动辄上千条，不分页会把整页拖垮。
    let visible = filtered
        .into_iter()
        .skip((page().clamp(1, page_count) - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .collect::<Vec<_>>();

    rsx! {
        main { class: "page",
            HrmNavigation { active: String::from("trajectory") }

            header { class: "page-header",
                div { class: "page-title",
                    h1 { "考勤轨迹" }
                    p { class: "page-subtitle", "汇总定位上报与考勤打卡坐标，用于复核人员出勤轨迹。" }
                }
                div { class: "page-actions",
                    Badge { variant: BadgeVariant::Secondary, "{total} 个轨迹点" }
                }
            }

            section { class: "section",
                Card {
                    CardContent {
                        div { class: "filters",
                            div { class: "field",
                                Label { html_for: "trajectory-keyword", "人员" }
                                Input {
                                    id: "trajectory-keyword",
                                    value: keyword,
                                    placeholder: "输入员工姓名",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "时间" }
                                th { "员工" }
                                th { "来源" }
                                th { "定位坐标" }
                            }
                        }
                        tbody {
                            if visible.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "4", "暂无定位轨迹" }
                                }
                            }
                            for (index , row) in visible.into_iter().enumerate() {
                                {
                                    let latitude = row.2 as f64 / 1e15;
                                    let longitude = row.3 as f64 / 1e15;
                                    let map_url = format!(
                                        "https://uri.amap.com/marker?position={longitude:.6},{latitude:.6}",
                                    );
                                    rsx! {
                                        tr { key: "trace-{index}-{row.1.to_micros_since_unix_epoch()}",
                                            td { class: "is-mono", "{format_datetime(Some(row.1))}" }
                                            td {
                                                strong { "{row.0}" }
                                            }
                                            td {
                                                Badge {
                                                    variant: if row.4 == "考勤打卡" { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                                    "{row.4}"
                                                }
                                            }
                                            td {
                                                a {
                                                    class: "is-mono hint",
                                                    href: "{map_url}",
                                                    target: "_blank",
                                                    rel: "noreferrer",
                                                    "{latitude:.6}, {longitude:.6} ↗"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Pager { page, total_pages: page_count, total_count: total }
            }
        }
    }
}
