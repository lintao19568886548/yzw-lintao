//! 个人中心：本人头像与显示姓名的自助维护。
//!
//! 页面对任何已登录账号开放（`permissions::routes` 有对应豁免）——个人资料
//! 不是业务权限，不该要求任何菜单授权。服务端只允许改登录者自己的行。

use dioxus::prelude::*;

use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
    },
    services::{
        change_my_password_record, update_my_avatar_record, update_my_name_record,
        upload_business_image, validate_business_image,
    },
    spacetime_bindings::uploaded_avatar_input_type::UploadedAvatarInput,
    state::WorkspaceState,
};

#[component]
pub fn ProfilePage() -> Element {
    let state = use_context::<WorkspaceState>();
    let user = (state.business_user)();
    let roles = state.roles.read().clone();
    // None = 未在编辑，显示当前值；Some = 输入框里的草稿。
    let mut name_draft = use_signal_sync(|| None::<String>);
    let mut avatar_busy = use_signal_sync(|| false);
    let mut name_busy = use_signal_sync(|| false);
    let mut old_password = use_signal_sync(String::new);
    let mut new_password = use_signal_sync(String::new);
    let mut confirm_password = use_signal_sync(String::new);
    let mut password_busy = use_signal_sync(|| false);
    let mut feedback = use_signal_sync(|| None::<String>);
    let mut error = use_signal_sync(|| None::<String>);

    let Some(user) = user else {
        return rsx! {
            main { class: "page inspect-page",
                section { class: "section",
                    Card { CardContent {
                        p { class: "hint", "正在加载账号信息…" }
                    } }
                }
            }
        };
    };
    let current_name = user.real_name.clone();
    let name_value = name_draft().unwrap_or_else(|| current_name.clone());
    let role_names = roles
        .iter()
        .map(|role| role.name.clone())
        .collect::<Vec<_>>()
        .join("、");

    rsx! {
        main { class: "page inspect-page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "个人中心" }
                    p { class: "page-subtitle", "维护自己的头像与显示姓名；登录名与角色由管理员管理。" }
                }
            }

            if let Some(message) = feedback() {
                p { class: "notice", role: "status", "{message}" }
            }
            if let Some(message) = error() {
                p { class: "form-error", role: "alert", "{message}" }
            }

            section { class: "section",
                Card { CardContent { div { class: "stack",
                    h4 { "头像" }
                    div { class: "row profile-avatar-row",
                        if let Some(url) = user.avatar_url.clone() {
                            img { class: "profile-avatar", src: "{url}", alt: "头像" }
                        } else {
                            div { class: "profile-avatar profile-avatar-empty", "{avatar_initial(&current_name)}" }
                        }
                        label { class: "business-image-upload",
                            input {
                                r#type: "file",
                                accept: "image/jpeg,image/png,image/webp",
                                disabled: avatar_busy(),
                                onchange: move |event| {
                                    let Some(file) = event.files().into_iter().next() else {
                                        return;
                                    };
                                    if let Err(message) = validate_business_image(&file) {
                                        error.set(Some(message));
                                        return;
                                    }
                                    error.set(None);
                                    feedback.set(None);
                                    avatar_busy.set(true);
                                    spawn(async move {
                                        let uploaded = match upload_business_image(file).await {
                                            Ok(image) => image,
                                            Err(message) => {
                                                avatar_busy.set(false);
                                                error.set(Some(message));
                                                return;
                                            }
                                        };
                                        let result = update_my_avatar_record(UploadedAvatarInput {
                                            img_url: uploaded.public_url,
                                            hash: uploaded.sha256,
                                        })
                                        .await;
                                        avatar_busy.set(false);
                                        match result {
                                            Ok(()) => feedback.set(Some("头像已更新".into())),
                                            Err(message) => error.set(Some(message)),
                                        }
                                    });
                                },
                            }
                            span { if avatar_busy() { "上传中…" } else { "更换头像" } }
                        }
                    }
                    small { class: "hint", "支持 JPG、PNG、WebP，单张不超过 10MB。" }
                } } }
            }

            section { class: "section",
                Card { CardContent { div { class: "stack",
                    h4 { "显示姓名" }
                    div { class: "field",
                        Label { html_for: "profile-name", "姓名" }
                        Input {
                            id: "profile-name",
                            value: name_value.clone(),
                            maxlength: 50,
                            oninput: move |event: FormEvent| name_draft.set(Some(event.value())),
                        }
                    }
                    small { class: "hint", "巡检、审批等历史记录里的姓名是提交当时的快照，改名不会改动历史。" }
                    div { class: "form-actions",
                        Button {
                            r#type: "button",
                            disabled: name_busy() || name_value.trim().is_empty(),
                            onclick: move |_| {
                                let next = name_value.trim().to_string();
                                if next.is_empty() {
                                    error.set(Some("姓名不能为空".into()));
                                    return;
                                }
                                error.set(None);
                                feedback.set(None);
                                name_busy.set(true);
                                spawn(async move {
                                    let result = update_my_name_record(next).await;
                                    name_busy.set(false);
                                    match result {
                                        Ok(()) => {
                                            name_draft.set(None);
                                            feedback.set(Some("姓名已更新".into()));
                                        }
                                        Err(message) => error.set(Some(message)),
                                    }
                                });
                            },
                            if name_busy() { "保存中…" } else { "保存姓名" }
                        }
                    }
                } } }
            }

            section { class: "section",
                Card { CardContent { div { class: "stack",
                    h4 { "修改密码" }
                    div { class: "field",
                        Label { html_for: "profile-old-password", "旧密码" }
                        Input {
                            id: "profile-old-password",
                            r#type: "password",
                            value: old_password(),
                            autocomplete: "current-password",
                            oninput: move |event: FormEvent| old_password.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "profile-new-password", "新密码" }
                        Input {
                            id: "profile-new-password",
                            r#type: "password",
                            value: new_password(),
                            autocomplete: "new-password",
                            placeholder: "8 到 128 位",
                            oninput: move |event: FormEvent| new_password.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "profile-confirm-password", "确认新密码" }
                        Input {
                            id: "profile-confirm-password",
                            r#type: "password",
                            value: confirm_password(),
                            autocomplete: "new-password",
                            oninput: move |event: FormEvent| confirm_password.set(event.value()),
                        }
                    }
                    small { class: "hint", "修改成功后，其他已登录设备会被退出，本设备保持登录。" }
                    div { class: "form-actions",
                        Button {
                            r#type: "button",
                            disabled: password_busy(),
                            onclick: move |_| {
                                if password_busy() {
                                    return;
                                }
                                let old_value = old_password();
                                let new_value = new_password();
                                if old_value.is_empty() {
                                    error.set(Some("请输入旧密码".into()));
                                    return;
                                }
                                let length = new_value.chars().count();
                                if !(8..=128).contains(&length) {
                                    error.set(Some("新密码长度需在 8 到 128 位之间".into()));
                                    return;
                                }
                                if new_value == old_value {
                                    error.set(Some("新密码不能与旧密码相同".into()));
                                    return;
                                }
                                if new_value != confirm_password() {
                                    error.set(Some("两次输入的新密码不一致".into()));
                                    return;
                                }
                                error.set(None);
                                feedback.set(None);
                                password_busy.set(true);
                                spawn(async move {
                                    let result =
                                        change_my_password_record(old_value, new_value).await;
                                    password_busy.set(false);
                                    match result {
                                        Ok(()) => {
                                            old_password.set(String::new());
                                            new_password.set(String::new());
                                            confirm_password.set(String::new());
                                            feedback.set(Some(
                                                "密码已修改，其他设备已退出登录".into(),
                                            ));
                                        }
                                        Err(message) => error.set(Some(message)),
                                    }
                                });
                            },
                            if password_busy() { "修改中…" } else { "修改密码" }
                        }
                    }
                } } }
            }

            section { class: "section",
                Card { CardContent { div { class: "stack",
                    h4 { "账号信息" }
                    dl { class: "facts",
                        div { dt { "登录名" } dd { "{user.username}" } }
                        div { dt { "手机号" } dd { {user.phone.clone().unwrap_or_else(|| "未绑定".into())} } }
                        div { dt { "用户编号" } dd { class: "is-mono", "#{user.id}" } }
                    }
                    if !role_names.is_empty() {
                        div { class: "row",
                            span { class: "hint", "角色" }
                            Badge { variant: BadgeVariant::Secondary, "{role_names}" }
                        }
                    }
                    small { class: "hint", "登录名、手机号与角色由管理员在用户与权限管理中维护。" }
                } } }
            }

            section { class: "section",
                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        onclick: move |_| {
                            navigator().go_back();
                        },
                        "返回"
                    }
                }
            }
        }
    }
}

/// 无头像时的占位：取姓名首个字符。
fn avatar_initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "我".into())
}
