//! 员工档案新增与编辑表单。

use dioxus::prelude::*;

use super::model::{format_date, parse_date, parse_time, seconds_to_time};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        DateField, TimeField,
    },
    services::{create_employee_record, update_employee_record},
    spacetime_bindings::{
        employee_input_type::EmployeeInput, employee_type::Employee,
        employee_user_option_type::EmployeeUserOption, role_type::Role,
    },
};

fn optional(value: String) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}

#[component]
pub(super) fn EmployeeFormDialog(
    employee: Option<Employee>,
    account_options: Vec<EmployeeUserOption>,
    role_options: Vec<Role>,
    can_assign_roles: bool,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let employee_id = employee.as_ref().map(|row| row.employee_id);
    let mut name = use_signal(|| {
        employee
            .as_ref()
            .map(|row| row.name.clone())
            .unwrap_or_default()
    });
    let mut gender = use_signal(|| {
        employee
            .as_ref()
            .map(|row| row.gender.clone())
            .unwrap_or_else(|| "男".into())
    });
    let mut phone = use_signal(|| {
        employee
            .as_ref()
            .map(|row| row.phone.clone())
            .unwrap_or_default()
    });
    let mut user_id = use_signal(|| {
        employee
            .as_ref()
            .and_then(|row| row.user_id)
            .map(|id| id.to_string())
            .unwrap_or_default()
    });
    let initial_role_ids = employee
        .as_ref()
        .and_then(|row| row.user_id)
        .and_then(|id| account_options.iter().find(|option| option.user_id == id))
        .map(|option| option.role_ids.clone())
        .unwrap_or_default();
    let mut role_ids = use_signal(move || initial_role_ids);
    let mut age = use_signal(|| {
        employee
            .as_ref()
            .and_then(|row| row.age)
            .map(|v| v.to_string())
            .unwrap_or_default()
    });
    let mut id_number = use_signal(|| {
        employee
            .as_ref()
            .and_then(|row| row.id_number.clone())
            .unwrap_or_default()
    });
    let mut department = use_signal(|| {
        employee
            .as_ref()
            .and_then(|row| row.department.clone())
            .unwrap_or_default()
    });
    let mut education = use_signal(|| {
        employee
            .as_ref()
            .and_then(|row| row.education.clone())
            .unwrap_or_default()
    });
    let mut hire_date = use_signal(|| {
        employee
            .as_ref()
            .map(|row| format_date(row.hire_date))
            .filter(|v| v != "--")
            .unwrap_or_default()
    });
    let mut resigned = use_signal(|| employee.as_ref().is_some_and(|row| row.is_resigned));
    let mut leave_date = use_signal(|| {
        employee
            .as_ref()
            .map(|row| format_date(row.leave_date))
            .filter(|v| v != "--")
            .unwrap_or_default()
    });
    let mut address = use_signal(|| {
        employee
            .as_ref()
            .and_then(|row| row.address.clone())
            .unwrap_or_default()
    });
    let mut check_in = use_signal(|| {
        employee
            .as_ref()
            .map(|row| seconds_to_time(row.check_in_seconds))
            .unwrap_or_default()
    });
    let mut check_out = use_signal(|| {
        employee
            .as_ref()
            .map(|row| seconds_to_time(row.check_out_seconds))
            .unwrap_or_default()
    });
    let mut remark = use_signal(|| {
        employee
            .as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut completed = use_signal(|| false);
    use_effect(move || {
        if completed() {
            on_saved.call(());
        }
    });
    let title = if employee_id.is_some() {
        "编辑员工档案"
    } else {
        "新增员工档案"
    };
    let selected_account = user_id()
        .parse::<u64>()
        .ok()
        .and_then(|id| account_options.iter().find(|option| option.user_id == id))
        .cloned();
    let account_options_for_change = account_options.clone();
    let gender_value: ReadSignal<Option<String>> = use_memo(move || Some(gender())).into();
    let education_value: ReadSignal<Option<String>> = use_memo(move || Some(education())).into();
    let resigned_value: ReadSignal<Option<String>> =
        use_memo(move || Some(if resigned() { "resigned" } else { "active" }.to_string())).into();
    let account_value: ReadSignal<Option<String>> = use_memo(move || Some(user_id())).into();

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { "{title}" }
            DialogDescription { "维护人员身份、账号关系和标准班次。" }
            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    if loading() {
                        return;
                    }
                    if name().trim().chars().count() < 2 {
                        error.set(Some("姓名至少需要 2 个字符".into()));
                        return;
                    }
                    let phone_value = phone().trim().to_string();
                    if phone_value.len() != 11 || !phone_value.bytes().all(|b| b.is_ascii_digit()) {
                        error.set(Some("请输入 11 位手机号码".into()));
                        return;
                    }
                    let age_value = if age().trim().is_empty() {
                        None
                    } else {
                        match age().parse::<i32>() {
                            Ok(v) if (0..=150).contains(&v) => Some(v),
                            _ => {
                                error.set(Some("年龄必须在 0 到 150 之间".into()));
                                return;
                            }
                        }
                    };
                    let hire = if hire_date().is_empty() {
                        None
                    } else {
                        match parse_date(&hire_date()) {
                            Ok(v) => Some(v),
                            Err(m) => {
                                error.set(Some(m));
                                return;
                            }
                        }
                    };
                    let leave = if resigned() {
                        if leave_date().is_empty() {
                            error.set(Some("离职员工必须填写离职日期".into()));
                            return;
                        }
                        match parse_date(&leave_date()) {
                            Ok(v) => Some(v),
                            Err(m) => {
                                error.set(Some(m));
                                return;
                            }
                        }
                    } else {
                        None
                    };
                    let check_in_seconds = match parse_time(&check_in()) {
                        Ok(v) => v,
                        Err(m) => {
                            error.set(Some(m));
                            return;
                        }
                    };
                    let check_out_seconds = match parse_time(&check_out()) {
                        Ok(v) => v,
                        Err(m) => {
                            error.set(Some(m));
                            return;
                        }
                    };
                    let input = EmployeeInput {
                        name: name().trim().to_string(),
                        gender: gender(),
                        phone: phone_value,
                        user_id: user_id().parse().ok(),
                        age: age_value,
                        id_number: optional(id_number()),
                        address: optional(address()),
                        education: optional(education()),
                        department: optional(department()),
                        hire_date: hire,
                        leave_date: leave,
                        remark: optional(remark()),
                        is_resigned: resigned(),
                        check_in_seconds,
                        check_out_seconds,
                    };
                    loading.set(true);
                    error.set(None);
                    let assigned_role_ids = can_assign_roles.then(|| role_ids());
                    spawn(async move {
                        let result = if let Some(id) = employee_id {
                            update_employee_record(id, input, assigned_role_ids).await
                        } else {
                            create_employee_record(input, assigned_role_ids).await
                        };
                        match result {
                            Ok(()) => completed.set(true),
                            Err(message) => {
                                loading.set(false);
                                error.set(Some(message));
                            }
                        }
                    });
                },
                div { class: "form-grid",
                    div { class: "field",
                        Label { html_for: "employee-name", "姓名 *" }
                        Input {
                            id: "employee-name",
                            value: name(),
                            maxlength: 50,
                            disabled: loading(),
                            oninput: move |e: FormEvent| name.set(e.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "employee-gender", "性别 *" }
                        Select {
                            id: "employee-gender",
                            value: Some(gender_value),
                            disabled: loading(),
                            on_value_change: move |value: Option<String>| gender.set(value.unwrap_or_else(|| "男".into())),
                            SelectOption::<String> { value: "男".to_string(), index: 0usize, text_value: "男".to_string(), "男" }
                            SelectOption::<String> { value: "女".to_string(), index: 1usize, text_value: "女".to_string(), "女" }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "employee-phone", "手机号 *" }
                        Input {
                            id: "employee-phone",
                            value: phone(),
                            inputmode: "numeric",
                            maxlength: 20,
                            disabled: loading(),
                            oninput: move |e: FormEvent| phone.set(e.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "employee-id-number", "身份证号" }
                        Input {
                            id: "employee-id-number",
                            value: id_number(),
                            maxlength: 18,
                            disabled: loading(),
                            oninput: move |e: FormEvent| id_number.set(e.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "employee-age", "年龄" }
                        Input {
                            id: "employee-age",
                            r#type: "number",
                            min: "0",
                            max: "150",
                            value: age(),
                            disabled: loading(),
                            oninput: move |e: FormEvent| age.set(e.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "employee-department", "部门" }
                        Input {
                            id: "employee-department",
                            value: department(),
                            maxlength: 50,
                            disabled: loading(),
                            oninput: move |e: FormEvent| department.set(e.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "employee-education", "学历" }
                        Select {
                            id: "employee-education",
                            value: Some(education_value),
                            disabled: loading(),
                            on_value_change: move |value: Option<String>| education.set(value.unwrap_or_default()),
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择".to_string(), "请选择" }
                            for (index , value) in ["小学", "初中", "高中", "专科", "本科", "研究生", "博士"]
                                .into_iter()
                                .enumerate()
                            {
                                SelectOption::<String> {
                                    key: "education-{value}",
                                    value: value.to_string(),
                                    index: index + 1,
                                    text_value: value.to_string(), "{value}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "入职日期" }
                        DateField {
                            value: hire_date(),
                            disabled: loading(),
                            max: (!leave_date().is_empty()).then(|| leave_date()),
                            on_change: move |value| hire_date.set(value),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "employee-resigned", "人员状态" }
                        Select {
                            id: "employee-resigned",
                            value: Some(resigned_value),
                            disabled: loading(),
                            on_value_change: move |value: Option<String>| {
                                resigned.set(value.as_deref() == Some("resigned"));
                            },
                            SelectOption::<String> { value: "active".to_string(), index: 0usize, text_value: "在职".to_string(), "在职" }
                            SelectOption::<String> { value: "resigned".to_string(), index: 1usize, text_value: "离职".to_string(), "离职" }
                        }
                    }
                    if resigned() {
                        div { class: "field",
                            span { class: "field-label", "离职日期 *" }
                            DateField {
                                value: leave_date(),
                                disabled: loading(),
                                min: (!hire_date().is_empty()).then(|| hire_date()),
                                on_change: move |value| leave_date.set(value),
                            }
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "标准上班时间" }
                        TimeField {
                            value: check_in(),
                            disabled: loading(),
                            on_change: move |value| check_in.set(value),
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "标准下班时间" }
                        TimeField {
                            value: check_out(),
                            disabled: loading(),
                            on_change: move |value| check_out.set(value),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "employee-address", "住址" }
                        Input {
                            id: "employee-address",
                            value: address(),
                            maxlength: 200,
                            disabled: loading(),
                            oninput: move |e: FormEvent| address.set(e.value()),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "employee-remark", "备注" }
                        Textarea {
                            id: "employee-remark",
                            value: remark(),
                            maxlength: 500,
                            rows: 3,
                            disabled: loading(),
                            oninput: move |e: FormEvent| remark.set(e.value()),
                        }
                    }
                }

                section { class: "subsection",
                    div { class: "section-header",
                        h4 { "账号与角色" }
                        Badge {
                            variant: if can_assign_roles { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                            if can_assign_roles {
                                "可配置"
                            } else {
                                "仅查看"
                            }
                        }
                    }
                    p { class: "hint", "将员工档案关联到登录账号；角色同时决定菜单入口、操作权限与园区数据范围。" }

                    div { class: "field",
                        Label { html_for: "employee-account", "绑定登录账号" }
                        Select {
                            id: "employee-account",
                            value: Some(account_value),
                            disabled: loading(),
                            on_value_change: move |value: Option<String>| {
                                let value = value.unwrap_or_default();
                                let selected_roles = value
                                    .parse::<u64>()
                                    .ok()
                                    .and_then(|id| {
                                        account_options_for_change.iter().find(|option| option.user_id == id)
                                    })
                                    .map(|option| option.role_ids.clone())
                                    .unwrap_or_default();
                                user_id.set(value);
                                role_ids.set(selected_roles);
                            },
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "暂不绑定账号".to_string(), "暂不绑定账号" }
                            for (index , option) in account_options.iter().enumerate() {
                                {
                                    // 已绑定到别人或已停用的账号不能再选
                                    let unavailable = option
                                        .bound_employee_id
                                        .is_some_and(|id| Some(id) != employee_id);
                                    let label = format!(
                                        "{} · {} ({})",
                                        option.real_name,
                                        option.username,
                                        option.phone.clone().unwrap_or_else(|| "无手机号".into()),
                                    );
                                    rsx! {
                                        SelectOption::<String> {
                                            key: "account-{option.user_id}",
                                            value: option.user_id.to_string(),
                                            index: index + 1,
                                            disabled: unavailable || option.status != 1,
                                            text_value: label.to_string(), "{label}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(account) = selected_account.clone() {
                        div { class: "panel is-tight is-plain",
                            div { class: "section-header",
                                div { class: "stack-tight",
                                    strong { "{account.real_name}" }
                                    small { class: "hint", "@{account.username}" }
                                }
                                Badge {
                                    variant: if account.status == 1 { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                    if account.status == 1 {
                                        "账号启用"
                                    } else {
                                        "账号停用"
                                    }
                                }
                            }
                        }
                    }

                    div { class: "section-header",
                        span { class: "field-label", "账号角色" }
                        span { class: "hint", "已选择 {role_ids().len()} 项" }
                    }
                    if selected_account.is_none() {
                        p { class: "empty", "请先选择一个登录账号" }
                    } else if can_assign_roles {
                        div { class: "grid-3",
                            for role in role_options.iter().filter(|role| role.status == 1) {
                                {
                                    let role_id = role.role_id;
                                    let selected = role_ids().contains(&role_id);
                                    let scope = match role.scope.as_str() {
                                        "system" => "系统范围",
                                        "organization" => "组织范围",
                                        _ => "租户范围",
                                    };
                                    rsx! {
                                        label {
                                            key: "employee-role-{role_id}",
                                            class: if selected { "tile is-selected" } else { "tile" },
                                            div { class: "row",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: selected,
                                                    disabled: loading(),
                                                    onchange: move |event| {
                                                        let checked = event.checked();
                                                        role_ids
                                                            .with_mut(|ids| {
                                                                if checked && !ids.contains(&role_id) {
                                                                    ids.push(role_id);
                                                                    ids.sort_unstable();
                                                                }
                                                                if !checked {
                                                                    ids.retain(|id| *id != role_id);
                                                                }
                                                            });
                                                    },
                                                }
                                                span { class: "tile-label", "{role.name}" }
                                            }
                                            small { class: "hint", "{scope}" }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        div { class: "row",
                            if let Some(account) = selected_account.clone() {
                                if account.role_names.is_empty() {
                                    span { class: "hint", "该账号暂未分配角色" }
                                }
                                for role_name in &account.role_names {
                                    Badge { key: "role-name-{role_name}", variant: BadgeVariant::Outline, "{role_name}" }
                                }
                            }
                        }
                        p { class: "hint", "当前账号没有角色管理权限；角色信息仅供查看。" }
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
                            "正在保存…"
                        } else {
                            "确认保存"
                        }
                    }
                }
            }
        }
    }
}
