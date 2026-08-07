//! 账单管理实时应收台账。

use std::collections::BTreeMap;

use dioxus::prelude::*;

use super::{
    collection::BillCollectionDialog,
    form::{BillConfirmDialog, BillDeleteDialog, BillFormDialog},
    import::{BillAiImportDialog, BillCreateModeDialog},
    model::{
        collection_status, confirmation_flags, format_date, format_datetime, format_money,
        overpaid_cents, parse_optional_date, reconciliation_state, remaining_cents, summarize,
        BillSummary, CollectionStatus, ReconciliationState,
    },
    print::BillPrintDialog,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        progress::Progress,
        select::{Select, SelectOption},
        total_pages, DateField, Pager,
    },
    services::{
        download_generated_file, export_amount_bill_excel, load_saved_token, AiAmountBillDraft,
        BillExcelRow,
    },
    spacetime_bindings::amount_bill_type::AmountBill,
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

#[derive(Clone, PartialEq)]
enum BillDialogState {
    Closed,
    CreateChoice,
    Create,
    AiImport,
    AiReview(Vec<AiAmountBillDraft>, usize),
    Collection(Vec<AmountBill>),
    View(AmountBill),
    Edit(AmountBill),
    NextMonth(AmountBill),
    Print(AmountBill),
    Delete(AmountBill),
    /// 确认已收齐——唯一能闭环的确认。
    ConfirmCollected(AmountBill),
    /// 确认差额——只表示知悉，不闭环。
    ConfirmShortfall(AmountBill),
}

fn build_excel_rows(rows: &[AmountBill], park_map: &BTreeMap<u64, String>) -> Vec<BillExcelRow> {
    rows.iter()
        .map(|bill| BillExcelRow {
            park_name: park_map
                .get(&bill.park_id)
                .cloned()
                .unwrap_or_else(|| "园区已移除".into()),
            project_name: bill.project_name.clone(),
            tenant_name: bill.tenant_name.clone().unwrap_or_default(),
            ele_fee_cents: bill.ele_fee_cents,
            water_fee_cents: bill.water_fee_cents,
            factory_rent_cents: bill.factory_rent_cents,
            management_fee_cents: bill.management_fee_cents,
            service_fee_cents: bill.service_fee_cents,
            garbage_fee_cents: bill.garbage_fee_cents,
            invoice_tax_cents: bill.invoice_tax_cents,
            penalty_fee_cents: bill.penalty_fee_cents.unwrap_or_default(),
            receive_fee_cents: bill.receive_fee_cents,
            total_fee_cents: bill.total_fee_cents,
            receipt_amount_cents: bill.receipt_amount_cents,
            remaining_amount_cents: remaining_cents(bill),
            collection_status: collection_status(bill).label().into(),
            receipt_date: format_date(bill.receipt_time),
            remark: bill.remark.clone().unwrap_or_default(),
        })
        .collect()
}

fn is_system_admin(state: WorkspaceState) -> bool {
    state
        .roles
        .read()
        .iter()
        .any(|role| role.name == "Super" && role.scope == "system" && role.status == 1)
}

#[component]
pub fn BillManagementPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let tenants = state.rental_tenants.read().clone();
    let parks = state.parks.read().clone();
    let can_manage = is_system_admin(state);
    // 确认是园区经理的动作（园区管理权限），与账单编辑（系统管理员）是两个岗位
    // 两条边界；服务端 require_rental_manager 是权威，这里只决定按钮显不显示。
    let can_confirm = {
        let roles = state.roles.read();
        let menus = state.menus.read();
        crate::permissions::can_manage_rental(&roles, &menus)
    };
    // 确认记录走订阅：确认一落库状态列实时更新，不必等下一次分页查询。
    let recon_flags = confirmation_flags(&state.bill_collection_confirmations.read());
    let park_map = parks
        .iter()
        .map(|park| (park.park_id, park.park_name.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut park_filter = use_signal(String::new);
    let mut project_filter = use_signal(String::new);
    let mut tenant_filter = use_signal(String::new);
    let mut status_filter = use_signal(String::new);
    // 账期按自然月走，默认就把日期区间落在当月——业务上看账单几乎总是先看本月，
    // 不限区间会把历史账单一起拉出来，首屏还得再筛一次。
    let default_range = use_hook(crate::pages::smart_meter::date::current_month_range);
    let mut bill_start = use_signal({
        let start = default_range.0.clone();
        move || start
    });
    let mut bill_end = use_signal({
        let end = default_range.1.clone();
        move || end
    });
    let mut page = use_signal(|| 1usize);
    let mut dialog = use_signal(|| BillDialogState::Closed);
    let mut exporting = use_signal(|| false);
    let mut action_message = use_signal(|| None::<String>);
    let mut action_error = use_signal(|| None::<String>);
    // 下面三个由服务端查询回调写入，回调要求 Send + Sync。
    let mut history = use_signal_sync(|| {
        None::<crate::spacetime_bindings::billing_page_result_type::BillingPageResult>
    });
    let mut history_loading = use_signal_sync(|| true);
    let mut history_error = use_signal_sync(|| None::<String>);

    let selected_park = park_filter().parse::<u64>().ok();
    let project_keyword = project_filter().trim().to_lowercase();
    let tenant_keyword = tenant_filter().trim().to_lowercase();
    let start_micros = parse_optional_date(&bill_start())
        .ok()
        .flatten()
        .map(|value| value.to_micros_since_unix_epoch());
    let end_micros = parse_optional_date(&bill_end())
        .ok()
        .flatten()
        .map(|value| value.to_micros_since_unix_epoch() + 86_400_000_000 - 1);
    use_effect(move || {
        let _dialog_state = dialog();
        let query = crate::services::BillingPageQuery {
            page: page() as u32,
            page_size: PAGE_SIZE as u32,
            project_keyword: project_filter(),
            tenant_keyword: tenant_filter(),
            collection_status: (!status_filter().is_empty()).then(|| status_filter()),
            park_id: park_filter().parse::<u64>().ok(),
            start_time_micros: parse_optional_date(&bill_start())
                .ok()
                .flatten()
                .map(|value| value.to_micros_since_unix_epoch()),
            end_time_micros: parse_optional_date(&bill_end())
                .ok()
                .flatten()
                .map(|value| value.to_micros_since_unix_epoch() + 86_400_000_000 - 1),
        };
        history_loading.set(true);
        history_error.set(None);
        let result = crate::services::query_billing_history(query, move |result| match result {
            Ok(value) => {
                if let Ok(mut history) = history.try_write() {
                    *history = Some(value);
                }
                if let Ok(mut loading) = history_loading.try_write() {
                    *loading = false;
                }
            }
            Err(message) => {
                if let Ok(mut error) = history_error.try_write() {
                    *error = Some(message);
                }
                if let Ok(mut loading) = history_loading.try_write() {
                    *loading = false;
                }
            }
        });
        if let Err(message) = result {
            history_error.set(Some(message));
            history_loading.set(false);
        }
    });
    let history_snapshot = history();
    let bills = history_snapshot
        .as_ref()
        .map(|value| value.rows.clone())
        .unwrap_or_default();
    let rows = bills
        .iter()
        .filter(|bill| selected_park.is_none_or(|park_id| bill.park_id == park_id))
        .filter(|bill| {
            project_keyword.is_empty()
                || bill.project_name.to_lowercase().contains(&project_keyword)
        })
        .filter(|bill| {
            tenant_keyword.is_empty()
                || bill
                    .tenant_name
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&tenant_keyword)
        })
        .filter(|bill| {
            let status = collection_status(bill).value();
            match status_filter().as_str() {
                "" => true,
                "unreceived" => matches!(
                    collection_status(bill),
                    CollectionStatus::Unpaid | CollectionStatus::Partial
                ),
                selected => status == selected,
            }
        })
        .filter(|bill| {
            let created_micros = bill.created_at.to_micros_since_unix_epoch();
            start_micros.is_none_or(|start| created_micros >= start)
                && end_micros.is_none_or(|end| created_micros <= end)
        })
        .cloned()
        .collect::<Vec<_>>();
    let summary = summarize(rows.iter());
    // 全量聚合必须取服务端的：`bills` 只是当前这一页的 20 行，拿它 fold 出来的
    // 是「本页合计」，而卡片上写着「总体回款率」「应收总额」。第一页恰好都是
    // 未收款时，回款率就永远显示 0%，和真实数据无关。
    //
    // 服务端在 procedures/history.rs 里按同一套筛选条件、在分页之前算好这四个值，
    // 口径与这里的 summarize 完全一致（outstanding = remaining，overpaid 同名）。
    let all_summary = history_snapshot
        .as_ref()
        .map(|value| BillSummary {
            total_cents: value.receivable_cents,
            receipt_cents: value.received_cents,
            remaining_cents: value.outstanding_cents,
            overpaid_cents: value.overpaid_cents,
        })
        .unwrap_or_default();
    let status_counts = [
        CollectionStatus::Unpaid,
        CollectionStatus::Partial,
        CollectionStatus::Paid,
        CollectionStatus::Overpaid,
    ]
    .map(|status| {
        (
            status,
            bills
                .iter()
                .filter(|bill| collection_status(bill) == status)
                .count(),
        )
    });
    let total = history_snapshot
        .as_ref()
        .map(|value| value.total)
        .unwrap_or_default() as usize;
    // 分页由服务端按 page/page_size 返回，这里只负责算总页数。
    let page_count = total_pages(total, PAGE_SIZE);
    let visible_rows = rows.clone();
    let collection_rate = if all_summary.total_cents <= 0 {
        0
    } else {
        ((all_summary.receipt_cents.min(all_summary.total_cents) as i128 * 100)
            / all_summary.total_cents as i128) as i64
    };
    let rows_for_export = rows.clone();
    let export_park_map = park_map.clone();
    let rows_for_collection = rows.clone();
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_filter())).into();
    let status_value: ReadSignal<Option<String>> = use_memo(move || Some(status_filter())).into();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "账单管理" }
                    p { class: "page-subtitle",
                        "统一管理费用构成、应收实收、收款状态与财务流水。"
                    }
                }
                if can_manage {
                    div { class: "page-actions",
                        Button {
                            variant: ButtonVariant::Outline,
                            r#type: "button",
                            onclick: move |_| dialog.set(BillDialogState::Collection(rows_for_collection.clone())),
                            "催收短信"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            r#type: "button",
                            disabled: exporting(),
                            onclick: move |_| {
                                if exporting() {
                                    return;
                                }
                                let Some(token) = load_saved_token() else {
                                    action_error.set(Some("登录凭证不存在，请重新登录".into()));
                                    return;
                                };
                                let excel_rows = build_excel_rows(&rows_for_export, &export_park_map);
                                exporting.set(true);
                                action_error.set(None);
                                action_message.set(None);
                                spawn(async move {
                                    match export_amount_bill_excel(token, excel_rows).await {
                                        Ok(file) => {
                                            match download_generated_file(file) {
                                                Ok(()) => action_message.set(Some("Excel 已生成并开始下载".into())),
                                                Err(message) => action_error.set(Some(message)),
                                            }
                                        }
                                        Err(error) => {
                                            action_error.set(Some(format!("导出 Excel 失败：{error}")))
                                        }
                                    }
                                    exporting.set(false);
                                });
                            },
                            if exporting() {
                                "正在导出…"
                            } else {
                                "导出 Excel"
                            }
                        }
                        Button {
                            r#type: "button",
                            onclick: move |_| dialog.set(BillDialogState::CreateChoice),
                            "新增总账单"
                        }
                    }
                }
            }

            if history_loading() && history_snapshot.is_none() {
                p { class: "notice", role: "status", "正在加载账单记录…" }
            }
            if let Some(message) = history_error() {
                p { class: "form-error", role: "alert", "账单记录加载失败：{message}" }
            }
            if let Some(message) = action_message() {
                p { class: "notice", role: "status", "{message}" }
            }
            if let Some(message) = action_error() {
                p { class: "form-error", role: "alert", "{message}" }
            }

            section { class: "grid-split",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "总体回款率" }
                                strong { class: "stat-value is-mono", "{collection_rate}%" }
                                Progress { value: collection_rate as f64, max: 100.0 }
                                div { class: "row",
                                    for (status , count) in status_counts {
                                        Badge {
                                            key: "status-{status.label()}",
                                            variant: status.badge_variant(),
                                            "{status.label()} {count}"
                                        }
                                    }
                                }
                                // 这几个计数只能按当前页统计：服务端的分页 Procedure
                                // 只返回四项金额聚合，没有按状态的条数。标注清楚口径，
                                // 免得和上面的全量回款率混为一谈。
                                span { class: "stat-caption", "状态分布按本页 {bills.len()} 份统计" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "未收敞口" }
                                strong { class: "stat-value is-compact is-mono",
                                    "{format_money(all_summary.remaining_cents)}"
                                }
                                span { class: "stat-caption",
                                    if all_summary.overpaid_cents > 0 {
                                        "另有多收 {format_money(all_summary.overpaid_cents)}"
                                    } else {
                                        "账实状态实时计算"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "grid-4",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "应收总额" }
                                strong { class: "stat-value is-compact is-mono", "{format_money(all_summary.total_cents)}" }
                                // total 是筛选后的全部条数；bills 只有当前页，写在这里会一直是 20
                                span { class: "stat-caption", "{total} 份权限内账单" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "实收总额" }
                                strong { class: "stat-value is-compact is-mono is-ok", "{format_money(all_summary.receipt_cents)}" }
                                span { class: "stat-caption", "实时关联收款状态" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "当前筛选未收" }
                                strong { class: "stat-value is-compact is-mono", "{format_money(summary.remaining_cents)}" }
                                span { class: "stat-caption", "共 {rows.len()} 条筛选结果" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "多收金额" }
                                strong { class: "stat-value is-compact is-mono", "{format_money(all_summary.overpaid_cents)}" }
                                span { class: "stat-caption", "需要后续核销" }
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
                                Label { html_for: "bill-park", "园区" }
                                Select {
                                    id: "bill-park",
                                    value: Some(park_value),
                                    on_value_change: move |value: Option<String>| {
                                        park_filter.set(value.unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "全部园区".to_string(), "全部园区" }
                                    for (index , park) in parks.iter().enumerate() {
                                        SelectOption::<String> {
                                            key: "bill-park-{park.park_id}",
                                            value: park.park_id.to_string(),
                                            index: index + 1,
                                            text_value: park.park_name.to_string(),
                                            "{park.park_name}"
                                        }
                                    }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "bill-project", "项目名称" }
                                Input {
                                    id: "bill-project",
                                    value: project_filter,
                                    placeholder: "输入或选择账期项目",
                                    oninput: move |event: FormEvent| {
                                        project_filter.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "bill-tenant", "租户名称" }
                                Input {
                                    id: "bill-tenant",
                                    value: tenant_filter,
                                    placeholder: "输入租赁客户名称",
                                    oninput: move |event: FormEvent| {
                                        tenant_filter.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "bill-status", "收款状态" }
                                Select {
                                    id: "bill-status",
                                    value: Some(status_value),
                                    on_value_change: move |value: Option<String>| {
                                        status_filter.set(value.unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    SelectOption::<String> { value: "unreceived".to_string(), index: 1usize, text_value: "未收合计".to_string(), "未收合计（未收+部分）" }
                                    SelectOption::<String> { value: "unpaid".to_string(), index: 2usize, text_value: "未收款".to_string(), "未收款" }
                                    SelectOption::<String> { value: "partial".to_string(), index: 3usize, text_value: "部分收款".to_string(), "部分收款" }
                                    SelectOption::<String> { value: "paid".to_string(), index: 4usize, text_value: "已收款".to_string(), "已收款" }
                                    SelectOption::<String> { value: "overpaid".to_string(), index: 5usize, text_value: "多收".to_string(), "多收" }
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "账单开始日期" }
                                DateField {
                                    value: bill_start(),
                                    max: (!bill_end().is_empty()).then(|| bill_end()),
                                    on_change: move |value: String| {
                                        bill_start.set(value);
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "账单结束日期" }
                                DateField {
                                    value: bill_end(),
                                    min: (!bill_start().is_empty()).then(|| bill_start()),
                                    on_change: move |value: String| {
                                        bill_end.set(value);
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field is-action",
                                Button {
                                    variant: ButtonVariant::Outline,
                                    r#type: "button",
                                    onclick: {
                                        let default_range = default_range.clone();
                                        move |_| {
                                            park_filter.set(String::new());
                                            project_filter.set(String::new());
                                            tenant_filter.set(String::new());
                                            status_filter.set(String::new());
                                            // 重置回当月，而不是清空——清空后的「无区间」不是
                                            // 默认状态，再点重置也回不到进页面时看到的样子。
                                            bill_start.set(default_range.0.clone());
                                            bill_end.set(default_range.1.clone());
                                            page.set(1);
                                        }
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
                    h2 { "总账单列表" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 份" }
                }
                {
                    // 「钱到了却没确认」要顶到眼前（租金收缴与对账确认流程.md §4.1）：
                    // 实收已达应收、只差经理点一下的账单，停着不动就会在账期截止时
                    // 被结转成一笔并不存在的欠款。只数当前页，数字口径如实写明。
                    let awaiting = visible_rows
                        .iter()
                        .filter(|bill| {
                            let (collected, shortfall) = recon_flags
                                .get(&bill.bill_id)
                                .copied()
                                .unwrap_or((false, false));
                            reconciliation_state(bill, collected, shortfall)
                                == ReconciliationState::AwaitingCollectedConfirm
                        })
                        .count();
                    rsx! {
                        if awaiting > 0 {
                            p { class: "notice", role: "status",
                                "本页有 {awaiting} 张账单实收已达应收但尚未确认收齐，请园区经理及时确认。"
                            }
                        }
                    }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "园区 / 项目" }
                                th { "租户" }
                                th { "电费" }
                                th { "水费" }
                                th { "租金" }
                                th { "应收" }
                                th { "实收" }
                                th { "未收" }
                                th { "状态" }
                                th { "创建 / 收款" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if visible_rows.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "11", "暂无符合条件的账单记录" }
                                }
                            }
                            for row in visible_rows.iter() {
                                {
                                    let status = collection_status(row);
                                    let (has_collected, has_shortfall) = recon_flags
                                        .get(&row.bill_id)
                                        .copied()
                                        .unwrap_or((false, false));
                                    let recon = reconciliation_state(row, has_collected, has_shortfall);
                                    let confirm_collected_row = row.clone();
                                    let confirm_shortfall_row = row.clone();
                                    let park_name = park_map
                                        .get(&row.park_id)
                                        .cloned()
                                        .unwrap_or_else(|| "园区已移除".into());
                                    let view_row = row.clone();
                                    let row_for_open = row.clone();
                                    let edit_row = row.clone();
                                    let next_row = row.clone();
                                    let print_row = row.clone();
                                    let delete_row = row.clone();
                                    let remaining = remaining_cents(row);
                                    let overpaid = overpaid_cents(row);
                                    rsx! {
                                        // 整行打开只读详情：表格列已经很挤，账单构成要在弹窗里看
                                        tr {
                                            key: "bill-{row.bill_id}",
                                            class: "is-clickable",
                                            onclick: move |_| dialog.set(BillDialogState::View(row_for_open.clone())),
                                            td {
                                                div { class: "stack-tight",
                                                    strong { "{park_name}" }
                                                    small { class: "hint", "{row.project_name}" }
                                                }
                                            }
                                            td {
                                                strong { {row.tenant_name.as_deref().unwrap_or("--")} }
                                            }
                                            td { class: "is-mono", "{format_money(row.ele_fee_cents)}" }
                                            td { class: "is-mono", "{format_money(row.water_fee_cents)}" }
                                            td { class: "is-mono", "{format_money(row.factory_rent_cents)}" }
                                            td { class: "is-mono", "{format_money(row.total_fee_cents)}" }
                                            td { class: "is-mono is-ok", "{format_money(row.receipt_amount_cents)}" }
                                            td { class: "is-mono", "{format_money(remaining)}" }
                                            td {
                                                div { class: "stack-tight",
                                                    Badge { variant: status.badge_variant(), "{status.label()}" }
                                                    // 对账状态与收款状态是两回事：上面是钱到没到，
                                                    // 这里是经理认没认（租金收缴与对账确认流程.md）。
                                                    Badge { variant: recon.badge_variant(), "{recon.label()}" }
                                                    if overpaid > 0 {
                                                        small { class: "hint is-mono", "+{format_money(overpaid)}" }
                                                    }
                                                }
                                            }
                                            td {
                                                // 「建 / 收」缩到一个字不好认，标签和日期也对不齐基线。
                                                // 拆成标签 + 等宽日期两栏。
                                                dl { class: "cell-facts",
                                                    dt { "创建" }
                                                    dd { class: "is-mono", "{format_datetime(row.created_at)}" }
                                                    dt { "收款" }
                                                    dd { class: "is-mono", "{format_date(row.receipt_time)}" }
                                                }
                                            }
                                            td {
                                                // 操作列拦冒泡，否则点按钮会连带打开详情
                                                div {
                                                    class: "table-actions",
                                                    onclick: move |event| event.stop_propagation(),
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: move |_| dialog.set(BillDialogState::View(view_row.clone())),
                                                        "查看"
                                                    }
                                                    if can_confirm && recon.can_confirm_collected() {
                                                        Button {
                                                            size: ButtonSize::Sm,
                                                            r#type: "button",
                                                            onclick: move |_| dialog.set(BillDialogState::ConfirmCollected(confirm_collected_row.clone())),
                                                            "确认收齐"
                                                        }
                                                    }
                                                    if can_confirm && recon.can_confirm_shortfall() {
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            size: ButtonSize::Sm,
                                                            r#type: "button",
                                                            onclick: move |_| dialog.set(BillDialogState::ConfirmShortfall(confirm_shortfall_row.clone())),
                                                            "确认差额"
                                                        }
                                                    }
                                                    if can_manage {
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            size: ButtonSize::Sm,
                                                            r#type: "button",
                                                            onclick: move |_| dialog.set(BillDialogState::NextMonth(next_row.clone())),
                                                            "新增下月"
                                                        }
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            size: ButtonSize::Sm,
                                                            r#type: "button",
                                                            onclick: move |_| dialog.set(BillDialogState::Edit(edit_row.clone())),
                                                            "编辑"
                                                        }
                                                    }
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: move |_| dialog.set(BillDialogState::Print(print_row.clone())),
                                                        "打印"
                                                    }
                                                    if can_manage {
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            class: "is-quiet-danger",
                                                            size: ButtonSize::Sm,
                                                            r#type: "button",
                                                            onclick: move |_| dialog.set(BillDialogState::Delete(delete_row.clone())),
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
                        if !visible_rows.is_empty() {
                            tfoot {
                                tr {
                                    td { colspan: "5", "当前页合计" }
                                    td { class: "is-mono",
                                        "{format_money(visible_rows.iter().map(|row| row.total_fee_cents).sum())}"
                                    }
                                    td { class: "is-mono",
                                        "{format_money(visible_rows.iter().map(|row| row.receipt_amount_cents).sum())}"
                                    }
                                    td { class: "is-mono",
                                        "{format_money(visible_rows.iter().map(remaining_cents).sum())}"
                                    }
                                    td { colspan: "3" }
                                }
                            }
                        }
                    }
                }
                Pager { page, total_pages: page_count, total_count: total }
            }
        }

        match dialog() {
            BillDialogState::Closed => rsx! {},
            BillDialogState::CreateChoice => rsx! { BillCreateModeDialog { on_manual: move |_| dialog.set(BillDialogState::Create), on_ai: move |_| dialog.set(BillDialogState::AiImport), on_close: move |_| dialog.set(BillDialogState::Closed) } },
            BillDialogState::Create => rsx! { BillFormDialog { bill: None, draft: None, review_label: None, tenants: tenants.clone(), parks: parks.clone(), create_as_new: false, readonly: false, on_close: move |_| dialog.set(BillDialogState::Closed), on_saved: move |_| dialog.set(BillDialogState::Closed) } },
            BillDialogState::AiImport => rsx! { BillAiImportDialog { on_close: move |_| dialog.set(BillDialogState::Closed), on_analyzed: move |drafts| dialog.set(BillDialogState::AiReview(drafts, 0)) } },
            BillDialogState::AiReview(drafts, index) => {
                let total = drafts.len();
                let draft = drafts.get(index).cloned().unwrap_or_default();
                let saved_drafts = drafts.clone();
                rsx! { BillFormDialog { bill: None, draft: Some(draft), review_label: Some(format!("第 {} / {} 份", index + 1, total)), tenants: tenants.clone(), parks: parks.clone(), create_as_new: false, readonly: false,
                    on_close: move |_| dialog.set(BillDialogState::Closed),
                    on_saved: move |_| if index + 1 < saved_drafts.len() { dialog.set(BillDialogState::AiReview(saved_drafts.clone(), index + 1)); } else { action_message.set(Some(format!("AI 导入的 {total} 份账单已全部核对保存"))); dialog.set(BillDialogState::Closed); }
                } }
            },
            BillDialogState::Collection(collection_rows) => rsx! { BillCollectionDialog { bills: collection_rows, tenants: tenants.clone(), parks: parks.clone(), on_close: move |_| dialog.set(BillDialogState::Closed) } },
            BillDialogState::View(row) => rsx! { BillFormDialog { bill: Some(row), draft: None, review_label: None, tenants: tenants.clone(), parks: parks.clone(), create_as_new: false, readonly: true, on_close: move |_| dialog.set(BillDialogState::Closed), on_saved: move |_| dialog.set(BillDialogState::Closed) } },
            BillDialogState::Edit(row) => rsx! { BillFormDialog { bill: Some(row), draft: None, review_label: None, tenants: tenants.clone(), parks: parks.clone(), create_as_new: false, readonly: false, on_close: move |_| dialog.set(BillDialogState::Closed), on_saved: move |_| dialog.set(BillDialogState::Closed) } },
            BillDialogState::NextMonth(row) => rsx! { BillFormDialog { bill: Some(row), draft: None, review_label: None, tenants: tenants.clone(), parks: parks.clone(), create_as_new: true, readonly: false, on_close: move |_| dialog.set(BillDialogState::Closed), on_saved: move |_| dialog.set(BillDialogState::Closed) } },
            BillDialogState::Print(row) => { let park_name = park_map.get(&row.park_id).cloned().unwrap_or_else(|| "--".into()); rsx! { BillPrintDialog { bill: row, park_name, on_close: move |_| dialog.set(BillDialogState::Closed) } } },
            BillDialogState::Delete(row) => rsx! { BillDeleteDialog { bill: row, on_close: move |_| dialog.set(BillDialogState::Closed), on_deleted: move |_| dialog.set(BillDialogState::Closed) } },
            BillDialogState::ConfirmCollected(row) => rsx! { BillConfirmDialog { bill: row, collected: true,
                on_close: move |_| dialog.set(BillDialogState::Closed),
                on_confirmed: move |_| { action_message.set(Some("已确认收齐，本张账单收缴闭环".into())); dialog.set(BillDialogState::Closed); } } },
            BillDialogState::ConfirmShortfall(row) => rsx! { BillConfirmDialog { bill: row, collected: false,
                on_close: move |_| dialog.set(BillDialogState::Closed),
                on_confirmed: move |_| { action_message.set(Some("差额已确认知悉，请继续向租户催交".into())); dialog.set(BillDialogState::Closed); } } },
        }
    }
}
