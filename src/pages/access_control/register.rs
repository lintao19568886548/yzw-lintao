//! 门岗使用的访客快速登记页面。

use dioxus::prelude::*;

use super::{navigation::AccessNavigation, visitor_form::valid_phone};
use crate::{
    components::{
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
    },
    permissions::can_manage_access,
    services::create_access_visitor_record,
    spacetime_bindings::access_visitor_input_type::AccessVisitorInput,
    state::WorkspaceState,
};

#[component]
pub fn AccessVisitorRegisterPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let parks = (state.parks)();
    let can_manage = can_manage_access(&(state.roles)());
    let mut park_id = use_signal(|| {
        // 只有一个园区时直接选中，门岗少点一次。
        if parks.len() == 1 {
            parks[0].park_id.to_string()
        } else {
            String::new()
        }
    });
    let mut name = use_signal(String::new);
    let mut phone = use_signal(String::new);
    let mut car = use_signal(String::new);
    let mut remark = use_signal(String::new);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut success = use_signal(|| None::<String>);
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();

    if !can_manage {
        return rsx! {
            main { class: "page",
                AccessNavigation { active: String::from("register") }
                Card {
                    CardContent {
                        div { class: "stack",
                            h1 { "当前账号不能登记访客" }
                            p { class: "page-subtitle",
                                "访客登记会直接写入门禁记录，目前仅系统管理员可以执行。"
                            }
                            div { class: "card-cta",
                                Link { to: crate::router::Route::AccessVisitorPage {},
                                    Button { variant: ButtonVariant::Outline, "返回访客管理" }
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
            AccessNavigation { active: String::from("register") }

            header { class: "page-header",
                div { class: "page-title",
                    h1 { "访客登记" }
                    p { class: "page-subtitle", "核对访客身份并选择园区，提交后实时进入访客管理台账。" }
                }
            }

            if let Some(message) = success() {
                div { class: "notice", role: "status",
                    div { class: "row",
                        span { "{message}" }
                        Link { to: crate::router::Route::AccessVisitorPage {}, "查看访客台账 →" }
                    }
                }
            }

            Card {
                CardContent {
                    form {
                        onsubmit: move |event| {
                            event.prevent_default();
                            if loading() {
                                return;
                            }
                            let park = match park_id().parse::<u64>() {
                                Ok(id) if id > 0 => id,
                                _ => {
                                    error.set(Some("请选择来访园区".into()));
                                    return;
                                }
                            };
                            let visitor = name().trim().to_string();
                            if visitor.chars().count() < 2 {
                                error.set(Some("请输入至少 2 个字符的访客姓名".into()));
                                return;
                            }
                            let phone_value = phone().trim().to_string();
                            if !valid_phone(&phone_value) {
                                error.set(Some("请输入正确的 11 位手机号码".into()));
                                return;
                            }
                            let reason = remark().trim().to_string();
                            if reason.is_empty() {
                                error.set(Some("请输入来访原因".into()));
                                return;
                            }
                            if reason.chars().count() > 100 {
                                error.set(Some("来访原因不能超过 100 个字符".into()));
                                return;
                            }
                            let input = AccessVisitorInput {
                                visitor_name: visitor.clone(),
                                car_num: (!car().trim().is_empty())
                                    .then(|| car().trim().to_uppercase()),
                                phone_number: phone_value,
                                status: 0,
                                register_time: None,
                                remark: Some(reason),
                                park_id: Some(park),
                            };
                            loading.set(true);
                            error.set(None);
                            success.set(None);
                            spawn(async move {
                                match create_access_visitor_record(input).await {
                                    Ok(()) => {
                                        loading.set(false);
                                        success.set(Some(format!("访客 {visitor} 已登记为进入状态。")));
                                        // 门岗会连续登记多位访客，提交成功后清空表单。
                                        name.set(String::new());
                                        phone.set(String::new());
                                        car.set(String::new());
                                        remark.set(String::new());
                                    }
                                    Err(message) => {
                                        loading.set(false);
                                        error.set(Some(message));
                                    }
                                }
                            });
                        },
                        div { class: "form-grid",
                            div { class: "field is-wide",
                                Label { html_for: "register-park", "来访园区 *" }
                                Select {
                                    id: "register-park",
                                    value: Some(park_value),
                                    disabled: loading(),
                                    on_value_change: move |value: Option<String>| park_id.set(value.unwrap_or_default()),
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                                    for (index , park) in parks.iter().enumerate() {
                                        SelectOption::<String> {
                                            key: "register-park-{park.park_id}",
                                            value: park.park_id.to_string(),
                                            index: index + 1,
                                            text_value: park.park_name.to_string(),
                                            "{park.park_name}"
                                        }
                                    }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "register-name", "访客姓名 *" }
                                Input {
                                    id: "register-name",
                                    autocomplete: "name",
                                    value: name(),
                                    placeholder: "请输入姓名",
                                    disabled: loading(),
                                    oninput: move |event: FormEvent| name.set(event.value()),
                                }
                            }
                            div { class: "field",
                                Label { html_for: "register-phone", "手机号码 *" }
                                Input {
                                    id: "register-phone",
                                    autocomplete: "tel",
                                    inputmode: "numeric",
                                    maxlength: 11,
                                    value: phone(),
                                    placeholder: "请输入 11 位手机号",
                                    disabled: loading(),
                                    oninput: move |event: FormEvent| phone.set(event.value()),
                                }
                            }
                            div { class: "field is-wide",
                                Label { html_for: "register-car", "车牌号" }
                                Input {
                                    id: "register-car",
                                    value: car(),
                                    maxlength: 20,
                                    placeholder: "选填，例如：粤A12345",
                                    disabled: loading(),
                                    oninput: move |event: FormEvent| car.set(event.value()),
                                }
                            }
                            div { class: "field is-wide",
                                Label { html_for: "register-remark", "来访原因 *" }
                                Textarea {
                                    id: "register-remark",
                                    value: remark(),
                                    maxlength: 100,
                                    rows: 3,
                                    placeholder: "例如：拜访招商部、设备维护或送货",
                                    disabled: loading(),
                                    oninput: move |event: FormEvent| remark.set(event.value()),
                                }
                                small { class: "hint", "{remark().chars().count()}/100" }
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
                                onclick: move |_| {
                                    name.set(String::new());
                                    phone.set(String::new());
                                    car.set(String::new());
                                    remark.set(String::new());
                                    error.set(None);
                                    success.set(None);
                                },
                                "重置"
                            }
                            Button {
                                r#type: "submit",
                                disabled: loading(),
                                if loading() {
                                    "提交中…"
                                } else {
                                    "提交登记"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
