//! 合同管理实时台账与到期提醒。

use std::collections::HashMap;

use dioxus::prelude::*;

use super::model::{
    contract_reminder, contract_sms_eligible, contract_status, format_date, format_money,
    increase_summary, today_timestamp, ContractStatus,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        total_pages, ConfirmDialog, Pager,
    },
    services::{
        delete_business_images_from_r2, delete_contract_record, send_contract_reminder_sms,
        StoredR2Image,
    },
    spacetime_bindings::{
        rental_tenant_type::RentalTenant, tenant_image_preview_type::TenantImagePreview,
    },
    router::Route,
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 10;

fn images_for_contract(
    rental_tenant_id: u64,
    previews: &[TenantImagePreview],
) -> Vec<TenantImagePreview> {
    previews
        .iter()
        .filter(|image| image.rental_tenant_id == rental_tenant_id)
        .cloned()
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusFilter {
    All,
    Active,
    Expiring,
    Expired,
    Missing,
}

impl StatusFilter {
    fn value(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Active => "active",
            Self::Expiring => "expiring",
            Self::Expired => "expired",
            Self::Missing => "missing",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "active" => Self::Active,
            "expiring" => Self::Expiring,
            "expired" => Self::Expired,
            "missing" => Self::Missing,
            _ => Self::All,
        }
    }

    fn matches(self, status: ContractStatus) -> bool {
        match self {
            Self::All => true,
            Self::Active => status == ContractStatus::Active,
            Self::Expiring => status == ContractStatus::Expiring,
            Self::Expired => status == ContractStatus::Expired,
            Self::Missing => status == ContractStatus::MissingDate,
        }
    }
}

#[component]
pub fn ContractManagementPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let contracts = (state.rental_tenants)();
    let parks = (state.parks)();
    let contract_images = (state.tenant_image_previews)();
    // 合同租在哪里由楼层关联算出，不再是自由文本字段。
    let locations = super::model::contract_locations(
        &(state.factories)(),
        &(state.factory_floors)(),
        &(state.rental_tenant_floors)(),
        &(state.dormitories)(),
        &(state.dormitory_floors)(),
        &(state.rental_tenant_dormitory_floors)(),
    );
    let today = today_timestamp();
    let park_map = parks
        .iter()
        .map(|park| (park.park_id, park.park_name.clone()))
        .collect::<HashMap<_, _>>();

    let mut keyword = use_signal(String::new);
    let mut park_filter = use_signal(String::new);
    let mut type_filter = use_signal(|| "all".to_string());
    let mut status_filter = use_signal(|| StatusFilter::All);
    let mut page = use_signal(|| 1usize);
    let navigator = use_navigator();
    let mut delete_contract = use_signal(|| None::<RentalTenant>);
    let mut sms_loading = use_signal(|| false);
    let mut sms_notice = use_signal(|| None::<String>);
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_filter())).into();
    let type_value: ReadSignal<Option<String>> = use_memo(move || Some(type_filter())).into();
    let status_value: ReadSignal<Option<String>> =
        use_memo(move || Some(status_filter().value().to_string())).into();

    let active_count = contracts
        .iter()
        .filter(|row| contract_status(row, today) == ContractStatus::Active)
        .count();
    let expiring_count = contracts
        .iter()
        .filter(|row| contract_status(row, today) == ContractStatus::Expiring)
        .count();
    let expired_count = contracts
        .iter()
        .filter(|row| contract_status(row, today) == ContractStatus::Expired)
        .count();
    let total_area = contracts
        .iter()
        .filter_map(|row| row.area_centi_square_metres)
        .sum::<i64>() as f64
        / 100.0;

    // 提前算好待提醒的合同 id：批量提醒的闭包会把 contracts 整个搬走，
    // 而下面的指标卡还要读它。
    let sms_eligible_ids = contracts
        .iter()
        .filter(|row| contract_sms_eligible(row, today))
        .map(|row| row.rental_tenant_id)
        .collect::<Vec<_>>();
    let contract_count = contracts.len();

    let normalized_keyword = keyword().trim().to_lowercase();
    let selected_park = park_filter().parse::<u64>().ok();
    let selected_type = type_filter();
    let selected_status = status_filter();
    let mut filtered = contracts
        .iter()
        .filter(|row| {
            (normalized_keyword.is_empty()
                || row.tenant_name.to_lowercase().contains(&normalized_keyword)
                || row.phone_number.contains(&normalized_keyword)
                || locations
                .get(&row.rental_tenant_id)
                .is_some_and(|value| value.to_lowercase().contains(&normalized_keyword)))
                && selected_park.is_none_or(|park_id| row.park_id == park_id)
                && (selected_type == "all"
                || (selected_type == "income" && row.transaction_type)
                || (selected_type == "expense" && !row.transaction_type))
                && selected_status.matches(contract_status(row, today))
        })
        .cloned()
        .collect::<Vec<_>>();
    filtered.sort_by_key(|row| std::cmp::Reverse(row.rental_tenant_id));
    let total = filtered.len();
    let page_count = total_pages(total, PAGE_SIZE);
    // 筛选收窄会让页数变少，越界时回落到最后一页，否则列表显示空白。
    let current_page = page().clamp(1, page_count);
    let page_rows = filtered
        .iter()
        .skip((current_page - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .cloned()
        .collect::<Vec<_>>();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "合同管理" }
                    p { class: "page-subtitle",
                        "管理合同主体、租期、租金、面积和递增规则，并实时提示即将到期合同。"
                    }
                }
                div { class: "page-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: sms_loading(),
                        onclick: move |_| {
                            let ids = sms_eligible_ids.clone();
                            if ids.is_empty() {
                                sms_notice.set(Some("当前没有进入提醒窗口的收入合同".into()));
                                return;
                            }
                            sms_loading.set(true);
                            sms_notice.set(Some(format!("正在发送 {} 条合同提醒…", ids.len())));
                            spawn(async move {
                                match send_contract_reminder_sms(ids, false).await {
                                    Ok(batch) => {
                                        let success = batch.results.iter().filter(|item| item.success).count();
                                        let failed = batch.results.len().saturating_sub(success);
                                        sms_notice
                                            .set(
                                                Some(format!("合同提醒发送完成：成功 {success} 条，失败 {failed} 条。")),
                                            );
                                    }
                                    Err(message) => sms_notice.set(Some(message)),
                                }
                                sms_loading.set(false);
                            });
                        },
                        if sms_loading() {
                            "正在发送…"
                        } else {
                            "批量到期提醒"
                        }
                    }
                    Button {
                        onclick: move |_| {
                            navigator.push(Route::ContractCreatePage {});
                        },
                        "新增合同"
                    }
                }
            }

            if let Some(message) = sms_notice() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-auto",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "合同主体" }
                                strong { class: "stat-value is-mono", "{contract_count}" }
                                span { class: "stat-caption", "当前可见合同" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "生效中" }
                                strong { class: "stat-value is-mono is-ok", "{active_count}" }
                                span { class: "stat-caption", "租期内" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "90 天内到期" }
                                strong { class: "stat-value is-mono", "{expiring_count}" }
                                span { class: "stat-caption", "需提前续签" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已过期" }
                                strong { class: "stat-value is-mono", "{expired_count}" }
                                span { class: "stat-caption", "需确认去留" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "在租面积" }
                                strong { class: "stat-value is-compact is-mono", "{total_area:.2} ㎡" }
                                span { class: "stat-caption", "按当前可见合同汇总" }
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
                                Label { html_for: "contract-keyword", "合同方 / 电话 / 地址" }
                                Input {
                                    id: "contract-keyword",
                                    value: keyword,
                                    placeholder: "输入关键词",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "contract-park", "园区" }
                                Select {
                                    id: "contract-park",
                                    value: Some(park_value),
                                    on_value_change: move |value: Option<String>| {
                                        park_filter.set(value.unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "全部园区".to_string(), "全部园区" }
                                    for (index , park) in parks.iter().enumerate() {
                                        SelectOption::<String> {
                                            key: "park-{park.park_id}",
                                            value: park.park_id.to_string(),
                                            index: index + 1,
                                            text_value: park.park_name.to_string(), "{park.park_name}"
                                        }
                                    }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "contract-type", "交易类型" }
                                Select {
                                    id: "contract-type",
                                    value: Some(type_value),
                                    on_value_change: move |value: Option<String>| {
                                        type_filter.set(value.unwrap_or_else(|| "all".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部类型".to_string(), "全部类型" }
                                    SelectOption::<String> { value: "income".to_string(), index: 1usize, text_value: "收入合同".to_string(), "收入合同" }
                                    SelectOption::<String> { value: "expense".to_string(), index: 2usize, text_value: "支出合同".to_string(), "支出合同" }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "contract-status", "合同状态" }
                                Select {
                                    id: "contract-status",
                                    value: Some(status_value),
                                    on_value_change: move |value: Option<String>| {
                                        status_filter.set(StatusFilter::parse(&value.unwrap_or_default()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    SelectOption::<String> { value: "active".to_string(), index: 1usize, text_value: "生效中".to_string(), "生效中" }
                                    SelectOption::<String> { value: "expiring".to_string(), index: 2usize, text_value: "即将到期".to_string(), "即将到期" }
                                    SelectOption::<String> { value: "expired".to_string(), index: 3usize, text_value: "已过期".to_string(), "已过期" }
                                    SelectOption::<String> { value: "missing".to_string(), index: 4usize, text_value: "待完善".to_string(), "待完善" }
                                }
                            }
                            div { class: "field",
                                Button {
                                    variant: ButtonVariant::Outline,
                                    onclick: move |_| {
                                        keyword.set(String::new());
                                        park_filter.set(String::new());
                                        type_filter.set("all".into());
                                        status_filter.set(StatusFilter::All);
                                        page.set(1);
                                    },
                                    "重置筛选"
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "合同台账" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 份" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "合同方" }
                                th { "类型" }
                                th { "园区 / 地址" }
                                th { "合同期限" }
                                th { "状态提醒" }
                                th { "面积" }
                                th { "月租金" }
                                th { "递增规则" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if page_rows.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "9", "暂无符合条件的合同" }
                                }
                            }
                            for row in page_rows.iter() {
                                {
                                    let status = contract_status(row, today);
                                    let park_name = park_map
                                        .get(&row.park_id)
                                        .cloned()
                                        .unwrap_or_else(|| "未分配园区".into());
                                    let area_label = row
                                        .area_centi_square_metres
                                        .map(|value| format!("{:.2}", value as f64 / 100.0))
                                        .unwrap_or_else(|| "--".into());
                                    let view_row = row.clone();
                                    let edit_row = row.clone();
                                    let remove_row = row.clone();
                                    let sms_row = row.clone();
                                    rsx! {
                                        tr { key: "contract-{row.rental_tenant_id}",
                                            td {
                                                div { class: "stack-tight",
                                                    strong { "{row.tenant_name}" }
                                                    small { class: "hint is-mono", "{row.phone_number}" }
                                                }
                                            }
                                            td {
                                                Badge {
                                                    variant: if row.transaction_type { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                                    if row.transaction_type {
                                                        "收入"
                                                    } else {
                                                        "支出"
                                                    }
                                                }
                                            }
                                            td {
                                                div { class: "stack-tight",
                                                    span { "{park_name}" }
                                                    small { class: "hint",
                                                        {locations.get(&row.rental_tenant_id).map(String::as_str).unwrap_or("未选定楼层")}
                                                    }
                                                }
                                            }
                                            td { class: "is-mono",
                                                "{format_date(row.contract_start)} → {format_date(row.contract_end)}"
                                            }
                                            td {
                                                div { class: "stack-tight",
                                                    Badge { variant: status.badge_variant(), "{status.label()}" }
                                                    small { class: "hint", "{contract_reminder(row, today)}" }
                                                }
                                            }
                                            td { class: "is-mono", "{area_label}" }
                                            td { class: "is-mono", "{format_money(row.rental_amount_cents)}" }
                                            td { class: "is-wrap hint", "{increase_summary(row)}" }
                                            td {
                                                div { class: "table-actions",
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        onclick: move |_| {
                                                            navigator
                                                                .push(Route::ContractDetailPage {
                                                                    id: view_row.rental_tenant_id,
                                                                });
                                                        },
                                                        "查看"
                                                    }
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        onclick: move |_| {
                                                            navigator
                                                                .push(Route::ContractEditPage {
                                                                    id: edit_row.rental_tenant_id,
                                                                });
                                                        },
                                                        "编辑"
                                                    }
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        disabled: sms_loading(),
                                                        title: "手动发送合同提醒",
                                                        onclick: move |_| {
                                                            sms_loading.set(true);
                                                            sms_notice
                                                                .set(Some(format!("正在向 {} 发送合同提醒…", sms_row.tenant_name)));
                                                            let id = sms_row.rental_tenant_id;
                                                            spawn(async move {
                                                                match send_contract_reminder_sms(vec![id], true).await {
                                                                    Ok(batch) => {
                                                                        sms_notice
                                                                            .set(
                                                                                batch
                                                                                    .results
                                                                                    .first()
                                                                                    .map(|item| item.message.clone())
                                                                                    .or(Some("短信服务未返回结果".into())),
                                                                            )
                                                                    }
                                                                    Err(message) => sms_notice.set(Some(message)),
                                                                }
                                                                sms_loading.set(false);
                                                            });
                                                        },
                                                        "短信"
                                                    }
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        class: "is-quiet-danger",
                                                        size: ButtonSize::Sm,
                                                        onclick: move |_| delete_contract.set(Some(remove_row.clone())),
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
                Pager { page, total_pages: page_count, total_count: total }
            }
        }

        if let Some(row) = delete_contract() {
            {
                let images = images_for_contract(row.rental_tenant_id, &contract_images)
                    .into_iter()
                    .map(|image| StoredR2Image {
                        img_id: image.img_id,
                        public_url: image.img_url,
                    })
                    .collect::<Vec<_>>();
                rsx! {
                    ContractDeleteDialog {
                        contract: row,
                        images,
                        on_close: move |_| delete_contract.set(None),
                        on_deleted: move |_| delete_contract.set(None),
                    }
                }
            }
        }
    }
}

#[component]
fn ContractDeleteDialog(
    contract: RentalTenant,
    images: Vec<StoredR2Image>,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut completed = use_signal(|| false);
    // 数据库删除成功但图片清理失败时，重试只需要重跑清理那一步。
    let mut database_deleted = use_signal(|| false);
    let tenant_name = contract.tenant_name.clone();
    let contract_id = contract.rental_tenant_id;

    use_effect(move || {
        if completed() {
            on_deleted.call(());
        }
    });

    rsx! {
        ConfirmDialog {
            title: "确认删除合同",
            description: format!(
                "将停用“{tenant_name}”合同主体，并归档关联工资和图片关系。历史账单会保留原合同方名称。此操作不可撤销。",
            ),
            confirm_label: if database_deleted() { "重试清理" } else { "确认删除" },
            busy: loading(),
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() {
                    return;
                }
                loading.set(true);
                error.set(None);
                let cleanup_images = images.clone();
                spawn(async move {
                    if !database_deleted() {
                        match delete_contract_record(contract_id).await {
                            Ok(()) => database_deleted.set(true),
                            Err(message) => {
                                loading.set(false);
                                error.set(Some(message));
                                return;
                            }
                        }
                    }
                    match delete_business_images_from_r2(cleanup_images).await {
                        Ok(_) => completed.set(true),
                        Err(message) => {
                            loading.set(false);
                            error.set(Some(format!("合同已删除，但{message}；请重试清理")));
                        }
                    }
                });
            },
        }
    }
}
