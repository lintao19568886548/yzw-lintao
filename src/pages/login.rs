//! 云园慧控密码与短信双模式登录页面。

use dioxus::prelude::*;

use crate::{
    components::ConnectivityStatus,
    components::{BrandLogo, LogoSize},
    components::{
        button::{Button, ButtonVariant},
        input::Input,
        label::Label,
    },
    services::{
        connect_workspace, load_saved_account, login_workspace, login_workspace_with_sms,
        send_login_sms_code, ConnectionConfig,
    },
    state::{ConnectionPhase, WorkspaceState},
};

#[derive(Clone, Copy, PartialEq)]
enum LoginMode {
    Password,
    Sms,
}

/// 校验密码表单并调用账号认证 Reducer。
fn submit_password_login(
    account: Signal<String>,
    password: Signal<String>,
    remember_me: Signal<bool>,
    state: WorkspaceState,
) {
    let account = account().trim().to_string();
    let password = password();
    let mut callback_state = state;
    if account.is_empty() || password.is_empty() {
        callback_state
            .error_message
            .set(Some("请输入账号和密码".into()));
        return;
    }
    if let Err(error) = login_workspace(account, password, remember_me(), state) {
        callback_state.error_message.set(Some(error));
    }
}

/// 短信发送仍要求共享连接在线；密码登录则支持在连接前排队。
fn ensure_connected(state: WorkspaceState) -> bool {
    if (state.phase)() == ConnectionPhase::Connected {
        return true;
    }
    let mut callback_state = state;
    callback_state
        .error_message
        .set(Some("实时服务正在建立，请稍后发送验证码".into()));
    false
}

/// 请求 SpacetimeDB Module 直接发送验证码。
fn request_sms_code(
    phone_number: Signal<String>,
    mut sending_code: SyncSignal<bool>,
    mut sms_hint: SyncSignal<Option<String>>,
    state: WorkspaceState,
) {
    let phone_number = phone_number().trim().to_string();
    let mut callback_state = state;
    if phone_number.len() != 11 || !phone_number.bytes().all(|byte| byte.is_ascii_digit()) {
        callback_state
            .error_message
            .set(Some("请输入 11 位手机号".into()));
        return;
    }
    if !ensure_connected(state) {
        return;
    }
    callback_state.error_message.set(None);
    sending_code.set(true);
    let request_result = send_login_sms_code(phone_number, move |result| {
        match result {
            Ok(expires_in) => {
                sms_hint.set(Some(format!("验证码已发送，{expires_in} 秒内有效")));
            }
            Err(error) => callback_state.error_message.set(Some(error)),
        }
        sending_code.set(false);
    });
    if let Err(error) = request_result {
        callback_state.error_message.set(Some(error));
        sending_code.set(false);
    }
}

/// 校验短信验证码并兑换为 SpacetimeDB 业务会话。
fn submit_sms_login(
    phone_number: Signal<String>,
    code: Signal<String>,
    remember_me: Signal<bool>,
    state: WorkspaceState,
) {
    let phone_number = phone_number().trim().to_string();
    let code = code().trim().to_string();
    let mut callback_state = state;
    if phone_number.len() != 11 || !phone_number.bytes().all(|byte| byte.is_ascii_digit()) {
        callback_state
            .error_message
            .set(Some("请输入 11 位手机号".into()));
        return;
    }
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        callback_state
            .error_message
            .set(Some("请输入 6 位短信验证码".into()));
        return;
    }
    if !ensure_connected(state) {
        return;
    }
    if let Err(error) = login_workspace_with_sms(phone_number, code, remember_me(), state) {
        callback_state.error_message.set(Some(error));
    }
}

/// 连接异常时重新建立底层实时连接，用户无需接触服务器配置。
fn retry_connection(state: WorkspaceState) {
    let mut callback_state = state;
    spawn(async move {
        if let Err(error) = connect_workspace(ConnectionConfig::default(), callback_state).await {
            callback_state.phase.set(ConnectionPhase::Failed);
            callback_state.error_message.set(Some(error));
        }
    });
}

#[component]
pub fn LoginPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let saved_account = load_saved_account();
    let mut mode = use_signal(|| LoginMode::Password);
    let mut account = use_signal(|| saved_account.clone().unwrap_or_default());
    let mut password = use_signal(String::new);
    let mut phone_number = use_signal(String::new);
    let mut sms_code = use_signal(String::new);
    // Procedure 回调可能从 SDK 工作线程返回，因此使用线程安全信号。
    let sending_code = use_signal_sync(|| false);
    let sms_hint = use_signal_sync(|| None::<String>);
    let mut remember_me = use_signal(|| saved_account.is_some());
    let phase = (state.phase)();
    let authenticating = (state.authenticating)();
    let busy = authenticating;

    rsx! {
        main { class: "auth",
            div { class: "auth-card",
                header { class: "auth-brand",
                    BrandLogo { size: LogoSize::Large }
                    div { class: "brand-copy",
                        strong { "云园慧控" }
                        small { "园区运营工作台" }
                    }
                }

                form {
                    class: "auth-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        if !busy {
                            match mode() {
                                LoginMode::Password => submit_password_login(account, password, remember_me, state),
                                LoginMode::Sms => submit_sms_login(phone_number, sms_code, remember_me, state),
                            }
                        }
                    },

                    div { class: "auth-tabs", role: "tablist", aria_label: "登录方式",
                        button {
                            class: if mode() == LoginMode::Password { "auth-tab is-active" } else { "auth-tab" },
                            r#type: "button",
                            role: "tab",
                            aria_selected: mode() == LoginMode::Password,
                            onclick: move |_| {
                                mode.set(LoginMode::Password);
                                let mut callback_state = state;
                                callback_state.error_message.set(None);
                            },
                            "密码登录"
                        }
                        button {
                            class: if mode() == LoginMode::Sms { "auth-tab is-active" } else { "auth-tab" },
                            r#type: "button",
                            role: "tab",
                            aria_selected: mode() == LoginMode::Sms,
                            onclick: move |_| {
                                mode.set(LoginMode::Sms);
                                let mut callback_state = state;
                                callback_state.error_message.set(None);
                            },
                            "短信验证码"
                        }
                    }

                    if mode() == LoginMode::Password {
                        div { class: "field",
                            Label { html_for: "login-account", "账号或手机号" }
                            Input {
                                id: "login-account",
                                value: account,
                                autocomplete: "username",
                                placeholder: "请输入账号或手机号",
                                disabled: busy,
                                oninput: move |event: FormEvent| account.set(event.value()),
                            }
                        }
                        div { class: "field",
                            Label { html_for: "login-password", "密码" }
                            Input {
                                id: "login-password",
                                r#type: "password",
                                value: password,
                                autocomplete: "current-password",
                                placeholder: "请输入密码",
                                disabled: busy,
                                oninput: move |event: FormEvent| password.set(event.value()),
                            }
                        }
                    } else {
                        div { class: "field",
                            Label { html_for: "login-phone", "手机号" }
                            Input {
                                id: "login-phone",
                                value: phone_number,
                                inputmode: "numeric",
                                maxlength: "11",
                                autocomplete: "tel",
                                placeholder: "请输入 11 位手机号",
                                disabled: busy,
                                oninput: move |event: FormEvent| phone_number.set(event.value()),
                            }
                        }
                        div { class: "field",
                            Label { html_for: "login-code", "短信验证码" }
                            div { class: "auth-code-row",
                                Input {
                                    id: "login-code",
                                    value: sms_code,
                                    inputmode: "numeric",
                                    maxlength: "6",
                                    autocomplete: "one-time-code",
                                    placeholder: "6 位验证码",
                                    disabled: busy,
                                    oninput: move |event: FormEvent| sms_code.set(event.value()),
                                }
                                Button {
                                    variant: ButtonVariant::Outline,
                                    r#type: "button",
                                    disabled: busy || sending_code(),
                                    onclick: move |_| request_sms_code(phone_number, sending_code, sms_hint, state),
                                    if sending_code() { "发送中" } else { "发送验证码" }
                                }
                            }
                        }
                        if let Some(hint) = sms_hint.read().as_ref() {
                            p { class: "hint", "{hint}" }
                        }
                    }

                    label { class: "auth-remember",
                        input {
                            r#type: "checkbox",
                            checked: remember_me,
                            disabled: busy,
                            onchange: move |event| remember_me.set(event.checked()),
                        }
                        span { "保持 30 天登录" }
                    }

                    if let Some(error) = state.error_message.read().as_ref() {
                        p { class: "form-error", role: "alert", "{error}" }
                    }

                    if phase == ConnectionPhase::Failed {
                        Button {
                            r#type: "button",
                            onclick: move |_| retry_connection(state),
                            "重新连接实时服务"
                        }
                    } else {
                        Button {
                            r#type: "submit",
                            disabled: busy,
                            if busy {
                                "正在验证登录"
                            } else if mode() == LoginMode::Sms {
                                "验证并进入"
                            } else {
                                "登录并进入"
                            }
                        }
                    }
                }

                footer { class: "auth-foot",
                    ConnectivityStatus { phase, compact: true }
                    span { "密码与短信双认证 · 多设备会话 · 实时权限同步" }
                }
            }
        }
    }
}
