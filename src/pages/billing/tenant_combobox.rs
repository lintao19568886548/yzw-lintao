//! 合同人搜索选择框。
//!
//! 自绘版本的样式在 CSS 重建时删掉了，键盘导航、失焦收起这些也都是手写的。
//! 换成组件库的 `Combobox`，这些行为由组件负责。

use dioxus::prelude::*;

use crate::{
    components::combobox::{Combobox, ComboboxOption},
    spacetime_bindings::rental_tenant_type::RentalTenant,
};

/// 下拉里最多展示多少个匹配项。
///
/// 合同人可能上千，全量渲染会明显拖慢输入；筛不到就继续输关键词。
const MAX_VISIBLE_OPTIONS: usize = 50;

#[component]
pub(crate) fn TenantCombobox(
    tenants: Vec<RentalTenant>,
    /// 选中的合同人 id，空串表示尚未选择。
    ///
    /// 用 sync 存储：账单表单里这个信号会被跨线程的保存回调读到。
    selected_id: SyncSignal<String>,
    readonly: bool,
) -> Element {
    let mut query = use_signal(String::new);
    let normalized = query().trim().to_lowercase();
    let matched = tenants
        .iter()
        .filter(|tenant| {
            normalized.is_empty()
                || tenant.tenant_name.to_lowercase().contains(&normalized)
                || tenant.phone_number.contains(&normalized)
        })
        .take(MAX_VISIBLE_OPTIONS)
        .cloned()
        .collect::<Vec<_>>();
    let selected_value: ReadSignal<Option<String>> = use_memo(move || Some(selected_id())).into();

    rsx! {
        Combobox {
            value: selected_value,
            disabled: readonly,
            placeholder: "输入姓名或手机号搜索".to_string(),
            aria_label: "合同人".to_string(),
            on_query_change: move |value: String| query.set(value),
            // 过滤已经在上面按姓名和手机号做过了，这里放行全部候选，
            // 否则组件会拿输入词再按显示文本过滤一遍，手机号搜索会失效。
            filter: move |_: (String, String)| true,
            on_value_change: move |value: Option<String>| {
                selected_id.set(value.unwrap_or_default());
            },
            for (index , tenant) in matched.iter().enumerate() {
                ComboboxOption::<String> {
                    key: "tenant-{tenant.rental_tenant_id}",
                    index,
                    value: tenant.rental_tenant_id.to_string(),
                    text_value: format!("{} {}", tenant.tenant_name, tenant.phone_number),
                    div { class: "stack-tight",
                        strong { "{tenant.tenant_name}" }
                        small { class: "hint is-mono", "{tenant.phone_number}" }
                    }
                }
            }
        }
    }
}
