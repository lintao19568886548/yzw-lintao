//! 访客管理实时台账。

use dioxus::prelude::*;

use super::{
    delete_dialog::{AccessDeleteDialog, AccessDeleteTarget},
    model::{format_datetime, park_name, status_label, status_variant},
    navigation::AccessNavigation,
    visitor_form::VisitorFormDialog,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        total_pages, Pager,
    },
    permissions::can_manage_access,
    services::park_ref,
    spacetime_bindings::access_visitor_type::AccessVisitor,
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

#[component]
pub fn AccessVisitorPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let rows = (state.access_visitors)();
    let parks = (state.parks)();
    let can_manage = can_manage_access(&(state.roles)());
    let mut keyword = use_signal(String::new);
    let mut park_filter = use_signal(String::new);
    let mut status_filter = use_signal(|| "all".to_string());
    let mut selected = use_signal(|| None::<AccessVisitor>);
    let mut readonly = use_signal(|| false);
    let mut form_open = use_signal(|| false);
    let mut deleting = use_signal(|| None::<AccessDeleteTarget>);
    let mut notice = use_signal(|| None::<String>);
    let mut page = use_signal(|| 1usize);
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_filter())).into();
    let status_value: ReadSignal<Option<String>> = use_memo(move || Some(status_filter())).into();

    let query = keyword().trim().to_lowercase();
    let filtered = rows
        .iter()
        .filter(|row| {
            (query.is_empty()
                || row.visitor_name.to_lowercase().contains(&query)
                || row.phone_number.contains(&query)
                || row
                    .car_num
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&query)
                || row
                    .remark
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&query))
                && (park_filter().is_empty()
                    || park_ref(row.park_id).map(|id| id.to_string()) == Some(park_filter()))
                && (status_filter() == "all" || row.status.to_string() == status_filter())
        })
        .cloned()
        .collect::<Vec<_>>();
    let inside = rows.iter().filter(|row| row.status == 0).count();
    let left = rows.len().saturating_sub(inside);
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
            AccessNavigation { active: String::from("visitor") }

            header { class: "page-header",
                div { class: "page-title",
                    h1 { "访客管理" }
                    p { class: "page-subtitle", "集中查看到访身份、联系方式、来访目的与当前通行状态。" }
                }
                if can_manage {
                    div { class: "page-actions",
                        Link { to: crate::router::Route::AccessVisitorRegisterPage {},
                            Button { variant: ButtonVariant::Outline, "快速登记" }
                        }
                        Button {
                            onclick: move |_| {
                                selected.set(None);
                                readonly.set(false);
                                form_open.set(true);
                            },
                            "新增访客记录"
                        }
                    }
                }
            }

            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-3",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "访客通行记录" }
                                strong { class: "stat-value is-mono", "{rows.len()}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "在场记录" }
                                strong { class: "stat-value is-mono is-ok", "{inside}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "离场记录" }
                                strong { class: "stat-value is-mono", "{left}" }
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
                                Label { html_for: "visitor-keyword", "搜索访客" }
                                Input {
                                    id: "visitor-keyword",
                                    value: keyword,
                                    placeholder: "姓名、手机号、车牌或原因",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "visitor-park", "所属园区" }
                                Select {
                                    id: "visitor-park",
                                    value: Some(park_value),
                                    on_value_change: move |value: Option<String>| {
                                        park_filter.set(value.unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "全部园区".to_string(), "全部园区" }
                                    for (index , park) in parks.iter().enumerate() {
                                        SelectOption::<String> {
                                            key: "visitor-park-{park.park_id}",
                                            value: park.park_id.to_string(),
                                            index: index + 1,
                                            text_value: park.park_name.to_string(),
                                            "{park.park_name}"
                                        }
                                    }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "visitor-status", "访问状态" }
                                Select {
                                    id: "visitor-status",
                                    value: Some(status_value),
                                    on_value_change: move |value: Option<String>| {
                                        status_filter.set(value.unwrap_or_else(|| "all".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    SelectOption::<String> { value: "0".to_string(), index: 1usize, text_value: "进入".to_string(), "进入" }
                                    SelectOption::<String> { value: "1".to_string(), index: 2usize, text_value: "离开".to_string(), "离开" }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "来访信息列表" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 条" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "访客" }
                                th { "联系方式" }
                                th { "车牌" }
                                th { "园区" }
                                th { "状态" }
                                th { "登记时间" }
                                th { "来访原因" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if visible.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "8",
                                        if rows.is_empty() {
                                            "暂无访客记录。完成访客登记后会实时显示在这里。"
                                        } else {
                                            "没有符合条件的访客记录，请修改筛选条件。"
                                        }
                                    }
                                }
                            }
                            for row in visible {
                                {
                                    let view = row.clone();
                                    let open_row = row.clone();
                                    let edit = row.clone();
                                    let remove_name = row.visitor_name.clone();
                                    let visitor_id = row.visitor_id;
                                    rsx! {
                                        tr {
                                            key: "visitor-{visitor_id}",
                                            class: "is-clickable",
                                            onclick: move |_| {
                                                selected.set(Some(open_row.clone()));
                                                readonly.set(true);
                                                form_open.set(true);
                                            },
                                            td {
                                                strong { "{row.visitor_name}" }
                                            }
                                            td { class: "is-mono", "{row.phone_number}" }
                                            td { class: "is-mono",
                                                {row.car_num.as_deref().unwrap_or("无车辆")}
                                            }
                                            td { "{park_name(&parks, row.park_id)}" }
                                            td {
                                                Badge { variant: status_variant(row.status, false), "{status_label(row.status, false)}" }
                                            }
                                            td { class: "is-mono", "{format_datetime(row.register_time)}" }
                                            td { class: "is-wrap hint",
                                                {row.remark.as_deref().unwrap_or("暂无来访原因")}
                                            }
                                            td {
                                                div {
                                                    class: "table-actions",
                                                    onclick: move |event| event.stop_propagation(),
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        onclick: move |_| {
                                                            selected.set(Some(view.clone()));
                                                            readonly.set(true);
                                                            form_open.set(true);
                                                        },
                                                        "查看"
                                                    }
                                                    if can_manage {
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            size: ButtonSize::Sm,
                                                            onclick: move |_| {
                                                                selected.set(Some(edit.clone()));
                                                                readonly.set(false);
                                                                form_open.set(true);
                                                            },
                                                            "编辑"
                                                        }
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            class: "is-quiet-danger",
                                                            size: ButtonSize::Sm,
                                                            onclick: move |_| {
                                                                deleting
                                                                    .set(
                                                                        Some(AccessDeleteTarget::Visitor {
                                                                            id: visitor_id,
                                                                            name: remove_name.clone(),
                                                                        }),
                                                                    )
                                                            },
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
                Pager { page, total_pages: page_count, total_count: total }
            }
        }

        if form_open() {
            VisitorFormDialog {
                record: selected(),
                parks: parks.clone(),
                readonly: readonly(),
                on_close: move |_| form_open.set(false),
                on_saved: move |_| {
                    form_open.set(false);
                    notice.set(Some("访客记录已保存。".into()));
                },
            }
        }
        if let Some(target) = deleting() {
            AccessDeleteDialog {
                target,
                on_close: move |_| deleting.set(None),
                on_deleted: move |_| {
                    deleting.set(None);
                    notice.set(Some("访客记录已删除。".into()));
                },
            }
        }
    }
}
