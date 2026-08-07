//! 车辆出入管理实时台账。

use dioxus::prelude::*;

use super::{
    car_form::CarFormDialog,
    delete_dialog::{AccessDeleteDialog, AccessDeleteTarget},
    model::{format_datetime, park_name, status_label, status_variant},
    navigation::AccessNavigation,
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
    spacetime_bindings::access_car_type::AccessCar,
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

#[component]
pub fn AccessCarPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let rows = (state.access_cars)();
    let parks = (state.parks)();
    let can_manage = can_manage_access(&(state.roles)());
    let mut keyword = use_signal(String::new);
    let mut park_filter = use_signal(String::new);
    let mut status_filter = use_signal(|| "all".to_string());
    let mut selected = use_signal(|| None::<AccessCar>);
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
                || row.car_number.to_lowercase().contains(&query)
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
    let entering = rows.iter().filter(|row| row.status == 1).count();
    let leaving = rows.len().saturating_sub(entering);
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
            AccessNavigation { active: String::from("car") }

            header { class: "page-header",
                div { class: "page-title",
                    h1 { "车辆出入管理" }
                    p { class: "page-subtitle", "实时记录车辆进出园区，保留门岗登记时间和通行说明。" }
                }
                if can_manage {
                    div { class: "page-actions",
                        Button {
                            onclick: move |_| {
                                selected.set(None);
                                readonly.set(false);
                                form_open.set(true);
                            },
                            "新增车辆记录"
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
                                span { class: "stat-label", "车辆通行记录" }
                                strong { class: "stat-value is-mono", "{rows.len()}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "进入" }
                                strong { class: "stat-value is-mono is-ok", "{entering}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "离开" }
                                strong { class: "stat-value is-mono", "{leaving}" }
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
                                Label { html_for: "car-keyword", "搜索车辆" }
                                Input {
                                    id: "car-keyword",
                                    value: keyword,
                                    placeholder: "车牌号或备注",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "car-park", "所属园区" }
                                Select {
                                    id: "car-park",
                                    value: Some(park_value),
                                    on_value_change: move |value: Option<String>| {
                                        park_filter.set(value.unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "全部园区".to_string(), "全部园区" }
                                    for (index , park) in parks.iter().enumerate() {
                                        SelectOption::<String> {
                                            key: "car-park-{park.park_id}",
                                            value: park.park_id.to_string(),
                                            index: index + 1,
                                            text_value: park.park_name.to_string(),
                                            "{park.park_name}"
                                        }
                                    }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "car-status", "出入状态" }
                                Select {
                                    id: "car-status",
                                    value: Some(status_value),
                                    on_value_change: move |value: Option<String>| {
                                        status_filter.set(value.unwrap_or_else(|| "all".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    SelectOption::<String> { value: "1".to_string(), index: 1usize, text_value: "进入".to_string(), "进入" }
                                    SelectOption::<String> { value: "0".to_string(), index: 2usize, text_value: "离开".to_string(), "离开" }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "车辆通行记录" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 条" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "车牌号码" }
                                th { "园区" }
                                th { "状态" }
                                th { "登记时间" }
                                th { "备注" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if visible.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "6",
                                        if rows.is_empty() {
                                            "暂无车辆记录。新增第一条车辆出入登记后会实时显示在这里。"
                                        } else {
                                            "没有符合条件的车辆记录，请修改筛选条件。"
                                        }
                                    }
                                }
                            }
                            for row in visible {
                                {
                                    let view = row.clone();
                                    let open_row = row.clone();
                                    let edit = row.clone();
                                    let remove_name = row.car_number.clone();
                                    let car_id = row.car_id;
                                    rsx! {
                                        tr {
                                            key: "car-{car_id}",
                                            class: "is-clickable",
                                            onclick: move |_| {
                                                selected.set(Some(open_row.clone()));
                                                readonly.set(true);
                                                form_open.set(true);
                                            },
                                            td { class: "is-mono",
                                                strong { "{row.car_number}" }
                                            }
                                            td { "{park_name(&parks, row.park_id)}" }
                                            td {
                                                Badge { variant: status_variant(row.status, true), "{status_label(row.status, true)}" }
                                            }
                                            td { class: "is-mono", "{format_datetime(row.register_time)}" }
                                            td { class: "is-wrap hint",
                                                {row.remark.as_deref().unwrap_or("暂无备注")}
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
                                                                        Some(AccessDeleteTarget::Car {
                                                                            id: car_id,
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
            CarFormDialog {
                record: selected(),
                parks: parks.clone(),
                readonly: readonly(),
                on_close: move |_| form_open.set(false),
                on_saved: move |_| {
                    form_open.set(false);
                    notice.set(Some("车辆出入记录已保存。".into()));
                },
            }
        }
        if let Some(target) = deleting() {
            AccessDeleteDialog {
                target,
                on_close: move |_| deleting.set(None),
                on_deleted: move |_| {
                    deleting.set(None);
                    notice.set(Some("车辆出入记录已删除。".into()));
                },
            }
        }
    }
}
