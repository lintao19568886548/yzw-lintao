//! 园区管理与租赁经营总览。

use dioxus::prelude::*;

use super::{
    detail::ParkDetailDialog,
    form::ParkProfileDialog,
    model::{format_area, park_is_enabled, ParkSnapshot, RentalSummary},
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        progress::Progress,
        select::{Select, SelectOption},
        ConfirmDialog,
    },
    permissions::can_manage_rental,
    router::Route,
    services::delete_park_record,
    state::WorkspaceState,
};

#[component]
pub fn RentalManagementPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let parks = (state.parks)();
    let tenants = (state.rental_tenants)();
    let factories = (state.factories)();
    let floors = (state.factory_floors)();
    let tenant_floors = (state.rental_tenant_floors)();
    let can_manage = can_manage_rental(&(state.roles)(), &(state.menus)());
    let mut keyword = use_signal(String::new);
    let mut occupancy = use_signal(|| "all".to_string());
    let mut selected = use_signal(|| None::<ParkSnapshot>);
    let mut form_open = use_signal(|| false);
    let mut deleting = use_signal(|| None::<ParkSnapshot>);
    let mut notice = use_signal(|| None::<String>);
    let occupancy_value: ReadSignal<Option<String>> = use_memo(move || Some(occupancy())).into();

    let mut snapshots = parks
        .into_iter()
        .filter(|park| !park.is_deleted)
        .map(|park| ParkSnapshot::from_asset_rows(park, &tenants, &factories, &floors, &tenant_floors))
        .collect::<Vec<_>>();
    snapshots.sort_by(|left, right| left.park.park_id.cmp(&right.park.park_id));
    let summary = RentalSummary::from_snapshots(&snapshots);
    let query = keyword().trim().to_lowercase();
    let filter = occupancy();
    let filtered = snapshots
        .iter()
        .filter(|row| {
            let park = &row.park;
            let text_matches = query.is_empty()
                || park.park_name.to_lowercase().contains(&query)
                || park.address.to_lowercase().contains(&query)
                || park
                    .manager
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&query)
                || park.contact.as_deref().unwrap_or_default().contains(&query);
            let occupancy_matches = match filter.as_str() {
                "vacant" => row.occupancy_basis_points < 5_000,
                "balanced" => (5_000..9_000).contains(&row.occupancy_basis_points),
                "full" => row.occupancy_basis_points >= 9_000,
                _ => true,
            };
            text_matches && occupancy_matches
        })
        .cloned()
        .collect::<Vec<_>>();
    let overall_rate = summary.occupancy_basis_points as f64 / 100.0;
    let total_area = format_area(summary.total_area);
    let rented_area = format_area(summary.rented_area);
    let vacant_area = format_area(summary.vacant_area);

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "园区管理" }
                    p { class: "page-subtitle",
                        "以厂房楼层为统计口径，实时汇总已用面积、可招商面积和出租率。"
                    }
                }
                if can_manage {
                    div { class: "page-actions",
                        Button { onclick: move |_| form_open.set(true), "新增园区" }
                    }
                }
            }

            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-split",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "整体出租率" }
                                strong { class: "stat-value", "{overall_rate:.1}%" }
                                Progress { value: overall_rate, max: 100.0 }
                                span { class: "stat-caption",
                                    "{snapshots.len()} 个园区 · {summary.active_contracts} 份有效合同"
                                }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            dl { class: "facts",
                                div {
                                    dt { "总面积" }
                                    dd { "{total_area} ㎡" }
                                }
                                div {
                                    dt { "已出租" }
                                    dd { "{rented_area} ㎡" }
                                }
                                div {
                                    dt { "可招商" }
                                    dd { "{vacant_area} ㎡" }
                                }
                                div {
                                    dt { "有效合同" }
                                    dd { "{summary.active_contracts} 份" }
                                }
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
                                Label { html_for: "park-manage-keyword", "搜索园区" }
                                Input {
                                    id: "park-manage-keyword",
                                    value: keyword,
                                    placeholder: "园区、地址、负责人或电话",
                                    oninput: move |event: FormEvent| keyword.set(event.value()),
                                }
                            }
                            div { class: "field",
                                Label { html_for: "park-manage-occupancy", "出租率" }
                                Select {
                                    id: "park-manage-occupancy",
                                    value: Some(occupancy_value),
                                    on_value_change: move |value: Option<String>| {
                                        occupancy.set(value.unwrap_or_else(|| "all".into()));
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部区间".to_string(), "全部区间" }
                                    SelectOption::<String> { value: "vacant".to_string(), index: 1usize, text_value: "低于 50%".to_string(), "低于 50%" }
                                    SelectOption::<String> { value: "balanced".to_string(), index: 2usize, text_value: "50% - 90%".to_string(), "50% - 90%" }
                                    SelectOption::<String> { value: "full".to_string(), index: 3usize, text_value: "90% 以上".to_string(), "90% 以上" }
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "当前结果" }
                                span { class: "field-static", "{filtered.len()} 个园区" }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "园区台账" }
                    span { class: "hint",
                        if can_manage {
                            "您可以查看、编辑和归档园区主档。"
                        } else {
                            "仅展示当前账号被授权的园区数据。"
                        }
                    }
                }
                if filtered.is_empty() {
                    p { class: "empty",
                        if snapshots.is_empty() {
                            "暂无可查看的园区。请联系系统管理员配置园区数据权限。"
                        } else {
                            "没有符合条件的园区。请修改搜索词或出租率筛选条件。"
                        }
                    }
                } else {
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "园区" }
                                    th { "状态" }
                                    th { "出租率" }
                                    th { "总面积(㎡)" }
                                    th { "已出租(㎡)" }
                                    th { "可招商(㎡)" }
                                    th { "有效合同" }
                                    th { "负责人" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                for row in filtered {
                                    {
                                        let detail = row.clone();
                                        let remove = row.clone();
                                        let park_id = row.park.park_id;
                                        let rate = row.occupancy_percent();
                                        let occupied = format_area(row.rented_area);
                                        let vacant = format_area(row.vacant_area);
                                        let total = format_area(row.total_area);
                                        let enabled = park_is_enabled(row.park.status.as_deref());
                                        let manager = row.park.manager.as_deref().unwrap_or("未设置");
                                        let contact = row.park.contact.as_deref().unwrap_or("--");
                                        rsx! {
                                            tr { key: "park-{park_id}",
                                                td {
                                                    div { class: "stack-tight",
                                                        strong { "{row.park.park_name}" }
                                                        small { class: "hint", "{row.park.address}" }
                                                    }
                                                }
                                                td {
                                                    Badge {
                                                        variant: if enabled { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                                        if enabled {
                                                            "运营中"
                                                        } else {
                                                            "已停用"
                                                        }
                                                    }
                                                }
                                                td {
                                                    // 进度条给出跨行可比的视觉量级，数字负责精确读数
                                                    div { class: "cell-meter",
                                                        span { class: "is-mono", "{rate:.1}%" }
                                                        Progress { value: rate, max: 100.0 }
                                                    }
                                                }
                                                td { class: "is-mono", "{total}" }
                                                td { class: "is-mono", "{occupied}" }
                                                td { class: "is-mono", "{vacant}" }
                                                td { class: "is-mono", "{row.active_contracts.len()}" }
                                                td {
                                                    div { class: "stack-tight",
                                                        span { "{manager}" }
                                                        small { class: "hint", "{contact}" }
                                                    }
                                                }
                                                td {
                                                    div { class: "table-actions",
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            onclick: move |_| selected.set(Some(detail.clone())),
                                                            "经营详情"
                                                        }
                                                        // 厂房、楼层、宿舍和图片的录入统一在园区档案页完成。
                                                        Link { to: Route::RentalParkDetailPage { id: park_id },
                                                            Button { variant: ButtonVariant::Outline, "园区档案" }
                                                        }
                                                        if can_manage {
                                                            Button {
                                                                variant: ButtonVariant::Destructive,
                                                                onclick: move |_| deleting.set(Some(remove.clone())),
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
                }
            }
        }

        if let Some(row) = selected() {
            ParkDetailDialog { snapshot: row, on_close: move |_| selected.set(None) }
        }
        if form_open() {
            ParkProfileDialog {
                on_close: move |_| form_open.set(false),
                on_saved: move |_| {
                    form_open.set(false);
                    notice.set(Some("园区主档已保存，经营指标正在实时更新。".into()));
                },
            }
        }
        if let Some(row) = deleting() {
            ParkDeleteDialog {
                snapshot: row,
                on_close: move |_| deleting.set(None),
                on_deleted: move |_| {
                    deleting.set(None);
                    notice.set(Some("园区及其名下厂房、宿舍、水电表已一并归档，历史记录仍被保留。".into()));
                },
            }
        }
    }
}

#[component]
fn ParkDeleteDialog(
    snapshot: ParkSnapshot,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut completed = use_signal(|| false);
    let park_id = snapshot.park.park_id;
    let park_name = snapshot.park.park_name.clone();
    let contract_count = snapshot.active_contracts.len();
    // 资产台账会随园区一起归档，删之前先把数量说清楚。
    let factory_count = (state.factories)()
        .iter()
        .filter(|row| row.park_id == park_id && !row.is_deleted)
        .count();
    let dormitory_count = (state.dormitories)()
        .iter()
        .filter(|row| row.park_id == park_id && !row.is_deleted)
        .count();
    let meter_count = (state.utility_meters)()
        .iter()
        .filter(|row| row.park_id == park_id && !row.is_deleted)
        .count();
    // 有有效合同时服务端一定拒绝，与其让用户点一个注定报错的按钮，不如直接
    // 说明原因并指向「停用」。
    let blocked = contract_count > 0;
    let description = if blocked {
        format!(
            "园区“{park_name}”下还有 {contract_count} 份有效合同，删除会让它们变成找不到园区的孤立数据。如果只是不再经营，请把园区状态改为「停用」，历史记录会全部保留。"
        )
    } else {
        let cascade = [
            (factory_count, "个厂房"),
            (dormitory_count, "个宿舍"),
            (meter_count, "块水电表"),
        ]
        .iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect::<Vec<_>>();
        if cascade.is_empty() {
            format!("园区“{park_name}”将从租赁台账中隐藏。这是逻辑删除，数据不会被真正删除。")
        } else {
            format!(
                "园区“{park_name}”及其名下 {}（含各自的楼层）将一并归档。这是逻辑删除，数据不会被真正删除。",
                cascade.join("、")
            )
        }
    };

    use_effect(move || {
        if completed() {
            on_deleted.call(());
        }
    });

    rsx! {
        ConfirmDialog {
            title: "确认删除园区",
            description,
            confirm_label: "确认删除",
            busy: loading(),
            confirm_disabled: blocked,
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() {
                    return;
                }
                loading.set(true);
                error.set(None);
                spawn(async move {
                    match delete_park_record(park_id).await {
                        Ok(()) => completed.set(true),
                        Err(message) => {
                            loading.set(false);
                            error.set(Some(message));
                        }
                    }
                });
            },
        }
    }
}
