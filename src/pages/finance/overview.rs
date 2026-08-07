//! 独立于租赁账单的财务收支流水台账。

use dioxus::{html::FileData, prelude::*};

use super::model::{format_date, format_money, money_input, parse_date, parse_money, today};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        total_pages, ConfirmDialog, DateField, Pager,
    },
    services::{
        create_finance_with_images_record, delete_finance_record, query_finance_history,
        update_finance_with_images_record, upload_business_image, validate_business_image,
        FinancePageQuery,
    },
    spacetime_bindings::{
        finance_image_type::FinanceImage, finance_input_type::FinanceInput,
        finance_page_result_type::FinancePageResult, finance_type::Finance, park_type::Park,
    },
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

#[derive(Clone, PartialEq)]
enum FinanceProof {
    Existing {
        id: u64,
        url: String,
        removed: bool,
    },
    Pending {
        file: FileData,
        name: String,
        preview_url: Option<String>,
    },
}

impl FinanceProof {
    fn url(&self) -> Option<&str> {
        match self {
            Self::Existing { url, .. } => Some(url),
            Self::Pending { preview_url, .. } => preview_url.as_deref(),
        }
    }
    fn name(&self) -> String {
        match self {
            Self::Existing { id, .. } => format!("财务凭证 #{id}"),
            Self::Pending { name, .. } => name.clone(),
        }
    }
    fn removed(&self) -> bool {
        matches!(self, Self::Existing { removed: true, .. })
    }
}

#[cfg(target_arch = "wasm32")]
fn create_finance_preview(file: &FileData) -> Option<String> {
    use dioxus::web::WebFileExt;
    file.get_web_file()
        .and_then(|file| web_sys::Url::create_object_url_with_blob(file.as_ref()).ok())
}
#[cfg(not(target_arch = "wasm32"))]
fn create_finance_preview(_file: &FileData) -> Option<String> {
    None
}

#[component]
pub fn FinanceManagementPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let parks = (state.parks)();
    let mut keyword = use_signal(String::new);
    let mut transaction_type = use_signal(|| "全部".to_string());
    let mut park_filter = use_signal(|| 0_u64);
    // 与账单管理一致：默认只看当月。上面四张统计卡取的是服务端按同一套筛选
    // 条件算出的聚合，所以区间一变，卡片数字跟着变——标题因此写「本期」而不是
    // 「累计」，否则默认视图会把当月数据说成历史累计。
    let default_range = use_hook(crate::pages::smart_meter::date::current_month_range);
    let mut start_date = use_signal({
        let start = default_range.0.clone();
        move || start
    });
    let mut end_date = use_signal({
        let end = default_range.1.clone();
        move || end
    });
    let mut mask_amount = use_signal(|| true);
    let mut editing = use_signal(|| None::<Finance>);
    let mut form_open = use_signal(|| false);
    let mut deleting = use_signal(|| None::<Finance>);
    let mut viewing = use_signal(|| None::<Finance>);
    let mut notice = use_signal(|| None::<String>);
    let mut page = use_signal(|| 1usize);
    // 下面三个由服务端查询回调写入，回调要求 Send + Sync。
    let mut history = use_signal_sync(|| None::<FinancePageResult>);
    let mut history_loading = use_signal_sync(|| true);
    let mut history_error = use_signal_sync(|| None::<String>);

    use_effect(move || {
        let keyword = keyword();
        let transaction_type = match transaction_type().as_str() {
            "全部" => None,
            value => Some(value.to_string()),
        };
        let park_id = (park_filter() != 0).then_some(park_filter());
        let start_time_micros = parse_date(&start_date())
            .ok()
            .map(|value| value.to_micros_since_unix_epoch());
        let end_time_micros = parse_date(&end_date())
            .ok()
            .map(|value| value.to_micros_since_unix_epoch() + 86_400_000_000 - 1);
        let current_page = page() as u32;
        let _dialog_state = (form_open(), deleting());
        history_loading.set(true);
        history_error.set(None);
        let result = query_finance_history(
            FinancePageQuery {
                page: current_page,
                page_size: PAGE_SIZE as u32,
                keyword,
                transaction_type,
                park_id,
                start_time_micros,
                end_time_micros,
            },
            move |result| match result {
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
            },
        );
        if let Err(message) = result {
            history_error.set(Some(message));
            history_loading.set(false);
        }
    });

    let park_names = parks
        .iter()
        .map(|park| (park.park_id, park.park_name.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let history_snapshot = history();
    let rows = history_snapshot
        .as_ref()
        .map(|value| value.rows.clone())
        .unwrap_or_default();
    let finance_images = history_snapshot
        .as_ref()
        .map(|value| value.images.clone())
        .unwrap_or_default();
    let income = history_snapshot
        .as_ref()
        .map(|value| value.income_cents)
        .unwrap_or_default();
    let expense = history_snapshot
        .as_ref()
        .map(|value| value.expense_cents)
        .unwrap_or_default();
    let income_count = history_snapshot
        .as_ref()
        .map(|value| value.income_count)
        .unwrap_or_default();
    let expense_count = history_snapshot
        .as_ref()
        .map(|value| value.expense_count)
        .unwrap_or_default();
    let total = history_snapshot
        .as_ref()
        .map(|value| value.total)
        .unwrap_or_default() as usize;
    let filtered = rows.clone();
    // 分页由服务端按 page/page_size 返回，这里只负责算总页数。
    let page_count = total_pages(total, PAGE_SIZE);
    let visible_rows = filtered.clone();
    let income_label = format_money(income);
    let expense_label = format_money(expense);
    let balance_label = format_money(income.saturating_sub(expense));
    let amounts_masked = mask_amount();
    let type_value: ReadSignal<Option<String>> = use_memo(move || Some(transaction_type())).into();
    let park_value: ReadSignal<Option<String>> =
        use_memo(move || Some(park_filter().to_string())).into();
    let form_images = editing()
        .as_ref()
        .map(|record| {
            finance_images
                .iter()
                .filter(|image| image.finance_id == record.finance_id)
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "财务管理" }
                    p { class: "page-subtitle",
                        "记录真实发生的收入与支出；租赁账单和报销审批会同步形成关联流水。"
                    }
                }
                div { class: "page-actions",
                    // 金额脱敏是给「当着客户的面看账」用的，放在页头随手可切
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        aria_pressed: amounts_masked,
                        onclick: move |_| {
                            let current = mask_amount();
                            mask_amount.set(!current);
                        },
                        if amounts_masked {
                            "显示金额"
                        } else {
                            "金额脱敏"
                        }
                    }
                    Button {
                        onclick: move |_| {
                            editing.set(None);
                            form_open.set(true);
                        },
                        "新增流水"
                    }
                }
            }

            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }
            if history_loading() && history_snapshot.is_none() {
                p { class: "notice", role: "status", "正在加载财务流水…" }
            }
            if let Some(message) = history_error() {
                p { class: "form-error", role: "alert", "财务流水加载失败：{message}" }
            }

            section { class: "grid-4", aria_label: "财务收支概览",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "本期收入" }
                                strong { class: "stat-value is-compact is-mono is-ok", "{income_label}" }
                                span { class: "stat-caption", "{income_count} 笔收入" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "本期支出" }
                                strong { class: "stat-value is-compact is-mono", "{expense_label}" }
                                span { class: "stat-caption", "{expense_count} 笔支出" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "账面净额" }
                                strong { class: "stat-value is-compact is-mono", "{balance_label}" }
                                span { class: "stat-caption", "收入减支出" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "流水总数" }
                                strong { class: "stat-value is-mono", "{total}" }
                                span { class: "stat-caption", "按需分页" }
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
                                Label { html_for: "finance-keyword", "搜索流水" }
                                Input {
                                    id: "finance-keyword",
                                    value: keyword,
                                    placeholder: "账目名称、分类或备注",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "finance-type", "收支类型" }
                                Select {
                                    id: "finance-type",
                                    value: Some(type_value),
                                    on_value_change: move |value: Option<String>| {
                                        transaction_type.set(value.unwrap_or_else(|| "全部".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "全部".to_string(), index: 0usize, text_value: "全部".to_string(), "全部" }
                                    SelectOption::<String> { value: "收入".to_string(), index: 1usize, text_value: "收入".to_string(), "收入" }
                                    SelectOption::<String> { value: "支出".to_string(), index: 2usize, text_value: "支出".to_string(), "支出" }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "finance-park", "所属园区" }
                                Select {
                                    id: "finance-park",
                                    value: Some(park_value),
                                    on_value_change: move |value: Option<String>| {
                                        park_filter
                                            .set(value.and_then(|value| value.parse().ok()).unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "0".to_string(), index: 0usize, text_value: "全部园区".to_string(), "全部园区" }
                                    for (index , park) in parks.iter().enumerate() {
                                        SelectOption::<String> {
                                            key: "finance-park-{park.park_id}",
                                            value: park.park_id.to_string(),
                                            index: index + 1,
                                            text_value: park.park_name.to_string(),
                                            "{park.park_name}"
                                        }
                                    }
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "开始日期" }
                                DateField {
                                    value: start_date(),
                                    max: (!end_date().is_empty()).then(|| end_date()),
                                    on_change: move |value: String| {
                                        start_date.set(value);
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "结束日期" }
                                DateField {
                                    value: end_date(),
                                    min: (!start_date().is_empty()).then(|| start_date()),
                                    on_change: move |value: String| {
                                        end_date.set(value);
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
                                            keyword.set(String::new());
                                            transaction_type.set("全部".into());
                                            park_filter.set(0);
                                            // 回到当月，而不是清空——清空得到的「不限区间」
                                            // 并不是进页面时的默认状态。
                                            start_date.set(default_range.0.clone());
                                            end_date.set(default_range.1.clone());
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
                    h2 { "收支流水台账" }
                    span { class: "hint", "账单是应收依据，财务流水是实际收支记录" }
                }
                if visible_rows.is_empty() {
                    p { class: "empty", "暂无符合条件的财务流水。调整筛选条件，或新增第一笔收入、支出记录。" }
                } else {
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "流水" }
                                    th { "园区" }
                                    th { "类型" }
                                    th { "分类" }
                                    th { "金额" }
                                    th { "交易日期" }
                                    th { "备注" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                for row in &visible_rows {
                                    {
                                        let edit = row.clone();
                                        let remove = row.clone();
                                        let view = row.clone();
                                        let park_name = park_names
                                            .get(&row.park_id)
                                            .cloned()
                                            .unwrap_or_else(|| "未分配园区".into());
                                        // 脱敏开关对表格同样生效——原来只有移动端卡片会遮
                                        let amount = if amounts_masked {
                                            masked_money(row.amount_cents)
                                        } else {
                                            format_money(row.amount_cents)
                                        };
                                        let is_income = row.transaction_type == "收入";
                                        rsx! {
                                            // 整行都是明细入口：表格放不下备注全文和凭证图片，
                                            // 而只把名称做成链接的话，点行内任何其他位置都没反应。
                                            tr {
                                                key: "finance-{row.finance_id}",
                                                class: "is-clickable",
                                                onclick: move |_| viewing.set(Some(view.clone())),
                                                td {
                                                    strong { "{row.bill_name}" }
                                                }
                                                td { "{park_name}" }
                                                td {
                                                    Badge {
                                                        variant: if is_income { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                                        "{row.transaction_type}"
                                                    }
                                                }
                                                td { "{row.bill_category}" }
                                                td { class: if is_income { "is-mono is-ok" } else { "is-mono" }, "{amount}" }
                                                td { class: "is-mono", "{format_date(row.transaction_time)}" }
                                                td { class: "is-wrap hint",
                                                    {row.remark.as_deref().unwrap_or("--")}
                                                }
                                                td {
                                                    // 操作列要拦掉冒泡，否则点编辑会连带打开明细
                                                    div {
                                                        class: "table-actions",
                                                        onclick: move |event| event.stop_propagation(),
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            size: ButtonSize::Sm,
                                                            onclick: move |_| {
                                                                editing.set(Some(edit.clone()));
                                                                form_open.set(true);
                                                            },
                                                            "编辑"
                                                        }
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            class: "is-quiet-danger",
                                                            size: ButtonSize::Sm,
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
                    Pager { page, total_pages: page_count, total_count: total }
                }
            }
        }

        if form_open() {
            FinanceFormDialog {
                record: editing(),
                images: form_images,
                parks: parks.clone(),
                on_close: move |_| form_open.set(false),
                on_saved: move |_| {
                    form_open.set(false);
                    notice.set(Some("财务流水与凭证已保存。".into()));
                },
            }
        }
        if let Some(record) = viewing() {
            {
                let images = finance_images
                    .iter()
                    .filter(|image| image.finance_id == record.finance_id)
                    .map(|image| image.url.clone())
                    .collect::<Vec<_>>();
                let park_name = park_names
                    .get(&record.park_id)
                    .cloned()
                    .unwrap_or_else(|| "未分配园区".into());
                rsx! {
                    FinanceDetailDialog {
                        record: record.clone(),
                        park_name,
                        images,
                        on_close: move |_| viewing.set(None),
                        on_edit: move |_| {
                            viewing.set(None);
                            editing.set(Some(record.clone()));
                            form_open.set(true);
                        },
                    }
                }
            }
        }
        if let Some(record) = deleting() {
            FinanceDeleteDialog {
                record,
                on_close: move |_| deleting.set(None),
                on_deleted: move |_| {
                    deleting.set(None);
                    notice.set(Some("财务流水已归档。".into()));
                },
            }
        }
    }
}

/// 财务流水明细：表格里放不下的备注全文和凭证图片在这里看。
#[component]
fn FinanceDetailDialog(
    record: Finance,
    park_name: String,
    images: Vec<String>,
    on_close: EventHandler<()>,
    on_edit: EventHandler<()>,
) -> Element {
    let is_income = record.transaction_type == "收入";
    let remark = record
        .remark
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "暂无备注".into());

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open {
                    on_close.call(());
                }
            },
            DialogTitle { "{record.bill_name}" }
            DialogDescription { "{park_name} · {record.bill_category}" }

            div { class: "stack",
                div { class: "row",
                    Badge {
                        variant: if is_income { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                        "{record.transaction_type}"
                    }
                    span { class: "hint is-mono", "FN-{record.finance_id:06}" }
                }

                div { class: "panel is-tight is-plain",
                    span { class: "stat-label", "金额" }
                    strong {
                        class: if is_income { "stat-value is-compact is-mono is-ok" } else { "stat-value is-compact is-mono" },
                        "{format_money(record.amount_cents)}"
                    }
                }

                dl { class: "facts",
                    div {
                        dt { "交易日期" }
                        dd { class: "is-mono", "{format_date(record.transaction_time)}" }
                    }
                    div {
                        dt { "所属园区" }
                        dd { "{park_name}" }
                    }
                    div {
                        dt { "账目分类" }
                        dd { "{record.bill_category}" }
                    }
                    div { class: "is-wide",
                        dt { "备注" }
                        dd { "{remark}" }
                    }
                }

                div { class: "subsection",
                    div { class: "section-header",
                        h4 { "凭证图片" }
                        Badge { variant: BadgeVariant::Outline, "{images.len()} 张" }
                    }
                    if images.is_empty() {
                        p { class: "empty", "该流水没有上传凭证" }
                    } else {
                        div { class: "grid-3",
                            for (index , url) in images.iter().enumerate() {
                                a {
                                    key: "finance-proof-{index}",
                                    class: "card-media",
                                    href: "{url}",
                                    target: "_blank",
                                    rel: "noreferrer",
                                    img { src: "{url}", alt: "凭证 {index + 1}", loading: "lazy" }
                                }
                            }
                        }
                    }
                }

                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        onclick: move |_| on_close.call(()),
                        "关闭"
                    }
                    Button { r#type: "button", onclick: move |_| on_edit.call(()), "编辑" }
                }
            }
        }
    }
}

#[component]
fn FinanceFormDialog(
    record: Option<Finance>,
    images: Vec<FinanceImage>,
    parks: Vec<Park>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let seed = record.clone();
    let mut bill_name = use_signal_sync(|| {
        seed.as_ref()
            .map(|row| row.bill_name.clone())
            .unwrap_or_default()
    });
    let mut category = use_signal_sync(|| {
        seed.as_ref()
            .map(|row| row.bill_category.clone())
            .unwrap_or_else(|| "其他费用".into())
    });
    let mut transaction_type = use_signal_sync(|| {
        seed.as_ref()
            .map(|row| row.transaction_type.clone())
            .unwrap_or_else(|| "支出".into())
    });
    let mut amount = use_signal_sync(|| {
        seed.as_ref()
            .map(|row| money_input(row.amount_cents))
            .unwrap_or_default()
    });
    let mut date = use_signal_sync(|| {
        seed.as_ref()
            .map(|row| format_date(row.transaction_time))
            .unwrap_or_else(today)
    });
    let mut park_id = use_signal_sync(|| seed.as_ref().map(|row| row.park_id).unwrap_or_default());
    let mut remark = use_signal_sync(|| {
        seed.as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    let mut loading = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let mut proofs = use_signal_sync(|| {
        images
            .into_iter()
            .map(|image| FinanceProof::Existing {
                id: image.id,
                url: image.url,
                removed: false,
            })
            .collect::<Vec<_>>()
    });
    let finance_id = record.as_ref().map(|row| row.finance_id);

    let category_value: ReadSignal<Option<String>> = use_memo(move || Some(category())).into();
    let type_value: ReadSignal<Option<String>> = use_memo(move || Some(transaction_type())).into();
    let park_value: ReadSignal<Option<String>> =
        use_memo(move || Some(park_id().to_string())).into();

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                // 上传进行中不允许关闭，否则请求还在飞就没人处理结果了。
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle {
                if finance_id.is_some() {
                    "修改收支"
                } else {
                    "新增收支"
                }
            }
            DialogDescription { "财务流水记录真实发生的收入或支出。" }

            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    let amount_cents = match parse_money(&amount()) {
                        Ok(value) => value,
                        Err(message) => {
                            error.set(Some(message));
                            return;
                        }
                    };
                    let transaction_time = match parse_date(&date()) {
                        Ok(value) => value,
                        Err(message) => {
                            error.set(Some(message));
                            return;
                        }
                    };
                    if bill_name().trim().is_empty() {
                        error.set(Some("请输入账目名称".into()));
                        return;
                    }
                    let input = FinanceInput {
                        bill_name: bill_name(),
                        bill_category: category(),
                        amount_cents,
                        transaction_type: transaction_type(),
                        transaction_time,
                        remark: (!remark().trim().is_empty()).then(|| remark()),
                        park_id: (park_id() != 0).then_some(park_id()),
                        status: 1,
                    };
                    let proof_values = proofs();
                    let mut image_urls = proof_values
                        .iter()
                        .filter_map(|proof| match proof {
                            FinanceProof::Existing { url, removed: false, .. } => Some(url.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let pending = proof_values
                        .into_iter()
                        .filter_map(|proof| match proof {
                            FinanceProof::Pending { file, .. } => Some(file),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    loading.set(true);
                    error.set(None);
                    spawn(async move {
                        for file in pending {
                            match upload_business_image(file).await {
                                Ok(image) => image_urls.push(image.public_url),
                                Err(message) => {
                                    loading.set(false);
                                    error.set(Some(message));
                                    return;
                                }
                            }
                        }
                        let result = if let Some(id) = finance_id {
                            update_finance_with_images_record(id, input, image_urls).await
                        } else {
                            create_finance_with_images_record(input, image_urls).await
                        };
                        match result {
                            Ok(()) => on_saved.call(()),
                            Err(message) => {
                                loading.set(false);
                                error.set(Some(message));
                            }
                        }
                    });
                },
                div { class: "form-grid",
                    div { class: "field is-wide",
                        Label { html_for: "finance-bill-name", "账目名称 *" }
                        Input {
                            id: "finance-bill-name",
                            value: bill_name(),
                            placeholder: "例如：7 月园区租金收入",
                            oninput: move |event: FormEvent| bill_name.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "finance-category", "账目分类 *" }
                        Select {
                            id: "finance-category",
                            value: Some(category_value),
                            on_value_change: move |value: Option<String>| {
                                category.set(value.unwrap_or_else(|| "其他费用".into()));
                            },
                            for (index , value) in [
                                    "账单收入",
                                    "水费",
                                    "电费",
                                    "燃气费",
                                    "报销支出",
                                    "维修费用",
                                    "采购费用",
                                    "其他费用",
                                ]
                                .into_iter()
                                .enumerate()
                            {
                                SelectOption::<String> {
                                    key: "category-{value}",
                                    value: value.to_string(),
                                    index,
                                    text_value: value.to_string(),
                                    "{value}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "finance-type-select", "收支类型 *" }
                        Select {
                            id: "finance-type-select",
                            value: Some(type_value),
                            on_value_change: move |value: Option<String>| {
                                transaction_type.set(value.unwrap_or_else(|| "支出".into()));
                            },
                            SelectOption::<String> { value: "收入".to_string(), index: 0usize, text_value: "收入".to_string(), "收入" }
                            SelectOption::<String> { value: "支出".to_string(), index: 1usize, text_value: "支出".to_string(), "支出" }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "finance-amount", "金额（元）*" }
                        Input {
                            id: "finance-amount",
                            inputmode: "decimal",
                            value: amount(),
                            placeholder: "0.00",
                            oninput: move |event: FormEvent| amount.set(event.value()),
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "交易日期 *" }
                        DateField {
                            value: date(),
                            disabled: loading(),
                            on_change: move |value: String| date.set(value),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "finance-park-select", "所属园区" }
                        Select {
                            id: "finance-park-select",
                            value: Some(park_value),
                            on_value_change: move |value: Option<String>| {
                                park_id.set(value.and_then(|value| value.parse().ok()).unwrap_or_default());
                            },
                            SelectOption::<String> { value: "0".to_string(), index: 0usize, text_value: "未分配园区".to_string(), "未分配园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "finance-form-park-{park.park_id}",
                                    value: park.park_id.to_string(),
                                    index: index + 1,
                                    text_value: park.park_name.to_string(),
                                    "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "finance-remark", "备注" }
                        Textarea {
                            id: "finance-remark",
                            maxlength: 300,
                            rows: 3,
                            value: remark(),
                            placeholder: "补充流水来源、用途或凭证说明",
                            oninput: move |event: FormEvent| remark.set(event.value()),
                        }
                    }
                    div { class: "field is-wide business-image-field",
                        div { class: "business-image-toolbar",
                            strong { "凭证图片" }
                            label { class: "business-image-upload",
                                input {
                                    r#type: "file",
                                    accept: "image/jpeg,image/png,image/webp",
                                    multiple: true,
                                    disabled: loading(),
                                    onchange: move |event| {
                                        for file in event.files() {
                                            if proofs().len() >= 8 {
                                                error.set(Some("每条财务流水最多上传 8 张凭证".into()));
                                                break;
                                            }
                                            match validate_business_image(&file) {
                                                Ok(()) => {
                                                    let name = file.name();
                                                    let preview_url = create_finance_preview(&file);
                                                    proofs
                                                        .write()
                                                        .push(FinanceProof::Pending {
                                                            file,
                                                            name,
                                                            preview_url,
                                                        });
                                                }
                                                Err(message) => {
                                                    error.set(Some(message));
                                                    break;
                                                }
                                            }
                                        }
                                    },
                                }
                                "选择凭证图片"
                            }
                        }
                        p { class: "hint", "支持 JPG、PNG、WebP，单张不超过 10MB，最多 8 张；确认保存后上传到 R2。" }
                        if proofs().is_empty() {
                            p { class: "business-image-empty", "暂无凭证图片" }
                        } else {
                            div { class: "business-image-list",
                                for (index , proof) in proofs().into_iter().enumerate() {
                                    {
                                        let url = proof.url().map(str::to_string);
                                        let name = proof.name();
                                        let removed = proof.removed();
                                        rsx! {
                                            div {
                                                key: "finance-proof-{index}-{name}",
                                                class: if removed { "is-removed" } else { "" },
                                                if let Some(url) = url {
                                                    img { src: "{url}", alt: "{name}" }
                                                } else {
                                                    div { class: "business-image-placeholder", "图片" }
                                                }
                                                span { "{name}" }
                                                if removed {
                                                    small { class: "is-removing", "确认后删除" }
                                                }
                                                button {
                                                    r#type: "button",
                                                    disabled: loading(),
                                                    onclick: move |_| {
                                                        if matches!(proofs().get(index), Some(FinanceProof::Pending { .. })) {
                                                            proofs.write().remove(index);
                                                        } else if let Some(FinanceProof::Existing { removed, .. }) = proofs
                                                            .write()
                                                            .get_mut(index)
                                                        {
                                                            *removed = !*removed;
                                                        }
                                                    },
                                                    if removed {
                                                        "撤销"
                                                    } else {
                                                        "移除"
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
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", "{message}" }
                }
                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        disabled: loading(),
                        onclick: move |_| on_close.call(()),
                        "取消"
                    }
                    Button {
                        r#type: "submit",
                        disabled: loading(),
                        if loading() {
                            "保存中…"
                        } else {
                            "确认保存"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FinanceDeleteDialog(
    record: Finance,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let finance_id = record.finance_id;
    let bill_name = record.bill_name.clone();
    let amount = format_money(record.amount_cents);

    rsx! {
        ConfirmDialog {
            title: "确认删除流水",
            description: format!(
                "将归档“{bill_name}”（{amount}），历史关联账单或报销不会被物理删除。",
            ),
            confirm_label: "确认删除",
            busy: loading(),
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() {
                    return;
                }
                loading.set(true);
                error.set(None);
                spawn(async move {
                    match delete_finance_record(finance_id).await {
                        Ok(()) => on_deleted.call(()),
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

fn masked_money(amount_cents: i64) -> String {
    let major = amount_cents.unsigned_abs() / 100;
    let decimal = amount_cents.unsigned_abs() % 100;
    let first = major.to_string().chars().next().unwrap_or('0');
    let sign = if amount_cents < 0 { "-" } else { "" };
    format!("{sign}¥ {first}*****.{decimal:02}")
}
