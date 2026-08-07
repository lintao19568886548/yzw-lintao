//! 考勤记录与异常状态汇总。

use dioxus::prelude::*;

use super::{
    model::{attendance_status, format_datetime, punch_source_label},
    navigation::HrmNavigation,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        total_pages, Pager,
    },
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

#[component]
pub fn HrmAttendanceRecordsPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let rows = (state.attendances)();
    let mut keyword = use_signal(String::new);
    let mut status = use_signal(|| "all".to_string());
    let mut page = use_signal(|| 1usize);
    let status_value: ReadSignal<Option<String>> = use_memo(move || Some(status())).into();

    let query = keyword().to_lowercase();
    let filtered = rows
        .iter()
        .filter(|row| {
            (query.is_empty() || row.username.to_lowercase().contains(&query))
                && (status() == "all"
                    || row.status.map(|value| value.to_string()).as_deref()
                        == Some(status().as_str()))
        })
        .collect::<Vec<_>>();
    let normal = rows.iter().filter(|row| row.status == Some(0)).count();
    let abnormal = rows
        .iter()
        .filter(|row| matches!(row.status, Some(1..=4)))
        .count();
    let completed = rows.iter().filter(|row| row.punch_out.is_some()).count();
    let total = filtered.len();
    let page_count = total_pages(total, PAGE_SIZE);
    // 筛选收窄会让页数变少，越界时回落到最后一页，否则列表显示空白。
    let visible = filtered
        .into_iter()
        .skip((page().clamp(1, page_count) - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .collect::<Vec<_>>();

    rsx! {
        main { class: "page",
            HrmNavigation { active: String::from("records") }

            header { class: "page-header",
                div { class: "page-title",
                    h1 { "考勤记录" }
                    p { class: "page-subtitle", "按人员与异常状态核对上下班时间，保留原始定位坐标。" }
                }
            }

            section { class: "grid-4",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "记录总数" }
                                strong { class: "stat-value is-mono", "{rows.len()}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "正常" }
                                strong { class: "stat-value is-mono is-ok", "{normal}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "异常" }
                                strong { class: "stat-value is-mono", "{abnormal}" }
                                span { class: "stat-caption", "迟到 / 早退 / 缺勤" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已完成下班卡" }
                                strong { class: "stat-value is-mono", "{completed}" }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                Card {
                    CardContent {
                        div { class: "filters",
                            div { class: "field",
                                Label { html_for: "attendance-keyword", "人员" }
                                Input {
                                    id: "attendance-keyword",
                                    value: keyword,
                                    placeholder: "搜索姓名",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "attendance-status", "状态" }
                                Select {
                                    id: "attendance-status",
                                    value: Some(status_value),
                                    on_value_change: move |value: Option<String>| {
                                        status.set(value.unwrap_or_else(|| "all".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    SelectOption::<String> { value: "0".to_string(), index: 1usize, text_value: "正常".to_string(), "正常" }
                                    SelectOption::<String> { value: "1".to_string(), index: 2usize, text_value: "迟到".to_string(), "迟到" }
                                    SelectOption::<String> { value: "2".to_string(), index: 3usize, text_value: "早退".to_string(), "早退" }
                                    SelectOption::<String> { value: "3".to_string(), index: 4usize, text_value: "迟到且早退".to_string(), "迟到且早退" }
                                    SelectOption::<String> { value: "4".to_string(), index: 5usize, text_value: "缺勤".to_string(), "缺勤" }
                                    SelectOption::<String> { value: "5".to_string(), index: 6usize, text_value: "请假".to_string(), "请假" }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "出勤明细" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 条" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "员工" }
                                th { "状态" }
                                th { "上班打卡" }
                                th { "下班打卡" }
                                th { "定位坐标" }
                            }
                        }
                        tbody {
                            if visible.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "5", "暂无考勤记录" }
                                }
                            }
                            for row in visible {
                                {
                                    let (label, variant) = attendance_status(row.status);
                                    let latitude = row.latitude_e_15 as f64 / 1e15;
                                    let longitude = row.longitude_e_15 as f64 / 1e15;
                                    let map_url = format!(
                                        "https://uri.amap.com/marker?position={longitude:.6},{latitude:.6}",
                                    );
                                    rsx! {
                                        tr { key: "attendance-{row.attendance_id}",
                                            td {
                                                strong { "{row.username}" }
                                            }
                                            td {
                                                Badge { variant, "{label}" }
                                            }
                                            td {
                                                div { class: "stack-tight",
                                                    span { class: "is-mono", "{format_datetime(Some(row.punch_in))}" }
                                                    small { class: "hint",
                                                        "{punch_source_label(row.punch_in_source.as_deref())}"
                                                    }
                                                }
                                            }
                                            td {
                                                div { class: "stack-tight",
                                                    span { class: "is-mono", "{format_datetime(row.punch_out)}" }
                                                    if row.punch_out.is_some() {
                                                        small { class: "hint",
                                                            "{punch_source_label(row.punch_out_source.as_deref())}"
                                                        }
                                                    }
                                                }
                                            }
                                            td {
                                                // 坐标本身没人读，直接给地图链接
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
