//! 园区经营详情。

use dioxus::prelude::*;

use super::model::{format_area, park_is_enabled, ParkSnapshot};
use crate::components::{
    badge::{Badge, BadgeVariant},
    button::{Button, ButtonSize, ButtonVariant},
    dialog::{Dialog, DialogDescription, DialogTitle},
    progress::Progress,
};

#[component]
pub(super) fn ParkDetailDialog(snapshot: ParkSnapshot, on_close: EventHandler<()>) -> Element {
    let state = use_context::<crate::state::WorkspaceState>();
    // 合同租在哪里由楼层关联算出，不再是自由文本字段。
    let locations = crate::pages::contract_locations(
        &(state.factories)(),
        &(state.factory_floors)(),
        &(state.rental_tenant_floors)(),
        &(state.dormitories)(),
        &(state.dormitory_floors)(),
        &(state.rental_tenant_dormitory_floors)(),
    );
    let park = &snapshot.park;
    let occupied = format_area(snapshot.rented_area);
    let vacant = format_area(snapshot.vacant_area);
    let total = format_area(snapshot.total_area);
    let rate = snapshot.occupancy_percent();
    let enabled = park_is_enabled(park.status.as_deref());
    let manager = park.manager.as_deref().unwrap_or("--");
    let contact = park.contact.as_deref().unwrap_or("--");
    let description = park.description.as_deref().unwrap_or("暂无园区说明");
    let mut description_open = use_signal(|| false);

    rsx! {
        Dialog {
            // 弹窗由父级按需挂载，这里恒为打开；关闭统一走 on_open_change，
            // Esc 和点击遮罩因此都能关闭，不必自己接键盘事件。
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open {
                    on_close.call(());
                }
            },
            DialogTitle { "{park.park_name}" }
            DialogDescription { "{park.address}" }

            div { class: "stack",
                div { class: "row",
                    Badge {
                        variant: if enabled { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                        if enabled {
                            "运营中"
                        } else {
                            "已停用"
                        }
                    }
                    span { class: "hint", "出租率 {rate:.1}%" }
                }
                Progress { value: rate, max: 100.0 }

                // 三个面积是这个弹窗的主信息，做成磁贴：大号数字 + 浅底，
                // 和下面的联系人字段拉开层级。原来它们和普通键值对一个样子，
                // 一眼扫过去分不出哪些是关键指标。
                div { class: "grid-3",
                    div { class: "panel is-tight",
                        span { class: "stat-label", "园区总面积" }
                        strong { class: "stat-value is-compact", "{total} ㎡" }
                    }
                    div { class: "panel is-tight",
                        span { class: "stat-label", "当前已出租" }
                        strong { class: "stat-value is-compact", "{occupied} ㎡" }
                    }
                    div { class: "panel is-tight",
                        span { class: "stat-label", "当前可招商" }
                        strong { class: "stat-value is-compact", "{vacant} ㎡" }
                    }
                }

                // 有效合同份数不再单列：下面「当前入驻合同」的徽章已经写了同一个数。
                dl { class: "facts",
                    div {
                        dt { "园区负责人" }
                        dd { "{manager}" }
                    }
                    div {
                        dt { "联系方式" }
                        dd { class: "is-mono", "{contact}" }
                    }
                }

                if !description.trim().is_empty() {
                    div { class: "stack-tight",
                        span { class: "field-label", "园区说明" }
                        // 默认只显示三行。说明动辄百来字，全文平铺会把入驻合同
                        // 整个推到弹窗可视区之外，而合同才是这里更常查的东西。
                        p {
                            class: if description_open() { "" } else { "line-clamp-3" },
                            "{description}"
                        }
                        div { class: "row",
                            Button {
                                variant: ButtonVariant::Link,
                                size: ButtonSize::Sm,
                                r#type: "button",
                                onclick: move |_| description_open.toggle(),
                                if description_open() {
                                    "收起说明"
                                } else {
                                    "展开说明"
                                }
                            }
                        }
                    }
                }

                div { class: "subsection",
                    div { class: "section-header",
                        h3 { "当前入驻合同" }
                        Badge {
                            variant: BadgeVariant::Secondary,
                            "{snapshot.active_contracts.len()} 份"
                        }
                    }
                    if snapshot.active_contracts.is_empty() {
                        p { class: "empty", "该园区暂无有效收入合同，可出租面积为园区总面积。" }
                    } else {
                        div { class: "list",
                            for contract in snapshot.active_contracts.iter() {
                                div { key: "tenant-{contract.rental_tenant_id}", class: "list-item",
                                    div { class: "list-item-copy",
                                        strong { "{contract.tenant_name}" }
                                        small {
                                            {locations.get(&contract.rental_tenant_id).map(String::as_str).unwrap_or("未选定楼层")}
                                        }
                                    }
                                    span { class: "hint",
                                        "{format_area(contract.area_centi_square_metres.unwrap_or_default())} ㎡"
                                    }
                                }
                            }
                        }
                    }
                }

                div { class: "form-actions",
                    Button { onclick: move |_| on_close.call(()), "完成" }
                }
            }
        }
    }
}
