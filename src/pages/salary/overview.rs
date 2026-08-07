//! 工资管理实时台账页面。

use std::collections::BTreeMap;

use dioxus::prelude::*;

use super::{
    form::{SalaryDeleteDialog, SalaryFormDialog},
    format::{format_date, format_money, parse_date},
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        total_pages, DateField, Pager,
    },
    spacetime_bindings::salary_type::Salary,
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

#[derive(Clone, PartialEq)]
enum SalaryDialogState {
    Closed,
    Create,
    View(Salary),
    Edit(Salary),
    Delete(Salary),
}

fn proofs_for_salary(
    salary_id: u64,
    previews: &[crate::spacetime_bindings::salary_image_preview_type::SalaryImagePreview],
) -> Vec<crate::spacetime_bindings::salary_image_preview_type::SalaryImagePreview> {
    previews
        .iter()
        .filter(|image| image.salary_id == salary_id)
        .cloned()
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
pub fn SalaryManagementPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let employees = state.employees.read().clone();
    let salary_image_previews = state.salary_image_previews.read().clone();
    let can_manage = is_system_admin(state);
    let employee_map = employees
        .iter()
        .map(|employee| (employee.employee_id, employee.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut department_filter = use_signal(String::new);
    let mut name_filter = use_signal(String::new);
    let mut phone_filter = use_signal(String::new);
    let mut issued_filter = use_signal(String::new);
    // 服务端查询的回调要求 Send + Sync，这三个信号必须用 sync 存储。
    let mut history = use_signal_sync(|| {
        None::<crate::spacetime_bindings::salary_page_result_type::SalaryPageResult>
    });
    let mut history_loading = use_signal_sync(|| true);
    let mut history_error = use_signal_sync(|| None::<String>);
    let mut start_date = use_signal(String::new);
    let mut end_date = use_signal(String::new);
    let mut page = use_signal(|| 1usize);
    let mut dialog = use_signal(|| SalaryDialogState::Closed);

    let issued_value: ReadSignal<Option<String>> = use_memo(move || Some(issued_filter())).into();
    let start_micros = parse_date(&start_date())
        .ok()
        .map(|value| value.to_micros_since_unix_epoch());
    let end_micros = parse_date(&end_date())
        .ok()
        .map(|value| value.to_micros_since_unix_epoch() + 86_400_000_000 - 1);
    let name_keyword = name_filter().trim().to_lowercase();
    let phone_keyword = phone_filter().trim().to_string();
    let department_keyword = department_filter().trim().to_lowercase();
    let selected_issued = match issued_filter().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    };
    use_effect(move || {
        let _dialog_state = dialog();
        let query = crate::services::SalaryPageQuery {
            page: page() as u32,
            page_size: PAGE_SIZE as u32,
            name_keyword: name_filter(),
            phone_keyword: phone_filter(),
            issued: match issued_filter().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            department: department_filter(),
            start_time_micros: parse_date(&start_date())
                .ok()
                .map(|value| value.to_micros_since_unix_epoch()),
            end_time_micros: parse_date(&end_date())
                .ok()
                .map(|value| value.to_micros_since_unix_epoch() + 86_400_000_000 - 1),
        };
        history_loading.set(true);
        history_error.set(None);
        let result = crate::services::query_salary_history(query, move |result| match result {
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
    let salaries = history_snapshot
        .as_ref()
        .map(|value| value.rows.clone())
        .unwrap_or_default();
    let rows = salaries
        .iter()
        .filter(|salary| {
            let Some(employee) = employee_map.get(&salary.employee_id) else {
                return false;
            };
            let issue_micros = salary
                .issue_date
                .map(|value| value.to_micros_since_unix_epoch());
            (department_keyword.is_empty()
                || employee
                    .department
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&department_keyword)))
                && (name_keyword.is_empty()
                    || employee.name.to_lowercase().contains(&name_keyword))
                && (phone_keyword.is_empty() || employee.phone.contains(&phone_keyword))
                && selected_issued.is_none_or(|issued| salary.issued.unwrap_or(false) == issued)
                && start_micros.is_none_or(|start| issue_micros.is_some_and(|value| value >= start))
                && end_micros.is_none_or(|end| issue_micros.is_some_and(|value| value <= end))
        })
        .cloned()
        .collect::<Vec<_>>();
    let total = history_snapshot
        .as_ref()
        .map(|value| value.total)
        .unwrap_or_default() as usize;
    let paid_count = history_snapshot
        .as_ref()
        .map(|value| value.paid_count)
        .unwrap_or_default() as usize;
    let pending_count = history_snapshot
        .as_ref()
        .map(|value| value.pending_count)
        .unwrap_or_default() as usize;
    let paid_cents = history_snapshot
        .as_ref()
        .map(|value| value.paid_cents)
        .unwrap_or_default();
    // 分页由服务端按 page/page_size 返回，这里只负责算总页数。
    let page_count = total_pages(total, PAGE_SIZE);
    let visible_rows = rows.clone();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "工资管理" }
                    p { class: "page-subtitle",
                        "按员工管理工资金额、发放日期、状态与凭证，数据通过 SpacetimeDB 实时同步。"
                    }
                }
                if can_manage {
                    div { class: "page-actions",
                        Button {
                            r#type: "button",
                            onclick: move |_| dialog.set(SalaryDialogState::Create),
                            "新增工资记录"
                        }
                    }
                }
            }

            if history_loading() && history_snapshot.is_none() {
                p { class: "notice", role: "status", "正在加载工资记录…" }
            }
            if let Some(message) = history_error() {
                p { class: "form-error", role: "alert", "工资记录加载失败：{message}" }
            }

            section { class: "grid-4",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "工资记录" }
                                strong { class: "stat-value is-mono", "{total}" }
                                span { class: "stat-caption", "权限范围内全部记录" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已发放" }
                                strong { class: "stat-value is-mono is-ok", "{paid_count}" }
                                span { class: "stat-caption", "累计 {format_money(Some(paid_cents))}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "待发放" }
                                strong { class: "stat-value is-mono", "{pending_count}" }
                                span { class: "stat-caption", "需要继续处理" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "在册员工" }
                                strong { class: "stat-value is-mono", "{employees.len()}" }
                                span { class: "stat-caption", "可选择的租赁客户" }
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
                                Label { html_for: "salary-department", "部门" }
                                Input {
                                    id: "salary-department",
                                    value: department_filter,
                                    placeholder: "输入部门名称",
                                    oninput: move |event: FormEvent| {
                                        department_filter.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "salary-name", "员工姓名" }
                                Input {
                                    id: "salary-name",
                                    value: name_filter,
                                    placeholder: "输入员工姓名",
                                    oninput: move |event: FormEvent| {
                                        name_filter.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "salary-phone", "手机号" }
                                Input {
                                    id: "salary-phone",
                                    value: phone_filter,
                                    inputmode: "numeric",
                                    placeholder: "输入联系电话",
                                    oninput: move |event: FormEvent| {
                                        phone_filter.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "salary-issued", "发放状态" }
                                Select {
                                    id: "salary-issued",
                                    value: Some(issued_value),
                                    on_value_change: move |value: Option<String>| {
                                        issued_filter.set(value.unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    SelectOption::<String> { value: "true".to_string(), index: 1usize, text_value: "已发放".to_string(), "已发放" }
                                    SelectOption::<String> { value: "false".to_string(), index: 2usize, text_value: "待发放".to_string(), "待发放" }
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
                            div { class: "field",
                                Button {
                                    variant: ButtonVariant::Outline,
                                    r#type: "button",
                                    onclick: move |_| {
                                        department_filter.set(String::new());
                                        name_filter.set(String::new());
                                        phone_filter.set(String::new());
                                        issued_filter.set(String::new());
                                        start_date.set(String::new());
                                        end_date.set(String::new());
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
                    h2 { "工资列表" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 条" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "员工" }
                                th { "手机号" }
                                th { "工资金额" }
                                th { "发放日期" }
                                th { "状态" }
                                th { "凭证" }
                                th { "备注" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if visible_rows.is_empty() {
                                tr {
                                    td { colspan: "8", class: "table-empty", "暂无符合条件的工资记录" }
                                }
                            }
                            for row in visible_rows.iter() {
                                {
                                    let employee = employee_map.get(&row.employee_id);
                                    let tenant_name = employee
                                        .map(|value| value.name.as_str())
                                        .unwrap_or("员工已移除");
                                    let phone = employee.map(|value| value.phone.as_str()).unwrap_or("--");
                                    let proofs = proofs_for_salary(row.salary_id, &salary_image_previews);
                                    let issued = row.issued.unwrap_or(false);
                                    let view_row = row.clone();
                                    let edit_row = row.clone();
                                    let delete_row = row.clone();
                                    rsx! {
                                        tr { key: "salary-{row.salary_id}",
                                            td {
                                                strong { "{tenant_name}" }
                                            }
                                            td { class: "is-mono", "{phone}" }
                                            td { class: "is-mono", "{format_money(row.salary_amount_cents)}" }
                                            td { class: "is-mono", "{format_date(row.issue_date)}" }
                                            td {
                                                Badge {
                                                    variant: if issued { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                                    if issued {
                                                        "已发放"
                                                    } else {
                                                        "待发放"
                                                    }
                                                }
                                            }
                                            td { class: "is-mono",
                                                if proofs.is_empty() {
                                                    "--"
                                                } else {
                                                    "{proofs.len()} 张"
                                                }
                                            }
                                            td { class: "is-wrap hint",
                                                {row.remark.as_deref().unwrap_or("--")}
                                            }
                                            td {
                                                div { class: "table-actions",
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: move |_| dialog.set(SalaryDialogState::View(view_row.clone())),
                                                        "查看"
                                                    }
                                                    if can_manage {
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            size: ButtonSize::Sm,
                                                            r#type: "button",
                                                            onclick: move |_| dialog.set(SalaryDialogState::Edit(edit_row.clone())),
                                                            "编辑"
                                                        }
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            class: "is-quiet-danger",
                                                            size: ButtonSize::Sm,
                                                            r#type: "button",
                                                            onclick: move |_| dialog.set(SalaryDialogState::Delete(delete_row.clone())),
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

        match dialog() {
            SalaryDialogState::Closed => rsx! {},
            SalaryDialogState::Create => rsx! { SalaryFormDialog { salary: None, employees: employees.clone(), image_previews: Vec::new(), readonly: false, on_close: move |_| dialog.set(SalaryDialogState::Closed), on_saved: move |_| dialog.set(SalaryDialogState::Closed) } },
            SalaryDialogState::View(row) => { let proofs = proofs_for_salary(row.salary_id, &salary_image_previews); rsx! { SalaryFormDialog { salary: Some(row), employees: employees.clone(), image_previews: proofs, readonly: true, on_close: move |_| dialog.set(SalaryDialogState::Closed), on_saved: move |_| dialog.set(SalaryDialogState::Closed) } } },
            SalaryDialogState::Edit(row) => { let proofs = proofs_for_salary(row.salary_id, &salary_image_previews); rsx! { SalaryFormDialog { salary: Some(row), employees: employees.clone(), image_previews: proofs, readonly: false, on_close: move |_| dialog.set(SalaryDialogState::Closed), on_saved: move |_| dialog.set(SalaryDialogState::Closed) } } },
            SalaryDialogState::Delete(row) => {
                let tenant_name = employee_map.get(&row.employee_id).map(|employee| employee.name.clone()).unwrap_or_else(|| "该员工".into());
                let images = proofs_for_salary(row.salary_id, &salary_image_previews).into_iter().map(|image| crate::services::StoredR2Image {
                    img_id: image.img_id,
                    public_url: image.img_url,
                }).collect::<Vec<_>>();
                rsx! { SalaryDeleteDialog { salary: row, tenant_name, images, on_close: move |_| dialog.set(SalaryDialogState::Closed), on_deleted: move |_| dialog.set(SalaryDialogState::Closed) } }
            },
        }
    }
}
