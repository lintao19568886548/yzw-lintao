//! 员工搜索选择框。
//!
//! 工资的收款人是员工，不是租赁合同方——这个选择器只列员工档案。
//! 账单那边仍按合同方选人，用的是 `pages::billing::tenant_combobox`。

use dioxus::prelude::*;

use crate::{
    components::combobox::{Combobox, ComboboxOption},
    spacetime_bindings::employee_type::Employee,
};

/// 下拉里最多展示多少个匹配项；筛不到就继续输关键词。
const MAX_VISIBLE_OPTIONS: usize = 50;

#[component]
pub(crate) fn EmployeeCombobox(
    employees: Vec<Employee>,
    /// 选中的员工 id，空串表示尚未选择。
    ///
    /// 用 sync 存储：保存回调会跨线程读它。
    selected_id: SyncSignal<String>,
    readonly: bool,
) -> Element {
    let mut query = use_signal(String::new);
    let normalized = query().trim().to_lowercase();
    let matched = employees
        .iter()
        .filter(|employee| !employee.is_deleted)
        .filter(|employee| {
            normalized.is_empty()
                || employee.name.to_lowercase().contains(&normalized)
                || employee.phone.contains(&normalized)
                || employee
                    .department
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&normalized))
        })
        .take(MAX_VISIBLE_OPTIONS)
        .cloned()
        .collect::<Vec<_>>();
    let selected_value: ReadSignal<Option<String>> = use_memo(move || Some(selected_id())).into();

    rsx! {
        Combobox {
            value: selected_value,
            disabled: readonly,
            placeholder: "输入姓名、手机号或部门搜索".to_string(),
            aria_label: "员工".to_string(),
            on_query_change: move |value: String| query.set(value),
            // 过滤已经在上面按姓名、手机号和部门做过了，这里放行全部候选，
            // 否则组件会拿输入词再按显示文本过滤一遍，手机号搜索会失效。
            filter: move |_: (String, String)| true,
            on_value_change: move |value: Option<String>| {
                selected_id.set(value.unwrap_or_default());
            },
            for (index , employee) in matched.iter().enumerate() {
                ComboboxOption::<String> {
                    key: "employee-{employee.employee_id}",
                    index,
                    value: employee.employee_id.to_string(),
                    text_value: format!("{} {}", employee.name, employee.phone),
                    div { class: "stack-tight",
                        strong { "{employee.name}" }
                        small { class: "hint is-mono",
                            "{employee.phone}"
                            if let Some(department) = employee.department.as_deref() {
                                " · {department}"
                            }
                        }
                    }
                }
            }
        }
    }
}
