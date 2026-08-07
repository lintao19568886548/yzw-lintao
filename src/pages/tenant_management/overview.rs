//! 租户主档台账与合同主体同步页面。

use std::collections::{BTreeSet, HashMap};

use dioxus::prelude::*;

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
        total_pages, ConfirmDialog, Pager,
    },
    services::{
        create_tenant_profile_record, delete_tenant_profile_record, sync_tenant_profiles_record,
        update_tenant_profile_record,
    },
    spacetime_bindings::{
        rental_tenant_type::RentalTenant, tenant_profile_input_type::TenantProfileInput,
        tenant_profile_type::TenantProfile,
    },
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

#[derive(Clone, PartialEq)]
enum TenantDialogState {
    Closed,
    Create,
    /// 从未建档的合同主体带资料建档，省掉重新抄一遍名称和电话。
    CreateFrom(UnfiledParty),
    Edit(TenantProfile),
}

/// 历史合同里出现过、但租户主档还没有对应记录的交易主体。
#[derive(Clone, PartialEq)]
struct UnfiledParty {
    tenant_name: String,
    phone_number: String,
    contract_count: usize,
    parks: Vec<String>,
}

#[derive(Clone)]
struct TenantDraft {
    tenant_name: String,
    tenant_type: String,
    unified_social_credit_code: String,
    legal_representative: String,
    contact_name: String,
    phone_number: String,
    email: String,
    address: String,
    source: String,
    status: i8,
    risk_level: String,
    remark: String,
}

impl Default for TenantDraft {
    fn default() -> Self {
        Self {
            tenant_name: String::new(),
            tenant_type: "enterprise".into(),
            unified_social_credit_code: String::new(),
            legal_representative: String::new(),
            contact_name: String::new(),
            phone_number: String::new(),
            email: String::new(),
            address: String::new(),
            source: "manual".into(),
            status: 1,
            risk_level: "normal".into(),
            remark: String::new(),
        }
    }
}

impl TenantDraft {
    fn from_profile(row: &TenantProfile) -> Self {
        Self {
            tenant_name: row.tenant_name.clone(),
            tenant_type: row.tenant_type.clone(),
            unified_social_credit_code: row.unified_social_credit_code.clone().unwrap_or_default(),
            legal_representative: row.legal_representative.clone().unwrap_or_default(),
            contact_name: row.contact_name.clone(),
            phone_number: row.phone_number.clone(),
            email: row.email.clone().unwrap_or_default(),
            address: row.address.clone().unwrap_or_default(),
            source: row.source.clone().unwrap_or_else(|| "manual".into()),
            status: row.status,
            risk_level: row.risk_level.clone(),
            remark: row.remark.clone().unwrap_or_default(),
        }
    }

    fn into_input(self) -> TenantProfileInput {
        TenantProfileInput {
            tenant_name: self.tenant_name,
            tenant_type: self.tenant_type,
            unified_social_credit_code: optional(self.unified_social_credit_code),
            legal_representative: optional(self.legal_representative),
            contact_name: self.contact_name,
            phone_number: self.phone_number,
            email: optional(self.email),
            address: optional(self.address),
            source: optional(self.source),
            status: self.status,
            risk_level: self.risk_level,
            remark: optional(self.remark),
        }
    }
}

#[component]
pub fn TenantManagementPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let profiles = (state.tenant_profiles)();
    let contracts = (state.rental_tenants)();
    let parks = (state.parks)();
    let mut keyword = use_signal(String::new);
    let mut type_filter = use_signal(|| "all".to_string());
    let mut status_filter = use_signal(|| "all".to_string());
    let mut dialog = use_signal(|| TenantDialogState::Closed);
    let mut notice = use_signal(|| None::<String>);
    let mut syncing = use_signal(|| false);
    let mut page = use_signal(|| 1usize);
    let mut show_unfiled = use_signal(|| false);
    let type_value: ReadSignal<Option<String>> = use_memo(move || Some(type_filter())).into();
    let status_value: ReadSignal<Option<String>> = use_memo(move || Some(status_filter())).into();

    let park_names = parks
        .iter()
        .map(|park| (park.park_id, park.park_name.clone()))
        .collect::<HashMap<_, _>>();
    let profile_keys = profiles
        .iter()
        .map(|row| party_key(&row.tenant_name, &row.phone_number))
        .collect::<BTreeSet<_>>();
    let unfiled = unfiled_parties(&contracts, &profile_keys, &park_names);
    let unfiled_count = unfiled.len();
    let active_count = profiles.iter().filter(|row| row.status == 1).count();
    let enterprise_count = profiles
        .iter()
        .filter(|row| row.tenant_type == "enterprise")
        .count();
    let risk_count = profiles
        .iter()
        .filter(|row| row.risk_level != "normal")
        .count();

    let query = keyword().trim().to_lowercase();
    let selected_type = type_filter();
    let selected_status = status_filter();
    let filtered = profiles
        .iter()
        .filter(|row| {
            (query.is_empty()
                || row.tenant_name.to_lowercase().contains(&query)
                || row.contact_name.to_lowercase().contains(&query)
                || row.phone_number.contains(&query)
                || row
                    .unified_social_credit_code
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&query)))
                && (selected_type == "all" || row.tenant_type == selected_type)
                && (selected_status == "all"
                    || (selected_status == "active" && row.status == 1)
                    || (selected_status == "inactive" && row.status == 0))
        })
        .cloned()
        .collect::<Vec<_>>();
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
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "租户管理" }
                    p { class: "page-subtitle",
                        "将企业与个人租户沉淀为独立主档，合同只记录交易关系，不再承担客户档案职责。"
                    }
                }
                div { class: "page-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: syncing(),
                        onclick: move |_| {
                            syncing.set(true);
                            notice.set(Some("正在从历史合同识别未建档主体…".into()));
                            spawn(async move {
                                match sync_tenant_profiles_record().await {
                                    Ok(()) => notice.set(Some("合同主体同步完成，租户主档已更新。".into())),
                                    Err(message) => notice.set(Some(message)),
                                }
                                syncing.set(false);
                            });
                        },
                        if syncing() {
                            "正在同步"
                        } else {
                            "同步合同主体"
                        }
                    }
                    Button { onclick: move |_| dialog.set(TenantDialogState::Create), "新增租户" }
                }
            }

            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-auto",
                TenantMetric { label: "租户主档", value: profiles.len(), detail: "独立业务主体" }
                TenantMetric { label: "合作中", value: active_count, detail: "状态正常" }
                TenantMetric { label: "企业客户", value: enterprise_count, detail: "企业主体" }
                TenantMetric { label: "风险关注", value: risk_count, detail: "需人工跟进" }
                // 待建档是唯一需要动手的指标，做成可点击：直接列出是哪几个主体，
                // 否则只看到一个数字，用户不知道该对谁建档。
                TenantMetric {
                    label: "待建档",
                    value: unfiled_count,
                    detail: if unfiled_count > 0 { "点击查看明细" } else { "来自历史合同" },
                    onclick: (unfiled_count > 0)
                        .then_some(EventHandler::new(move |_| show_unfiled.toggle())),
                    active: show_unfiled(),
                }
            }

            if show_unfiled() && !unfiled.is_empty() {
                section { class: "section",
                    Card {
                        CardContent {
                            div { class: "stack",
                                div { class: "section-header",
                                    h2 { "待建档主体" }
                                    span { class: "hint",
                                        "这些主体在历史合同里出现过，但还没有对应的租户主档"
                                    }
                                }
                                div { class: "table-shell",
                                    table { class: "table",
                                        thead {
                                            tr {
                                                th { "主体名称" }
                                                th { "联系电话" }
                                                th { "合同" }
                                                th { "涉及园区" }
                                                th { "操作" }
                                            }
                                        }
                                        tbody {
                                            for party in unfiled.iter() {
                                                {
                                                    let target = party.clone();
                                                    let parks_label = if party.parks.is_empty() {
                                                        "—".to_string()
                                                    } else {
                                                        party.parks.join("、")
                                                    };
                                                    rsx! {
                                                        tr { key: "unfiled-{party.tenant_name}-{party.phone_number}",
                                                            td {
                                                                strong { "{party.tenant_name}" }
                                                            }
                                                            td { class: "is-mono", "{party.phone_number}" }
                                                            td { class: "is-mono", "{party.contract_count}" }
                                                            td { class: "is-wrap", "{parks_label}" }
                                                            td {
                                                                div { class: "table-actions",
                                                                    Button {
                                                                        size: ButtonSize::Sm,
                                                                        onclick: move |_| dialog.set(TenantDialogState::CreateFrom(target.clone())),
                                                                        "建档"
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
            }

            section { class: "section",
                Card {
                    CardContent {
                        div { class: "filters",
                            div { class: "field",
                                Label { html_for: "tenant-keyword", "搜索租户" }
                                Input {
                                    id: "tenant-keyword",
                                    value: keyword,
                                    placeholder: "名称、联系人、电话或信用代码",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "tenant-type", "主体类型" }
                                Select {
                                    id: "tenant-type",
                                    value: Some(type_value),
                                    on_value_change: move |value: Option<String>| {
                                        type_filter.set(value.unwrap_or_else(|| "all".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部类型".to_string(), "全部类型" }
                                    SelectOption::<String> { value: "enterprise".to_string(), index: 1usize, text_value: "企业".to_string(), "企业" }
                                    SelectOption::<String> { value: "individual".to_string(), index: 2usize, text_value: "个人".to_string(), "个人" }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "tenant-status", "合作状态" }
                                Select {
                                    id: "tenant-status",
                                    value: Some(status_value),
                                    on_value_change: move |value: Option<String>| {
                                        status_filter.set(value.unwrap_or_else(|| "all".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    SelectOption::<String> { value: "active".to_string(), index: 1usize, text_value: "合作中".to_string(), "合作中" }
                                    SelectOption::<String> { value: "inactive".to_string(), index: 2usize, text_value: "已停用".to_string(), "已停用" }
                                }
                            }
                            div { class: "field is-action",
                                Button {
                                    variant: ButtonVariant::Outline,
                                    onclick: move |_| {
                                        keyword.set(String::new());
                                        type_filter.set("all".into());
                                        status_filter.set("all".into());
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
                    h2 { "租户主档台账" }
                    span { class: "hint", "主档只保存主体身份，租期与租金由合同管理维护" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "租户" }
                                th { "主体" }
                                th { "主要联系人" }
                                th { "关联园区" }
                                th { "合同" }
                                th { "风险" }
                                th { "状态" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if visible.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "8", "暂无符合条件的租户档案" }
                                }
                            }
                            for row in visible.iter() {
                                {tenant_table_row(row, &contracts, &park_names, dialog)}
                            }
                        }
                    }
                }
                Pager { page, total_pages: page_count, total_count: total }
            }
        }

        match dialog() {
            TenantDialogState::Closed => rsx! {},
            TenantDialogState::Create => rsx! {
                TenantProfileDialog { profile: None, on_close: move |_| dialog.set(TenantDialogState::Closed) }
            },
            TenantDialogState::CreateFrom(party) => rsx! {
                TenantProfileDialog {
                    profile: None,
                    prefill: Some(party),
                    on_close: move |_| dialog.set(TenantDialogState::Closed),
                }
            },
            TenantDialogState::Edit(profile) => rsx! {
                TenantProfileDialog {
                    profile: Some(profile),
                    on_close: move |_| dialog.set(TenantDialogState::Closed),
                }
            },
        }
    }
}

#[component]
fn TenantMetric(
    label: String,
    value: usize,
    detail: String,
    /// 有值时整块指标变成按钮；没有就是普通展示卡。
    #[props(default)]
    onclick: Option<EventHandler<()>>,
    #[props(default)] active: bool,
) -> Element {
    let body = rsx! {
        div { class: "stat",
            span { class: "stat-label", "{label}" }
            strong { class: "stat-value is-mono", "{value}" }
            span { class: "stat-caption", "{detail}" }
        }
    };

    match onclick {
        Some(handler) => rsx! {
            button {
                class: if active { "metric-button is-active" } else { "metric-button" },
                r#type: "button",
                aria_pressed: active,
                onclick: move |_| handler.call(()),
                {body}
            }
        },
        None => rsx! {
            div {
                Card {
                    CardContent { {body} }
                }
            }
        },
    }
}

/// 风险等级到徽章：正常不着色，关注和高风险逐级加重。
fn risk_variant(value: &str) -> BadgeVariant {
    match value {
        "high" => BadgeVariant::Destructive,
        "watch" => BadgeVariant::Secondary,
        _ => BadgeVariant::Outline,
    }
}

fn tenant_table_row(
    row: &TenantProfile,
    contracts: &[RentalTenant],
    park_names: &HashMap<u64, String>,
    mut dialog: Signal<TenantDialogState>,
) -> Element {
    let contract_rows = matching_contracts(row, contracts);
    let parks = related_parks(&contract_rows, park_names);
    let profile = row.clone();
    let contact_name = if row.contact_name.is_empty() {
        "未填写联系人"
    } else {
        row.contact_name.as_str()
    };
    let tenant_type_label = if row.tenant_type == "enterprise" {
        "企业"
    } else {
        "个人"
    };
    let parks_label = if parks.is_empty() {
        "尚未关联合同".to_string()
    } else {
        parks.join("、")
    };
    let active = row.status == 1;
    rsx! {
        tr { key: "tenant-profile-{row.tenant_profile_id}",
            td {
                div { class: "stack-tight",
                    strong { "{row.tenant_name}" }
                    small { class: "hint is-mono", "TP-{row.tenant_profile_id:06}" }
                }
            }
            td { "{tenant_type_label}" }
            td {
                div { class: "stack-tight",
                    span { "{contact_name}" }
                    small { class: "hint is-mono", "{row.phone_number}" }
                }
            }
            td { class: "is-wrap", "{parks_label}" }
            td { class: "is-mono", "{contract_rows.len()}" }
            td {
                Badge { variant: risk_variant(&row.risk_level), "{risk_label(&row.risk_level)}" }
            }
            td {
                Badge {
                    variant: if active { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                    if active {
                        "合作中"
                    } else {
                        "已停用"
                    }
                }
            }
            td {
                div { class: "table-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        onclick: move |_| dialog.set(TenantDialogState::Edit(profile.clone())),
                        "管理档案"
                    }
                }
            }
        }
    }
}

#[component]
fn TenantProfileDialog(
    profile: Option<TenantProfile>,
    /// 从未建档的合同主体带过来的名称与电话。
    #[props(default)]
    prefill: Option<UnfiledParty>,
    on_close: EventHandler<()>,
) -> Element {
    let editing = profile.is_some();
    let profile_id = profile.as_ref().map(|row| row.tenant_profile_id);
    let initial = profile
        .as_ref()
        .map(TenantDraft::from_profile)
        .unwrap_or_else(|| match prefill.as_ref() {
            Some(party) => TenantDraft {
                tenant_name: party.tenant_name.clone(),
                phone_number: party.phone_number.clone(),
                // 来源标成合同同步，事后能看出这条主档是从历史合同补建的。
                source: "contract_sync".into(),
                ..TenantDraft::default()
            },
            None => TenantDraft::default(),
        });
    let mut draft = use_signal(|| initial);
    let mut saving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut confirm_delete = use_signal(|| false);
    let type_value: ReadSignal<Option<String>> = use_memo(move || Some(draft().tenant_type)).into();
    let status_value: ReadSignal<Option<String>> =
        use_memo(move || Some(draft().status.to_string())).into();
    let source_value: ReadSignal<Option<String>> = use_memo(move || Some(draft().source)).into();
    let risk_value: ReadSignal<Option<String>> = use_memo(move || Some(draft().risk_level)).into();

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                // 保存过程中不允许关闭，否则请求还在飞就没人处理结果了。
                if !open && !saving() {
                    on_close.call(());
                }
            },
            DialogTitle {
                if editing {
                    "维护租户主档"
                } else {
                    "新增租户主档"
                }
            }
            DialogDescription { "主档只保存主体身份和主要联系人，租期与租金仍由合同管理维护。" }

            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    if saving() {
                        return;
                    }
                    let value = draft();
                    if value.tenant_name.trim().is_empty() || value.phone_number.trim().is_empty() {
                        error.set(Some("请填写租户名称和联系电话".into()));
                        return;
                    }
                    saving.set(true);
                    error.set(None);
                    spawn(async move {
                        let result = if let Some(id) = profile_id {
                            update_tenant_profile_record(id, value.into_input()).await
                        } else {
                            create_tenant_profile_record(value.into_input()).await
                        };
                        match result {
                            Ok(()) => on_close.call(()),
                            Err(message) => error.set(Some(message)),
                        }
                        saving.set(false);
                    });
                },
                div { class: "form-grid",
                    div { class: "field is-wide",
                        Label { html_for: "tenant-name", "租户名称 *" }
                        Input {
                            id: "tenant-name",
                            value: draft().tenant_name,
                            placeholder: "企业全称或个人姓名",
                            oninput: move |event: FormEvent| draft.write().tenant_name = event.value(),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "tenant-form-type", "主体类型" }
                        Select {
                            id: "tenant-form-type",
                            value: Some(type_value),
                            on_value_change: move |value: Option<String>| {
                                draft.write().tenant_type = value.unwrap_or_else(|| "enterprise".into());
                            },
                            SelectOption::<String> { value: "enterprise".to_string(), index: 0usize, text_value: "企业".to_string(), "企业" }
                            SelectOption::<String> { value: "individual".to_string(), index: 1usize, text_value: "个人".to_string(), "个人" }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "tenant-form-status", "合作状态" }
                        Select {
                            id: "tenant-form-status",
                            value: Some(status_value),
                            on_value_change: move |value: Option<String>| {
                                draft.write().status = value
                                    .and_then(|value| value.parse().ok())
                                    .unwrap_or(1);
                            },
                            SelectOption::<String> { value: "1".to_string(), index: 0usize, text_value: "合作中".to_string(), "合作中" }
                            SelectOption::<String> { value: "0".to_string(), index: 1usize, text_value: "已停用".to_string(), "已停用" }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "tenant-credit", "统一社会信用代码" }
                        Input {
                            id: "tenant-credit",
                            value: draft().unified_social_credit_code,
                            placeholder: "企业主体选填",
                            oninput: move |event: FormEvent| {
                                draft.write().unified_social_credit_code = event.value()
                            },
                        }
                    }
                    div { class: "field",
                        Label { html_for: "tenant-legal", "法定代表人" }
                        Input {
                            id: "tenant-legal",
                            value: draft().legal_representative,
                            placeholder: "企业主体选填",
                            oninput: move |event: FormEvent| {
                                draft.write().legal_representative = event.value()
                            },
                        }
                    }
                    div { class: "field",
                        Label { html_for: "tenant-contact", "主要联系人" }
                        Input {
                            id: "tenant-contact",
                            value: draft().contact_name,
                            placeholder: "业务联系人姓名",
                            oninput: move |event: FormEvent| draft.write().contact_name = event.value(),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "tenant-phone", "联系电话 *" }
                        Input {
                            id: "tenant-phone",
                            value: draft().phone_number,
                            inputmode: "tel",
                            placeholder: "手机或座机",
                            oninput: move |event: FormEvent| draft.write().phone_number = event.value(),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "tenant-email", "电子邮箱" }
                        Input {
                            id: "tenant-email",
                            value: draft().email,
                            inputmode: "email",
                            placeholder: "name@company.com",
                            oninput: move |event: FormEvent| draft.write().email = event.value(),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "tenant-source", "客户来源" }
                        Select {
                            id: "tenant-source",
                            value: Some(source_value),
                            on_value_change: move |value: Option<String>| {
                                draft.write().source = value.unwrap_or_else(|| "manual".into());
                            },
                            SelectOption::<String> { value: "manual".to_string(), index: 0usize, text_value: "人工建档".to_string(), "人工建档" }
                            SelectOption::<String> { value: "contract_sync".to_string(), index: 1usize, text_value: "合同同步".to_string(), "合同同步" }
                            SelectOption::<String> { value: "crm".to_string(), index: 2usize, text_value: "招商转化".to_string(), "招商转化" }
                            SelectOption::<String> { value: "referral".to_string(), index: 3usize, text_value: "客户推荐".to_string(), "客户推荐" }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "tenant-risk", "风险等级" }
                        Select {
                            id: "tenant-risk",
                            value: Some(risk_value),
                            on_value_change: move |value: Option<String>| {
                                draft.write().risk_level = value.unwrap_or_else(|| "normal".into());
                            },
                            SelectOption::<String> { value: "normal".to_string(), index: 0usize, text_value: "正常".to_string(), "正常" }
                            SelectOption::<String> { value: "watch".to_string(), index: 1usize, text_value: "关注".to_string(), "关注" }
                            SelectOption::<String> { value: "high".to_string(), index: 2usize, text_value: "高风险".to_string(), "高风险" }
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "tenant-address", "联系地址" }
                        Input {
                            id: "tenant-address",
                            value: draft().address,
                            placeholder: "注册地址或常用联系地址",
                            oninput: move |event: FormEvent| draft.write().address = event.value(),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "tenant-remark", "备注" }
                        Textarea {
                            id: "tenant-remark",
                            value: draft().remark,
                            maxlength: 500,
                            rows: 3,
                            placeholder: "补充经营情况、沟通偏好或风险说明",
                            oninput: move |event: FormEvent| draft.write().remark = event.value(),
                        }
                    }
                }

                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", "{message}" }
                }

                div { class: "form-actions",
                    // 停用留在最左侧，和右侧的取消/保存拉开距离，避免误触
                    if editing {
                        Button {
                            variant: ButtonVariant::Outline,
                            class: "is-quiet-danger",
                            r#type: "button",
                            disabled: saving(),
                            onclick: move |_| confirm_delete.set(true),
                            "停用档案"
                        }
                    }
                    span { class: "spacer" }
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        disabled: saving(),
                        onclick: move |_| on_close.call(()),
                        "取消"
                    }
                    Button {
                        r#type: "submit",
                        disabled: saving(),
                        if saving() {
                            "正在保存"
                        } else {
                            "确认保存"
                        }
                    }
                }
            }
        }

        if confirm_delete() {
            ConfirmDialog {
                title: "停用租户档案？",
                description: "合同和账单历史不会删除，主档将从租户台账隐藏。",
                confirm_label: "确认停用",
                busy: saving(),
                error: error(),
                on_cancel: move |_| confirm_delete.set(false),
                on_confirm: move |_| {
                    let Some(id) = profile_id else { return };
                    saving.set(true);
                    error.set(None);
                    spawn(async move {
                        match delete_tenant_profile_record(id).await {
                            Ok(()) => on_close.call(()),
                            Err(message) => error.set(Some(message)),
                        }
                        saving.set(false);
                    });
                },
            }
        }
    }
}

/// 汇总历史合同里尚未建档的交易主体。
///
/// 同一主体可能签过多份合同，按「名称 + 电话」归并，顺带带出合同份数和
/// 涉及园区，建档时不用再去合同列表里翻。
fn unfiled_parties(
    contracts: &[RentalTenant],
    profile_keys: &BTreeSet<String>,
    park_names: &HashMap<u64, String>,
) -> Vec<UnfiledParty> {
    let mut grouped = std::collections::BTreeMap::<String, UnfiledParty>::new();
    for row in contracts.iter().filter(|row| !row.is_deleted) {
        let key = party_key(&row.tenant_name, &row.phone_number);
        if profile_keys.contains(&key) {
            continue;
        }
        let entry = grouped.entry(key).or_insert_with(|| UnfiledParty {
            tenant_name: row.tenant_name.clone(),
            phone_number: row.phone_number.clone(),
            contract_count: 0,
            parks: Vec::new(),
        });
        entry.contract_count += 1;
        if let Some(park) = park_names.get(&row.park_id) {
            if !entry.parks.contains(park) {
                entry.parks.push(park.clone());
            }
        }
    }
    let mut rows = grouped.into_values().collect::<Vec<_>>();
    // 合同多的主体优先建档，影响面更大。
    rows.sort_by(|left, right| {
        right
            .contract_count
            .cmp(&left.contract_count)
            .then_with(|| left.tenant_name.cmp(&right.tenant_name))
    });
    rows
}

fn matching_contracts<'a>(
    profile: &TenantProfile,
    contracts: &'a [RentalTenant],
) -> Vec<&'a RentalTenant> {
    let key = party_key(&profile.tenant_name, &profile.phone_number);
    contracts
        .iter()
        .filter(|row| !row.is_deleted && party_key(&row.tenant_name, &row.phone_number) == key)
        .collect()
}

fn related_parks(contracts: &[&RentalTenant], park_names: &HashMap<u64, String>) -> Vec<String> {
    contracts
        .iter()
        .filter_map(|row| park_names.get(&row.park_id).cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// 客户档案和合同之间唯一的关联口径——姓名+电话的文本匹配，不是外键。
/// 数据库里 `RentalTenant`（合同）没有指向 `TenantProfile`（客户档案）的
/// 字段，两张表能对上纯粹靠这个约定；改这个函数就是改了两张表之间
/// "谁算同一个客户"的判定标准，所以整个仓库只有这一处实现，别处引用它。
#[pure_function::pure]
pub(crate) fn party_key(name: &str, phone: &str) -> String {
    format!("{}|{}", name.trim().to_lowercase(), phone.trim())
}

fn optional(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn risk_label(value: &str) -> &'static str {
    match value {
        "watch" => "需关注",
        "high" => "高风险",
        _ => "正常",
    }
}
