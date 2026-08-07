//! 员工花名册与档案管理。

use dioxus::prelude::*;

use super::{
    employee_form::EmployeeFormDialog,
    model::{format_date, seconds_to_time},
    navigation::HrmNavigation,
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
    permissions::{can_manage_hr, can_manage_permissions},
    services::delete_employee_record,
    spacetime_bindings::employee_type::Employee,
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

#[component]
pub fn HrmEmployeePage() -> Element {
    let state = use_context::<WorkspaceState>();
    let employees = (state.employees)();
    let options = (state.employee_user_options)();
    let current_roles = (state.roles)();
    let current_menus = (state.menus)();
    let role_options = (state.permission_roles)();
    let admin = can_manage_hr(&current_roles, &(state.permission_codes)());
    let can_assign_roles = can_manage_permissions(&current_roles, &current_menus);
    let mut keyword = use_signal(String::new);
    let mut status = use_signal(|| "active".to_string());
    let mut selected = use_signal(|| None::<Employee>);
    let mut form_open = use_signal(|| false);
    let mut deleting = use_signal(|| None::<Employee>);
    let mut notice = use_signal(|| None::<String>);
    let mut page = use_signal(|| 1usize);
    let status_value: ReadSignal<Option<String>> = use_memo(move || Some(status())).into();

    let active = employees.iter().filter(|r| !r.is_resigned).count();
    let resigned = employees.len().saturating_sub(active);
    let bound = employees.iter().filter(|r| r.user_id.is_some()).count();
    let query = keyword().trim().to_lowercase();
    let filtered = employees
        .iter()
        .filter(|r| {
            (query.is_empty()
                || r.name.to_lowercase().contains(&query)
                || r.phone.contains(&query)
                || r.department
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&query))
                && (status() == "all"
                    || (status() == "active" && !r.is_resigned)
                    || (status() == "resigned" && r.is_resigned))
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

    if !admin {
        return rsx! {
            main { class: "page",
                HrmNavigation { active: String::from("employee") }
                Card {
                    CardContent {
                        div { class: "stack",
                            h1 { "员工档案仅向人事管理角色开放" }
                            p { class: "page-subtitle",
                                "普通员工仍可使用考勤打卡、查看本人考勤与轨迹，并提交自己的请假申请。"
                            }
                            div { class: "card-cta",
                                Link { to: crate::router::Route::HrmAttendancePunchPage {},
                                    Button { variant: ButtonVariant::Outline, "进入考勤打卡" }
                                }
                            }
                        }
                    }
                }
            }
        };
    }

    rsx! {
        main { class: "page",
            HrmNavigation { active: String::from("employee") }

            header { class: "page-header",
                div { class: "page-title",
                    h1 { "员工信息" }
                    p { class: "page-subtitle",
                        "以员工档案为中心，关联业务账号、标准班次、考勤和请假记录。"
                    }
                }
                div { class: "page-actions",
                    Button {
                        onclick: move |_| {
                            selected.set(None);
                            form_open.set(true);
                        },
                        "新增员工"
                    }
                }
            }

            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-4",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "员工档案" }
                                strong { class: "stat-value is-mono", "{employees.len()}" }
                                span { class: "stat-caption", "全部记录" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "在职" }
                                strong { class: "stat-value is-mono is-ok", "{active}" }
                                span { class: "stat-caption", "未标记离职" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已绑定账号" }
                                strong { class: "stat-value is-mono", "{bound}" }
                                span { class: "stat-caption", "可登录系统" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已离职" }
                                strong { class: "stat-value is-mono", "{resigned}" }
                                span { class: "stat-caption", "档案保留" }
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
                                Label { html_for: "employee-keyword", "搜索人员" }
                                Input {
                                    id: "employee-keyword",
                                    value: keyword,
                                    placeholder: "姓名、手机号或部门",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "employee-status", "人员状态" }
                                Select {
                                    id: "employee-status",
                                    value: Some(status_value),
                                    on_value_change: move |value: Option<String>| {
                                        status.set(value.unwrap_or_else(|| "active".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部".to_string(), "全部" }
                                    SelectOption::<String> { value: "active".to_string(), index: 1usize, text_value: "在职".to_string(), "在职" }
                                    SelectOption::<String> { value: "resigned".to_string(), index: 2usize, text_value: "离职".to_string(), "离职" }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "员工花名册" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 人" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "姓名" }
                                th { "部门" }
                                th { "状态" }
                                th { "联系电话" }
                                th { "绑定账号" }
                                th { "账号角色" }
                                th { "入职日期" }
                                th { "标准班次" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if visible.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "9", "暂无符合条件的员工档案" }
                                }
                            }
                            for row in visible {
                                {
                                    let edit = row.clone();
                                    let remove = row.clone();
                                    let employee_id = row.employee_id;
                                    let department = row
                                        .department
                                        .clone()
                                        .unwrap_or_else(|| "未分配".into());
                                    let account_option = options
                                        .iter()
                                        .find(|option| Some(option.user_id) == row.user_id);
                                    let account = account_option
                                        .map(|option| format!("{} · {}", option.real_name, option.username))
                                        .unwrap_or_else(|| "未绑定".into());
                                    let role_summary = account_option
                                        .map(|option| {
                                            if option.role_names.is_empty() {
                                                "暂未分配角色".into()
                                            } else {
                                                option.role_names.join("、")
                                            }
                                        })
                                        .unwrap_or_else(|| "—".into());
                                    let check_in = seconds_to_time(row.check_in_seconds);
                                    let check_out = seconds_to_time(row.check_out_seconds);
                                    let schedule = if check_in.is_empty() && check_out.is_empty() {
                                        "未设置".to_string()
                                    } else {
                                        format!("{check_in} — {check_out}")
                                    };
                                    rsx! {
                                        tr { key: "employee-{employee_id}",
                                            td {
                                                strong { "{row.name}" }
                                            }
                                            td { "{department}" }
                                            td {
                                                Badge {
                                                    variant: if row.is_resigned { BadgeVariant::Outline } else { BadgeVariant::Secondary },
                                                    if row.is_resigned {
                                                        "已离职"
                                                    } else {
                                                        "在职"
                                                    }
                                                }
                                            }
                                            td { class: "is-mono", "{row.phone}" }
                                            td { "{account}" }
                                            td { class: "is-wrap hint", "{role_summary}" }
                                            td { class: "is-mono", "{format_date(row.hire_date)}" }
                                            td { class: "is-mono", "{schedule}" }
                                            td {
                                                div { class: "table-actions",
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        onclick: move |_| {
                                                            selected.set(Some(edit.clone()));
                                                            form_open.set(true);
                                                        },
                                                        "编辑档案"
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

        if form_open() {
            EmployeeFormDialog {
                employee: selected(),
                account_options: options.clone(),
                role_options: role_options.clone(),
                can_assign_roles,
                on_close: move |_| form_open.set(false),
                on_saved: move |_| {
                    form_open.set(false);
                    notice.set(Some("员工档案、账号与角色已保存".into()));
                },
            }
        }
        if let Some(row) = deleting() {
            EmployeeDeleteDialog {
                employee: row,
                on_close: move |_| deleting.set(None),
                on_deleted: move |_| {
                    deleting.set(None);
                    notice.set(Some("员工档案已删除并归档".into()));
                },
            }
        }
    }
}

#[component]
fn EmployeeDeleteDialog(
    employee: Employee,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut done = use_signal(|| false);
    let employee_id = employee.employee_id;
    let employee_name = employee.name.clone();

    use_effect(move || {
        if done() {
            on_deleted.call(())
        }
    });

    rsx! {
        ConfirmDialog {
            title: "确认删除员工",
            description: format!(
                "员工“{employee_name}”将被逻辑删除并记录离职时间，历史考勤不会丢失。",
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
                    match delete_employee_record(employee_id).await {
                        Ok(()) => done.set(true),
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
