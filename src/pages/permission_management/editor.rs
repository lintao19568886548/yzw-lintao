//! 角色基本信息、园区范围与菜单权限编辑器。

use dioxus::prelude::*;

use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
    },
    spacetime_bindings::{
        menu_type::Menu, park_type::Park, role_menu_type::RoleMenu, role_type::Role,
    },
};

use super::model::{allowed_menu_ids, menu_depth, role_is_descendant, RoleDraft};

#[component]
pub(super) fn RoleEditor(
    mut draft: Signal<RoleDraft>,
    roles: Vec<Role>,
    menus: Vec<Menu>,
    parks: Vec<Park>,
    role_menus: Vec<RoleMenu>,
    saving: bool,
    error_message: Option<String>,
    on_save: EventHandler<RoleDraft>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut menu_query = use_signal(String::new);
    let current = draft();
    let allowed_ids = allowed_menu_ids(&current, &roles, &menus, &role_menus);
    let keyword = menu_query().trim().to_lowercase();
    let mut visible_menus = menus
        .iter()
        .filter(|menu| {
            allowed_ids.contains(&menu.menu_id)
                && (keyword.is_empty()
                    || menu.name.to_lowercase().contains(&keyword)
                    || menu.path.to_lowercase().contains(&keyword)
                    || menu
                        .auth_code
                        .as_ref()
                        .is_some_and(|code| code.to_lowercase().contains(&keyword)))
        })
        .cloned()
        .collect::<Vec<_>>();
    visible_menus.sort_by_key(|menu| (menu_depth(menu, &menus), menu.menu_id));
    let has_visible_menus = !visible_menus.is_empty();
    let name_is_empty = current.name.trim().is_empty();
    let selected_count = current
        .menu_ids
        .iter()
        .filter(|menu_id| allowed_ids.contains(menu_id))
        .count();

    let parent_value: ReadSignal<Option<String>> = use_memo(move || {
        Some(
            draft()
                .parent_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
        )
    })
    .into();
    let auth_value: ReadSignal<Option<String>> = use_memo(move || {
        Some(
            draft()
                .reimbursement_auth
                .map(|value| value.to_string())
                .unwrap_or_default(),
        )
    })
    .into();
    let status_value: ReadSignal<Option<String>> =
        use_memo(move || Some(draft().status.to_string())).into();
    let selectable_parents = roles
        .iter()
        .filter(|role| {
            current.role_id != Some(role.role_id)
                && !current
                    .role_id
                    .is_some_and(|id| role_is_descendant(&roles, role.role_id, id))
        })
        .cloned()
        .collect::<Vec<_>>();
    let roles_for_parent = roles.clone();
    let menus_for_parent = menus.clone();
    let role_menus_for_parent = role_menus.clone();
    let allowed_for_toggle = allowed_ids.clone();

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !saving {
                    on_cancel.call(());
                }
            },
            DialogTitle {
                if current.role_id.is_some() {
                    "编辑角色"
                } else {
                    "新增角色"
                }
            }
            DialogDescription { "角色信息、园区数据范围和菜单权限将通过同一个事务保存。" }

            div { class: "stack",
                div { class: "form-grid",
                    div { class: "field",
                        Label { html_for: "role-name", "角色名称" }
                        Input {
                            id: "role-name",
                            value: current.name.clone(),
                            placeholder: "请输入角色名称",
                            disabled: saving,
                            oninput: move |event: FormEvent| draft.write().name = event.value(),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "role-parent", "上级角色" }
                        Select {
                            id: "role-parent",
                            value: Some(parent_value),
                            disabled: saving,
                            on_value_change: move |value: Option<String>| {
                                let next = value.and_then(|value| value.parse::<u64>().ok());
                                // 换了上级就要重新裁剪已选菜单：子角色不能超出上级的权限范围。
                                let allowed = allowed_menu_ids(
                                    &RoleDraft {
                                        parent_id: next,
                                        ..draft()
                                    },
                                    &roles_for_parent,
                                    &menus_for_parent,
                                    &role_menus_for_parent,
                                );
                                let mut value = draft.write();
                                value.parent_id = next;
                                value.menu_ids.retain(|menu_id| allowed.contains(menu_id));
                            },
                            SelectOption::<String> {
                                value: String::new(),
                                index: 0usize,
                                text_value: "无上级角色".to_string(),
                                "无上级角色"
                            }
                            for (index , role) in selectable_parents.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "parent-{role.role_id}",
                                    value: role.role_id.to_string(),
                                    index: index + 1,
                                    text_value: role.name.to_string(),
                                    "{role.name}"
                                }
                            }
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "role-remark", "角色说明" }
                        Input {
                            id: "role-remark",
                            value: current.remark.clone(),
                            placeholder: "说明这个角色负责的业务范围",
                            disabled: saving,
                            oninput: move |event: FormEvent| draft.write().remark = event.value(),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "role-status", "角色状态" }
                        Select {
                            id: "role-status",
                            value: Some(status_value),
                            disabled: saving,
                            on_value_change: move |value: Option<String>| {
                                draft.write().status = if value.as_deref() == Some("0") { 0 } else { 1 };
                            },
                            SelectOption::<String> { value: "1".to_string(), index: 0usize, text_value: "启用".to_string(), "启用" }
                            SelectOption::<String> { value: "0".to_string(), index: 1usize, text_value: "禁用".to_string(), "禁用" }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "role-auth", "报销审核权限" }
                        Select {
                            id: "role-auth",
                            value: Some(auth_value),
                            disabled: saving,
                            on_value_change: move |value: Option<String>| {
                                let next = value.and_then(|value| value.parse::<i32>().ok());
                                let mut value = draft.write();
                                value.reimbursement_auth = next;
                                // 拒绝审核时没有可用额度，金额必须同步归零。
                                if next == Some(0) {
                                    value.rates = "0".into();
                                }
                            },
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "未配置".to_string(), "未配置" }
                            SelectOption::<String> { value: "1".to_string(), index: 1usize, text_value: "允许审核".to_string(), "允许审核" }
                            SelectOption::<String> { value: "0".to_string(), index: 2usize, text_value: "拒绝审核".to_string(), "拒绝审核" }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "role-rates", "最大审核金额（元）" }
                        Input {
                            id: "role-rates",
                            r#type: "number",
                            min: "0",
                            value: current.rates.clone(),
                            placeholder: if current.reimbursement_auth == Some(0) { "拒绝审核时固定为 0" } else { "留空表示不限制" },
                            disabled: saving || current.reimbursement_auth == Some(0),
                            oninput: move |event: FormEvent| draft.write().rates = event.value(),
                        }
                    }
                }

                section { class: "subsection",
                    div { class: "section-header",
                        h4 { "所属园区" }
                        Badge {
                            variant: BadgeVariant::Outline,
                            "{current.park_ids.len()} / {parks.len()}"
                        }
                    }
                    p { class: "hint", "决定该角色可以读取和操作哪些园区数据。" }
                    if parks.is_empty() {
                        p { class: "empty", "暂无可授权园区" }
                    } else {
                        div { class: "grid-3",
                            for park in parks.iter() {
                                {
                                    let park_id = park.park_id;
                                    let selected = current.park_ids.contains(&park_id);
                                    rsx! {
                                        button {
                                            key: "park-chip-{park_id}",
                                            r#type: "button",
                                            class: if selected { "tile is-selected" } else { "tile" },
                                            aria_pressed: selected,
                                            disabled: saving,
                                            onclick: move |_| {
                                                let mut value = draft.write();
                                                if !value.park_ids.insert(park_id) {
                                                    value.park_ids.remove(&park_id);
                                                }
                                            },
                                            span { class: "tile-label", "{park.park_name}" }
                                            small { class: "hint", "{park.address}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                section { class: "subsection",
                    div { class: "section-header",
                        h4 { "菜单与按钮权限" }
                        Badge {
                            variant: BadgeVariant::Outline,
                            "{selected_count} / {allowed_ids.len()}"
                        }
                    }
                    p { class: "hint", "子角色只能选择上级角色已经拥有的权限。" }
                    div { class: "filters",
                        div { class: "field",
                            Label { html_for: "role-menu-query", "搜索权限" }
                            Input {
                                id: "role-menu-query",
                                value: menu_query,
                                placeholder: "搜索菜单、路由或权限码",
                                oninput: move |event: FormEvent| menu_query.set(event.value()),
                            }
                        }
                        div { class: "field is-action",
                            Button {
                                variant: ButtonVariant::Outline,
                                r#type: "button",
                                disabled: saving,
                                onclick: move |_| {
                                    let mut value = draft.write();
                                    if allowed_for_toggle.iter().all(|id| value.menu_ids.contains(id)) {
                                        value.menu_ids.retain(|id| !allowed_for_toggle.contains(id));
                                    } else {
                                        value.menu_ids.extend(allowed_for_toggle.iter().copied());
                                    }
                                },
                                if selected_count == allowed_ids.len() && !allowed_ids.is_empty() {
                                    "全部取消"
                                } else {
                                    "全部选择"
                                }
                            }
                        }
                    }
                    if !has_visible_menus {
                        p { class: "empty", "没有匹配的权限项" }
                    } else {
                        div { class: "list",
                            for menu in visible_menus {
                                {
                                    let menu_id = menu.menu_id;
                                    let selected = current.menu_ids.contains(&menu_id);
                                    // 菜单层级同样靠缩进表达
                                    let indent = format!(
                                        "padding-left: {}rem",
                                        0.5 + menu_depth(&menu, &menus) as f64 * 1.25,
                                    );
                                    rsx! {
                                        button {
                                            key: "menu-row-{menu_id}",
                                            r#type: "button",
                                            class: if selected { "list-item is-selected" } else { "list-item" },
                                            style: "{indent}",
                                            aria_pressed: selected,
                                            disabled: saving,
                                            onclick: move |_| {
                                                let mut value = draft.write();
                                                if !value.menu_ids.insert(menu_id) {
                                                    value.menu_ids.remove(&menu_id);
                                                }
                                            },
                                            div { class: "list-item-copy",
                                                strong { "{menu.name}" }
                                                small { class: "is-mono", "{menu.path}" }
                                            }
                                            div { class: "row",
                                                if menu.menu_type == "button" {
                                                    Badge { variant: BadgeVariant::Outline, "按钮" }
                                                }
                                                if menu.template_internal_only {
                                                    Badge { variant: BadgeVariant::Outline, "内部" }
                                                }
                                                if let Some(code) = &menu.auth_code {
                                                    small { class: "hint is-mono", "{code}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(message) = error_message {
                    p { class: "form-error", role: "alert", "{message}" }
                }

                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        disabled: saving,
                        onclick: move |_| on_cancel.call(()),
                        "取消"
                    }
                    Button {
                        r#type: "button",
                        disabled: saving || name_is_empty,
                        onclick: move |_| on_save.call(draft()),
                        if saving {
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
