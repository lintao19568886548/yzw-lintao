//! 车辆出入记录表单。

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
    services::{create_access_car_record, park_ref, update_access_car_record},
    spacetime_bindings::{
        access_car_input_type::AccessCarInput, access_car_type::AccessCar, park_type::Park,
    },
};

#[component]
pub(super) fn CarFormDialog(
    record: Option<AccessCar>,
    parks: Vec<Park>,
    readonly: bool,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let record_id = record.as_ref().map(|row| row.car_id);
    let mut car_number = use_signal(|| {
        record
            .as_ref()
            .map(|row| row.car_number.clone())
            .unwrap_or_default()
    });
    let mut status = use_signal(|| record.as_ref().map(|row| row.status).unwrap_or(1));
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
        "查看车辆记录"
    } else if record_id.is_some() {
        "编辑车辆记录"
    } else {
        "新增车辆记录"
    };
    // 登记时间在表单里是 `YYYY-MM-DDTHH:MM`，拆成日期和时间两个控件。
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
            DialogDescription { "登记车辆进入或离开园区的时间与状态。" }

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
                    let number = car_number().trim().to_uppercase();
                    if number.chars().count() < 2 || number.chars().count() > 20 {
                        error.set(Some("车牌号长度应为 2 至 20 个字符".into()));
                        return;
                    }
                    if remark().chars().count() > 200 {
                        error.set(Some("备注不能超过 200 个字符".into()));
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
                    let input = AccessCarInput {
                        car_number: number,
                        status: status(),
                        register_time: registered,
                        remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                        park_id: park,
                    };
                    loading.set(true);
                    error.set(None);
                    spawn(async move {
                        let result = if let Some(id) = record_id {
                            update_access_car_record(id, input).await
                        } else {
                            create_access_car_record(input).await
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
                        Label { html_for: "car-form-number", "车牌号 *" }
                        Input {
                            id: "car-form-number",
                            value: car_number(),
                            readonly,
                            placeholder: "例如：粤A12345",
                            maxlength: 20,
                            oninput: move |event: FormEvent| car_number.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "car-form-status", "出入状态" }
                        Select {
                            id: "car-form-status",
                            value: Some(status_value),
                            disabled: readonly,
                            on_value_change: move |value: Option<String>| {
                                status.set(value.and_then(|value| value.parse().ok()).unwrap_or(1));
                            },
                            SelectOption::<String> { value: "1".to_string(), index: 0usize, text_value: "进入".to_string(), "进入" }
                            SelectOption::<String> { value: "0".to_string(), index: 1usize, text_value: "离开".to_string(), "离开" }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "car-form-park", "所属园区" }
                        Select {
                            id: "car-form-park",
                            value: Some(park_value),
                            disabled: readonly,
                            on_value_change: move |value: Option<String>| park_id.set(value.unwrap_or_default()),
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "car-form-park-{park.park_id}",
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
                        Label { html_for: "car-form-remark", "备注" }
                        Textarea {
                            id: "car-form-remark",
                            value: remark(),
                            readonly,
                            maxlength: 200,
                            rows: 3,
                            placeholder: "补充车辆、来访或通行说明",
                            oninput: move |event: FormEvent| remark.set(event.value()),
                        }
                        small { class: "hint", "{remark().chars().count()}/200" }
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
