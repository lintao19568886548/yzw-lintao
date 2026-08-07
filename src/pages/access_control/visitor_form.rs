//! 访客记录新增、查看与编辑表单。

use dioxus::prelude::*;

use super::model::{datetime_input, parse_datetime};
use crate::{
    components::{
        button::{Button, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        DateField, TimeField,
    },
    services::{create_access_visitor_record, park_ref, update_access_visitor_record},
    spacetime_bindings::{
        access_visitor_input_type::AccessVisitorInput, access_visitor_type::AccessVisitor,
        park_type::Park,
    },
};

pub(super) fn valid_phone(value: &str) -> bool {
    let value = value.trim();
    value.len() == 11
        && value.starts_with('1')
        && value
            .as_bytes()
            .get(1)
            .is_some_and(|v| matches!(v, b'3'..=b'9'))
        && value.bytes().all(|v| v.is_ascii_digit())
}

#[component]
pub(super) fn VisitorFormDialog(
    record: Option<AccessVisitor>,
    parks: Vec<Park>,
    readonly: bool,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let record_id = record.as_ref().map(|row| row.visitor_id);
    let mut name = use_signal(|| {
        record
            .as_ref()
            .map(|row| row.visitor_name.clone())
            .unwrap_or_default()
    });
    let mut phone = use_signal(|| {
        record
            .as_ref()
            .map(|row| row.phone_number.clone())
            .unwrap_or_default()
    });
    let mut car_num = use_signal(|| {
        record
            .as_ref()
            .and_then(|row| row.car_num.clone())
            .unwrap_or_default()
    });
    let mut status = use_signal(|| record.as_ref().map(|row| row.status).unwrap_or(0));
    let mut register_time = use_signal(|| {
        record
            .as_ref()
            .map(|row| datetime_input(row.register_time))
            .unwrap_or_default()
    });
    let mut park_id = use_signal(|| {
        record
            .as_ref()
            .and_then(|row| park_ref(row.park_id))
            .map(|id| id.to_string())
            .unwrap_or_default()
    });
    let mut remark = use_signal(|| {
        record
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
    let title = if readonly {
        "查看访客记录"
    } else if record_id.is_some() {
        "编辑访客记录"
    } else {
        "新增访客记录"
    };
    let register_date = use_memo(move || {
        register_time()
            .split('T')
            .next()
            .unwrap_or_default()
            .to_string()
    });
    let register_clock = use_memo(move || {
        register_time()
            .split('T')
            .nth(1)
            .unwrap_or_default()
            .to_string()
    });
    let status_value: ReadSignal<Option<String>> =
        use_memo(move || Some(status().to_string())).into();
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { "{title}" }
            DialogDescription { "记录访客身份、到访原因与通行状态。" }

            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    if readonly {
                        on_close.call(());
                        return;
                    }
                    if loading() {
                        return;
                    }
                    let name_value = name().trim().to_string();
                    if name_value.chars().count() < 2 || name_value.chars().count() > 50 {
                        error.set(Some("姓名长度应为 2 至 50 个字符".into()));
                        return;
                    }
                    let phone_value = phone().trim().to_string();
                    if !valid_phone(&phone_value) {
                        error.set(Some("请输入正确的 11 位手机号码".into()));
                        return;
                    }
                    if car_num().chars().count() > 20 {
                        error.set(Some("车牌号不能超过 20 个字符".into()));
                        return;
                    }
                    if remark().chars().count() > 100 {
                        error.set(Some("来访原因不能超过 100 个字符".into()));
                        return;
                    }
                    let registered = if register_time().trim().is_empty() {
                        None
                    } else {
                        match parse_datetime(&register_time()) {
                            Ok(value) => Some(value),
                            Err(message) => {
                                error.set(Some(message));
                                return;
                            }
                        }
                    };
                    let park = park_id().parse::<u64>().ok().filter(|id| *id > 0);
                    let input = AccessVisitorInput {
                        visitor_name: name_value,
                        car_num: (!car_num().trim().is_empty())
                            .then(|| car_num().trim().to_uppercase()),
                        phone_number: phone_value,
                        status: status(),
                        register_time: registered,
                        remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                        park_id: park,
                    };
                    loading.set(true);
                    error.set(None);
                    spawn(async move {
                        let result = if let Some(id) = record_id {
                            update_access_visitor_record(id, input).await
                        } else {
                            create_access_visitor_record(input).await
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
                        Label { html_for: "visitor-form-name", "访客姓名 *" }
                        Input {
                            id: "visitor-form-name",
                            value: name(),
                            readonly,
                            placeholder: "请输入姓名",
                            maxlength: 50,
                            oninput: move |event: FormEvent| name.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "visitor-form-phone", "手机号码 *" }
                        Input {
                            id: "visitor-form-phone",
                            inputmode: "numeric",
                            value: phone(),
                            readonly,
                            placeholder: "请输入 11 位手机号",
                            maxlength: 11,
                            oninput: move |event: FormEvent| phone.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "visitor-form-car", "车牌号" }
                        Input {
                            id: "visitor-form-car",
                            value: car_num(),
                            readonly,
                            placeholder: "选填，例如：粤A12345",
                            maxlength: 20,
                            oninput: move |event: FormEvent| car_num.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "visitor-form-status", "访问状态" }
                        Select {
                            id: "visitor-form-status",
                            value: Some(status_value),
                            disabled: readonly,
                            on_value_change: move |value: Option<String>| {
                                status.set(value.and_then(|value| value.parse().ok()).unwrap_or(0));
                            },
                            SelectOption::<String> { value: "0".to_string(), index: 0usize, text_value: "进入".to_string(), "进入" }
                            SelectOption::<String> { value: "1".to_string(), index: 1usize, text_value: "离开".to_string(), "离开" }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "visitor-form-park", "所属园区" }
                        Select {
                            id: "visitor-form-park",
                            value: Some(park_value),
                            disabled: readonly,
                            on_value_change: move |value: Option<String>| park_id.set(value.unwrap_or_default()),
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "visitor-form-park-{park.park_id}",
                                    value: park.park_id.to_string(),
                                    index: index + 1,
                                    text_value: park.park_name.to_string(),
                                    "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "登记时间" }
                        div { class: "row",
                            DateField {
                                value: register_date(),
                                disabled: readonly,
                                on_change: move |value: String| {
                                    register_time.set(format!("{value}T{}", register_clock()))
                                },
                            }
                            TimeField {
                                value: register_clock(),
                                disabled: readonly,
                                on_change: move |value: String| {
                                    register_time.set(format!("{}T{value}", register_date()))
                                },
                            }
                        }
                        if record_id.is_none() {
                            small { class: "hint", "留空则使用服务器当前时间" }
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "visitor-form-remark", "来访原因" }
                        Textarea {
                            id: "visitor-form-remark",
                            value: remark(),
                            readonly,
                            maxlength: 100,
                            rows: 3,
                            placeholder: "请输入来访原因",
                            oninput: move |event: FormEvent| remark.set(event.value()),
                        }
                        small { class: "hint", "{remark().chars().count()}/100" }
                    }
                }

                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", "{message}" }
                }

                div { class: "form-actions",
                    if readonly {
                        Button { r#type: "submit", "完成" }
                    } else {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn 手机号格式与服务端一致() {
        assert!(valid_phone("13800138000"));
        assert!(!valid_phone("12800138000"));
    }
}
