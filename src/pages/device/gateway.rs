//! 边缘计算设备管理页。
//!
//! 边缘设备是装在园区现场那台 Windows 电脑上的常驻程序，直接作为 SpacetimeDB
//! 客户端接入。这一页管三件事：建档发码、看它此刻死活、看那台电脑健不健康。
//!
//! **电脑是客户的**，所以健康那一列不是锦上添花——机器被当办公机用、硬盘塞满、
//! 被杀毒软件拦，这些都不是我们能控制的，但必须能看见，否则每次故障都会先赖
//! 到我们头上。设计见 `docs/边缘计算设备与摄像头接入.md`。

use dioxus::prelude::*;

use super::model::*;
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        ConfirmDialog,
    },
    services::{
        delete_edge_gateway_record, regenerate_edge_registration_code_record,
        save_edge_gateway_record,
    },
    spacetime_bindings::{
        edge_gateway_input_type::EdgeGatewayInput, edge_gateway_type::EdgeGateway, park_type::Park,
    },
    state::WorkspaceState,
};

#[component]
pub fn DeviceGatewayPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let mut feedback = use_signal_sync(|| None::<String>);
    // Some(None) = 新增，Some(Some(gateway)) = 编辑。
    let mut form = use_signal(|| None::<Option<EdgeGateway>>);
    let mut confirm_delete = use_signal(|| None::<EdgeGateway>);
    let mut confirm_regenerate = use_signal(|| None::<EdgeGateway>);
    let pending = use_signal_sync(|| false);

    let parks = state.parks.read().clone();
    let gateways = state.edge_gateways.read().clone();
    // 时钟只在这一层读一次，往下传的是数值——model 层保持可脱离运行环境单测。
    let now_micros = now_micros();

    let total = gateways.len();
    let online = gateways.iter().filter(|row| row.is_online).count();
    let attention = gateways
        .iter()
        .filter(|row| health_needs_attention(&row.health_level))
        .count();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "边缘计算设备" }
                    p { class: "page-subtitle",
                        "边缘计算设备即园区现场的一台 Windows 计算机，安装本系统的常驻程序后即可承担该职责，无需另行采购硬件。摄像头接入该设备，识别在本地完成，视频流不出园区，平台仅接收结构化事件。"
                    }
                }
                div { class: "page-actions",
                    Button {
                        r#type: "button",
                        onclick: move |_| {
                            feedback.set(None);
                            form.set(Some(None));
                        },
                        "新增设备"
                    }
                }
            }

            if let Some(message) = feedback() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-3", aria_label: "边缘计算设备总览",
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "在册设备" }
                    strong { class: "stat-value is-mono", "{total}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "当前在线" }
                    strong { class: "stat-value is-mono", "{online}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "健康异常" }
                    strong {
                        class: if attention > 0 { "stat-value is-mono is-bad" } else { "stat-value is-mono" },
                        "{attention}"
                    }
                } } } }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "设备台账" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 台" }
                }
                if gateways.is_empty() {
                    p { class: "empty",
                        "暂无边缘设备。请先在此建档并获取注册码，再于园区计算机上安装程序并输入该注册码完成激活。"
                    }
                } else {
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "边缘设备" }
                                    th { "园区" }
                                    th { "连接" }
                                    th { "机器健康" }
                                    th { "接入" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                for row in gateways.iter().cloned() {
                                    tr { key: "{row.gateway_id}",
                                        td {
                                            div { class: "stack-tight",
                                                strong { "{row.gateway_name}" }
                                                small { class: "hint",
                                                    match row.agent_version.as_deref() {
                                                        Some(version) => format!("程序版本 {version}"),
                                                        None => "尚未装机".to_string(),
                                                    }
                                                }
                                            }
                                        }
                                        td { "{park_label(&parks, row.park_id)}" }
                                        td {
                                            div { class: "stack-tight",
                                                if row.is_online {
                                                    Badge { variant: BadgeVariant::Secondary, "在线" }
                                                } else {
                                                    Badge { variant: BadgeVariant::Outline, "离线" }
                                                }
                                                small { class: "hint",
                                                    "已 {elapsed_label(row.status_changed_at.to_micros_since_unix_epoch(), now_micros)}"
                                                }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                Badge { variant: health_variant(&row.health_level),
                                                    "{health_label(&row.health_level)}"
                                                }
                                                if let Some(detail) = row.health_detail.clone() {
                                                    small { class: "hint", "{detail}" }
                                                }
                                            }
                                        }
                                        td {
                                            match row.registration_code.clone() {
                                                Some(code) => rsx! {
                                                    div { class: "stack-tight",
                                                        strong { class: "is-mono", "{code}" }
                                                        small { class: "hint", "待装机，请使用此码激活" }
                                                    }
                                                },
                                                None => rsx! {
                                                    div { class: "stack-tight",
                                                        Badge { variant: BadgeVariant::Secondary, "已激活" }
                                                        small { class: "hint", "身份已绑定至该计算机" }
                                                    }
                                                },
                                            }
                                        }
                                        td {
                                            div { class: "table-actions",
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let row = row.clone();
                                                        move |_| {
                                                            feedback.set(None);
                                                            form.set(Some(Some(row.clone())));
                                                        }
                                                    },
                                                    "编辑"
                                                }
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let row = row.clone();
                                                        move |_| confirm_regenerate.set(Some(row.clone()))
                                                    },
                                                    "重发注册码"
                                                }
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    class: "is-quiet-danger",
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let row = row.clone();
                                                        move |_| confirm_delete.set(Some(row.clone()))
                                                    },
                                                    "注销"
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

            section { class: "section",
                Card { CardContent {
                    p { class: "hint",
                        "「连接」反映边缘设备与数据库之间连接的实时状态，由连接的建立与断开直接触发，不依赖心跳推断，因此不存在判定延迟。"
                    }
                    p { class: "hint",
                        "「机器健康」反映该计算机自身的运行状况，包含磁盘、负载与摄像头在线情况。设备归客户所有，此列用于在故障发生时区分设备侧与平台侧的责任归属。"
                    }
                } }
            }
        }

        if let Some(editing) = form() {
            GatewayDialog {
                gateway: editing,
                parks: parks.clone(),
                on_close: move |_| form.set(None),
                on_saved: move |_| {
                    form.set(None);
                    feedback.set(Some("已保存，注册码请在台账中查看".into()));
                },
            }
        }

        if let Some(row) = confirm_regenerate() {
            ConfirmDialog {
                title: "重发注册码并解绑当前计算机？",
                description: "原计算机将立即失去身份，无法继续上报数据；新注册码 24 小时内有效。适用于更换计算机、重装系统，或该计算机不再可信的情形。",
                confirm_label: if pending() { "处理中…" } else { "确认重发" },
                on_cancel: move |_| confirm_regenerate.set(None),
                on_confirm: move |_| {
                    if pending() {
                        return;
                    }
                    let gateway_id = row.gateway_id;
                    let mut feedback = feedback;
                    let mut confirm_regenerate = confirm_regenerate;
                    let mut pending = pending;
                    pending.set(true);
                    spawn(async move {
                        let result = regenerate_edge_registration_code_record(gateway_id).await;
                        pending.set(false);
                        confirm_regenerate.set(None);
                        feedback.set(Some(match result {
                            Ok(()) => "新注册码已生成，原计算机已解绑".into(),
                            Err(error) => error,
                        }));
                    });
                },
            }
        }

        if let Some(row) = confirm_delete() {
            ConfirmDialog {
                title: "确认注销这台设备？",
                description: "此操作为逻辑删除：设备将从台账中隐藏，身份一并解绑，该计算机立即失去上报能力。数据不会被物理删除。",
                confirm_label: if pending() { "注销中…" } else { "确认注销" },
                on_cancel: move |_| confirm_delete.set(None),
                on_confirm: move |_| {
                    if pending() {
                        return;
                    }
                    let gateway_id = row.gateway_id;
                    let mut feedback = feedback;
                    let mut confirm_delete = confirm_delete;
                    let mut pending = pending;
                    pending.set(true);
                    spawn(async move {
                        let result = delete_edge_gateway_record(gateway_id).await;
                        pending.set(false);
                        confirm_delete.set(None);
                        feedback.set(Some(match result {
                            Ok(()) => "边缘设备已注销".into(),
                            Err(error) => error,
                        }));
                    });
                },
            }
        }
    }
}

#[component]
fn GatewayDialog(
    gateway: Option<EdgeGateway>,
    parks: Vec<Park>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let gateway_id = gateway.as_ref().map(|row| row.gateway_id);
    let mut name = use_signal_sync(|| {
        gateway
            .as_ref()
            .map(|row| row.gateway_name.clone())
            .unwrap_or_default()
    });
    let mut remark = use_signal_sync(|| {
        gateway
            .as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    let mut park_id = use_signal_sync(|| {
        gateway
            .as_ref()
            .map(|row| row.park_id.to_string())
            .unwrap_or_default()
    });
    let mut loading = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();

    rsx! {
        Dialog {
            open: Some(true),
            is_modal: true,
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { if gateway_id.is_some() { "编辑边缘计算设备" } else { "新增边缘计算设备" } }
            DialogDescription {
                "建档后将生成一个 24 小时内有效的注册码，由装机人员在园区计算机上输入以完成激活。"
            }
            div { class: "stack",
                div { class: "form-grid",
                    div { class: "field",
                        Label { html_for: "gateway-park", "所属园区 *" }
                        Select {
                            id: "gateway-park",
                            value: Some(park_value),
                            on_value_change: move |value: Option<String>| park_id.set(value.unwrap_or_default()),
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "gateway-park-{park.park_id}",
                                    value: park.park_id.to_string(),
                                    index: index + 1,
                                    text_value: park.park_name.to_string(),
                                    "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "gateway-name", "设备名称 *" }
                        Input {
                            id: "gateway-name",
                            value: name(),
                            maxlength: 50,
                            placeholder: "例如：北区值班室电脑",
                            oninput: move |event: FormEvent| name.set(event.value()),
                        }
                        small { class: "hint", "园区内唯一。现场排查时以此识别设备，建议注明所在房间。" }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "gateway-remark", "备注" }
                        Textarea {
                            id: "gateway-remark",
                            maxlength: 200,
                            rows: 2,
                            value: remark(),
                            placeholder: "计算机配置、联系人、装机日期等",
                            oninput: move |event: FormEvent| remark.set(event.value()),
                        }
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
                        r#type: "button",
                        disabled: loading(),
                        onclick: move |_| {
                            if loading() {
                                return;
                            }
                            let park_id = match park_id().parse::<u64>() {
                                Ok(value) if value != 0 => value,
                                _ => {
                                    error.set(Some("请选择所属园区".into()));
                                    return;
                                }
                            };
                            let name_value = name().trim().to_string();
                            if name_value.is_empty() {
                                error.set(Some("请输入设备名称".into()));
                                return;
                            }
                            let input = EdgeGatewayInput {
                                park_id,
                                gateway_name: name_value,
                                remark: (!remark().trim().is_empty())
                                    .then(|| remark().trim().to_string()),
                            };
                            error.set(None);
                            loading.set(true);
                            spawn(async move {
                                match save_edge_gateway_record(gateway_id, input).await {
                                    Ok(()) => {
                                        loading.set(false);
                                        on_saved.call(());
                                    }
                                    Err(message) => {
                                        loading.set(false);
                                        error.set(Some(message));
                                    }
                                }
                            });
                        },
                        if loading() { "保存中…" } else { "确认保存" }
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn now_micros() -> i64 {
    (js_sys::Date::now() * 1_000.0).min(i64::MAX as f64) as i64
}

#[cfg(not(target_arch = "wasm32"))]
fn now_micros() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_micros().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[component]
    fn GatewayRenderTestRoot() -> Element {
        let state = crate::app::use_workspace_state();
        use_context_provider(|| state);
        rsx! { DeviceGatewayPage {} }
    }

    #[test]
    fn 边缘计算设备页可以完成首次渲染() {
        let html = dioxus_ssr::render_element(rsx! { GatewayRenderTestRoot {} });
        assert!(html.contains("边缘计算设备"), "页面标题没有进入渲染结果");
        assert!(html.contains("设备台账"), "台账区块没有进入渲染结果");
    }
}
