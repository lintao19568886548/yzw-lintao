//! 权限管理总览与角色树操作。

use dioxus::prelude::*;

use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        ConfirmDialog,
    },
    permissions::can_manage_permissions,
    services::{delete_role_policy, save_role_policy},
    spacetime_bindings::{role_type::Role, RolePolicyInput},
    state::WorkspaceState,
};

use super::{
    editor::RoleEditor,
    model::{ordered_roles, role_scope_label, RoleDraft},
};

/// 原项目 `/system/role` 的角色、园区和菜单权限工作台。
#[component]
pub fn RoleManagementPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let mut draft = use_signal(RoleDraft::default);
    let mut editor_open = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let mut notice = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);
    let mut delete_target = use_signal(|| None::<Role>);

    let current_roles = (state.roles)();
    let current_menus = (state.menus)();
    let can_manage = can_manage_permissions(&current_roles, &current_menus);
    let roles = (state.permission_roles)();
    let menus = (state.permission_menus)();
    let parks = (state.permission_parks)();
    let role_menus = (state.permission_role_menus)();
    let role_parks = (state.permission_role_parks)();
    let ordered = ordered_roles(&roles);

    if !can_manage {
        return rsx! {
            main { class: "page",
                Card {
                    CardContent {
                        div { class: "stack",
                            h1 { "没有角色管理权限" }
                            p { class: "page-subtitle",
                                "角色管理入口由动态菜单授权；请让上级角色为当前账号分配「角色管理」菜单。"
                            }
                        }
                    }
                }
            }
        };
    }

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "角色管理" }
                    p { class: "page-subtitle",
                        "角色树、园区范围、菜单与按钮权限、报销审核策略均来自 SpacetimeDB 实时数据。"
                    }
                }
                div { class: "page-actions",
                    Button {
                        onclick: move |_| {
                            draft.set(RoleDraft::new(None));
                            editor_open.set(true);
                            error.set(None);
                            notice.set(None);
                        },
                        "新增角色"
                    }
                }
            }

            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }
            if let Some(message) = error() {
                p { class: "form-error", role: "alert", "{message}" }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "角色树" }
                    Badge { variant: BadgeVariant::Secondary, "{roles.len()} 个角色" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "角色" }
                                th { "范围" }
                                th { "状态" }
                                th { "审核额度" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if roles.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "5", "暂无可管理角色。请确认当前账号已获得角色管理菜单权限。" }
                                }
                            }
                            for (role , depth) in ordered {
                                {
                                    // 上下级关系靠缩进表达：角色树最多几层，
                                    // 单独做成可折叠的树反而更难一眼看全。
                                    let indent = format!("padding-left: {}rem", depth as f64 * 1.25);
                                    let enabled = role.status == 1;
                                    let is_system_super = role.name == "Super" && role.scope == "system";
                                    let edit_role = role.clone();
                                    let edit_menus = role_menus.clone();
                                    let edit_parks = role_parks.clone();
                                    let child_parent_id = role.role_id;
                                    let remove_role = role.clone();
                                    rsx! {
                                        tr { key: "role-{role.role_id}",
                                            td {
                                                div { class: "stack-tight", style: "{indent}",
                                                    strong { "{role.name}" }
                                                    small { class: "hint",
                                                        if let Some(remark) = &role.remark {
                                                            "{remark}"
                                                        } else {
                                                            "未填写说明"
                                                        }
                                                    }
                                                }
                                            }
                                            td { "{role_scope_label(&role)}" }
                                            td {
                                                Badge {
                                                    variant: if enabled { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                                    if enabled {
                                                        "启用"
                                                    } else {
                                                        "禁用"
                                                    }
                                                }
                                            }
                                            td { class: "is-mono",
                                                if role.reimbursement_auth == Some(0) {
                                                    "¥0"
                                                } else if let Some(rate) = role.rates {
                                                    "¥{rate}"
                                                } else {
                                                    "不限"
                                                }
                                            }
                                            td {
                                                div { class: "table-actions",
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        onclick: move |_| {
                                                            draft.set(RoleDraft::new(Some(child_parent_id)));
                                                            editor_open.set(true);
                                                            error.set(None);
                                                        },
                                                        "新增下级"
                                                    }
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        onclick: move |_| {
                                                            draft
                                                                .set(RoleDraft::from_role(&edit_role, &edit_menus, &edit_parks));
                                                            editor_open.set(true);
                                                            error.set(None);
                                                        },
                                                        "编辑"
                                                    }
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        class: "is-quiet-danger",
                                                        size: ButtonSize::Sm,
                                                        // 系统 Super 删掉就没人能再管权限了，直接禁用入口
                                                        disabled: is_system_super,
                                                        onclick: move |_| delete_target.set(Some(remove_role.clone())),
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
        }

        if editor_open() {
            RoleEditor {
                draft,
                roles: roles.clone(),
                menus: menus.clone(),
                parks: parks.clone(),
                role_menus: role_menus.clone(),
                saving: saving(),
                error_message: error(),
                on_cancel: move |_| editor_open.set(false),
                on_save: move |value: RoleDraft| {
                    saving.set(true);
                    error.set(None);
                    notice.set(None);
                    spawn(async move {
                        let rates = if value.reimbursement_auth == Some(0) {
                            Some(0)
                        } else if value.rates.trim().is_empty() {
                            None
                        } else {
                            match value.rates.trim().parse::<i32>() {
                                Ok(number) => Some(number),
                                Err(_) => {
                                    saving.set(false);
                                    error.set(Some("审核金额必须是有效整数".into()));
                                    return;
                                }
                            }
                        };
                        let result = save_role_policy(RolePolicyInput {
                                role_id: value.role_id,
                                name: value.name,
                                remark: (!value.remark.trim().is_empty()).then_some(value.remark),
                                status: value.status,
                                parent_id: value.parent_id,
                                reimbursement_auth: value.reimbursement_auth,
                                rates,
                                menu_ids: value.menu_ids.into_iter().collect(),
                                park_ids: value.park_ids.into_iter().collect(),
                            })
                            .await;
                        saving.set(false);
                        match result {
                            Ok(()) => {
                                editor_open.set(false);
                                notice
                                    .set(
                                        Some("角色权限已写入 SpacetimeDB，并会实时推送到相关账号。".into()),
                                    );
                            }
                            Err(message) => error.set(Some(message)),
                        }
                    });
                },
            }
        }

        if let Some(role) = delete_target() {
            ConfirmDialog {
                title: format!("删除角色“{}”", role.name),
                description: "该角色的用户、菜单、园区和权限码关系会一并清理；存在下级角色时服务端会拒绝删除。"
                    .to_string(),
                confirm_label: "确认删除",
                busy: saving(),
                on_cancel: move |_| delete_target.set(None),
                on_confirm: move |_| {
                    let role_id = role.role_id;
                    saving.set(true);
                    error.set(None);
                    spawn(async move {
                        let result = delete_role_policy(role_id).await;
                        saving.set(false);
                        delete_target.set(None);
                        match result {
                            Ok(()) => notice.set(Some("角色已删除。".into())),
                            Err(message) => error.set(Some(message)),
                        }
                    });
                },
            }
        }
    }
}
