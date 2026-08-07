//! 园区详情页的水电表台账区块。
//!
//! 表按园区列而不是嵌在每一层的表格行里，是因为并非每块表都属于某一层：
//! 生产账单里「公共用电」有 299 条，那些表装在园区公共区域。按层展示会把
//! 它们整批藏起来，园区里到底有几块表也就没有一个地方能看全。

use dioxus::prelude::*;

use super::asset_form::{decimal_input, parse_decimal};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent, CardHeader, CardTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        ConfirmDialog,
    },
    pages::meter_location_label,
    permissions::can_manage_rental,
    router::Route,
    services::{
        create_utility_meter_record, delete_utility_meter_record, update_utility_meter_record,
    },
    spacetime_bindings::{
        dormitory_floor_type::DormitoryFloor, dormitory_type::Dormitory,
        factory_floor_type::FactoryFloor, factory_type::Factory,
        utility_meter_input_type::UtilityMeterInput, utility_meter_type::UtilityMeter,
    },
    state::WorkspaceState,
};

/// 安装位置下拉框的取值编码。
///
/// 厂房楼层和宿舍楼层的自增主键互相独立，1 号厂房层和 1 号宿舍层会撞号，
/// 所以下拉值必须带上类型前缀，不能只放一个裸 id。
pub(crate) const PUBLIC_AREA: &str = "public";

#[component]
pub(super) fn MeterSection(park_id: u64) -> Element {
    let state = use_context::<WorkspaceState>();
    let can_edit = can_manage_rental(&(state.roles)(), &(state.menus)());
    let mut adding = use_signal_sync(|| false);

    let mut meters = (state.utility_meters)()
        .into_iter()
        .filter(|meter| !meter.is_deleted && meter.park_id == park_id)
        .collect::<Vec<_>>();
    // 电表在前、水表在后，同类按表号排——抄表和对账都按表号认表。
    meters.sort_by(|left, right| {
        right
            .is_electric
            .cmp(&left.is_electric)
            .then(left.meter_code.cmp(&right.meter_code))
    });

    let links = (state.rental_tenant_meters)();
    let tenants = (state.rental_tenants)();
    let electric_count = meters.iter().filter(|meter| meter.is_electric).count();
    let water_count = meters.len() - electric_count;

    rsx! {
        div { class: "section",
            Card {
                CardHeader {
                    div { class: "section-header",
                        CardTitle { "水电表" }
                        div { class: "section-tools",
                            Badge { variant: BadgeVariant::Secondary, "电 {electric_count} · 水 {water_count}" }
                            if can_edit && !adding() {
                                Button { size: ButtonSize::Sm, r#type: "button", onclick: move |_| adding.set(true), "新增水电表" }
                            }
                        }
                    }
                }
                if adding() {
                    MeterInlineForm { park_id, meter: None, on_done: move |_| adding.set(false) }
                }
                CardContent {
                    if meters.is_empty() && !adding() {
                        p { class: "empty", "暂无水电表台账" }
                        p { class: "hint", "登记表之后，签合同时就能直接勾选谁用哪块表、按什么单价结算。" }
                    } else if !meters.is_empty() {
                        div { class: "table-scroll",
                            table { class: "table",
                                thead { tr {
                                    th { "表号" } th { "类型" } th { "安装位置" }
                                    th { "倍率" } th { "计价" } th { "在用合同" }
                                    if can_edit { th { "操作" } }
                                } }
                                tbody {
                                    for meter in meters.iter() {
                                        {
                                            let meter_id = meter.meter_id;
                                            // 「在用」只数未删除的合同：合同注销后关联行会被服务端一并清掉，
                                            // 这里再挡一层，避免脏数据把表锁成不可删除。
                                            let holders = links
                                                .iter()
                                                .filter(|link| link.meter_id == meter_id)
                                                .filter_map(|link| tenants.iter().find(|row| row.rental_tenant_id == link.rental_tenant_id))
                                                .filter(|row| !row.is_deleted)
                                                .map(|row| row.tenant_name.clone())
                                                .collect::<Vec<_>>();
                                            let location = meter_location_label(
                                                meter,
                                                &(state.factories)(),
                                                &(state.factory_floors)(),
                                                &(state.dormitories)(),
                                                &(state.dormitory_floors)(),
                                            );
                                            rsx! {
                                                tr { key: "meter-{meter_id}",
                                                    td { class: "is-mono", "{meter.meter_code}" }
                                                    td { if meter.is_electric { "电表" } else { "水表" } }
                                                    td { "{location}" }
                                                    td { class: "is-mono", "×{decimal_input(Some(meter.multiplier_centi))}" }
                                                    td {
                                                        if meter.is_time_of_use { "分时（尖峰平谷）" } else { "单一单价" }
                                                    }
                                                    td {
                                                        if holders.is_empty() {
                                                            span { class: "hint", "空闲" }
                                                        } else {
                                                            "{holders.join(\"、\")}"
                                                        }
                                                    }
                                                    if can_edit {
                                                        td {
                                                            MeterRowActions { meter: meter.clone(), holders: holders.len() }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        p { class: "hint", "单价不在这里填——同一块表换一户就换一个价，报价属于合同条款。" }
                    }
                }
            }
        }
    }
}

/// 单块表的编辑与删除。
///
/// 已被合同引用的表不允许删除——服务端也会挡，这里提前把原因说清楚。
#[component]
fn MeterRowActions(meter: UtilityMeter, holders: usize) -> Element {
    let mut editing = use_signal_sync(|| false);
    let mut confirming = use_signal_sync(|| false);
    let mut deleting = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let meter_id = meter.meter_id;
    let park_id = meter.park_id;

    rsx! {
        if editing() {
            MeterInlineForm { park_id, meter: Some(meter.clone()), on_done: move |_| editing.set(false) }
        } else {
            div { class: "row-tight",
                Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, r#type: "button", onclick: move |_| editing.set(true), "编辑" }
                if holders == 0 {
                    Button { variant: ButtonVariant::Outline, class: "is-quiet-danger", size: ButtonSize::Sm, r#type: "button", onclick: move |_| confirming.set(true), "删除" }
                } else {
                    span { class: "hint", "{holders} 份合同在用，不可删除" }
                }
                if confirming() {
                    ConfirmDialog {
                        title: "删除该水电表？",
                        description: "表会被归档，已开出的账单不受影响。",
                        confirm_label: "确认删除",
                        busy: deleting(),
                        error: error(),
                        on_cancel: move |_| confirming.set(false),
                        on_confirm: move |_| {
                            deleting.set(true);
                            error.set(None);
                            spawn(async move {
                                match delete_utility_meter_record(meter_id).await {
                                    Ok(()) => { deleting.set(false); confirming.set(false); }
                                    Err(message) => { deleting.set(false); error.set(Some(message)); }
                                }
                            });
                        },
                    }
                }
            }
        }
        if let Some(message) = error() { p { class: "form-error", "{message}" } }
    }
}

#[component]
fn MeterInlineForm(park_id: u64, meter: Option<UtilityMeter>, on_done: EventHandler<()>) -> Element {
    let state = use_context::<WorkspaceState>();
    let meter_id = meter.as_ref().map(|row| row.meter_id);
    let mut code = use_signal_sync(|| {
        meter
            .as_ref()
            .map(|row| row.meter_code.clone())
            .unwrap_or_default()
    });
    let mut is_electric = use_signal_sync(|| meter.as_ref().is_none_or(|row| row.is_electric));
    let mut location = use_signal_sync(|| {
        meter
            .as_ref()
            .map(|row| encode_location(row.factory_floor_id, row.dormitory_floor_id))
            .unwrap_or_else(|| PUBLIC_AREA.to_string())
    });
    let mut multiplier = use_signal_sync(|| {
        meter
            .as_ref()
            .map(|row| decimal_input(Some(row.multiplier_centi)))
            .unwrap_or_else(|| "1".to_string())
    });
    let mut time_of_use = use_signal_sync(|| meter.as_ref().is_some_and(|row| row.is_time_of_use));
    let mut device_id = use_signal_sync(|| {
        meter
            .as_ref()
            .and_then(|row| row.external_device_id.clone())
            .unwrap_or_default()
    });
    let mut remark = use_signal_sync(|| {
        meter
            .as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    let mut loading = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);

    // 安装位置候选：本园区的厂房楼层与宿舍楼层，外加"公共区域"。
    let options = meter_location_options(
        park_id,
        &(state.factories)(),
        &(state.factory_floors)(),
        &(state.dormitories)(),
        &(state.dormitory_floors)(),
    );

    let kind_value: ReadSignal<Option<String>> =
        use_memo(move || Some(if is_electric() { "e" } else { "w" }.to_string())).into();
    let location_value: ReadSignal<Option<String>> = use_memo(move || Some(location())).into();
    let title = if meter_id.is_some() {
        "编辑水电表"
    } else {
        "新增水电表"
    };

    rsx! {
        form { class: "inline-form",
            onsubmit: move |event| {
                event.prevent_default();
                if loading() { return; }
                let code_value = code().trim().to_string();
                if code_value.is_empty() { error.set(Some("请填写表号".into())); return; }
                let multiplier_value = match parse_decimal(&multiplier(), "倍率", true) {
                    Ok(Some(value)) if value > 0 => value,
                    Ok(_) => { error.set(Some("倍率必须大于零".into())); return; }
                    Err(message) => { error.set(Some(message)); return; }
                };
                let (factory_floor_id, dormitory_floor_id) = decode_location(&location());
                let input = UtilityMeterInput {
                    park_id,
                    meter_code: code_value,
                    is_electric: is_electric(),
                    factory_floor_id,
                    dormitory_floor_id,
                    multiplier_centi: multiplier_value,
                    // 水表不分时：勾选状态留在界面上没意义，服务端也会强制关掉。
                    is_time_of_use: is_electric() && time_of_use(),
                    external_device_id: (!device_id().trim().is_empty()).then(|| device_id().trim().to_string()),
                    remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                };
                error.set(None);
                loading.set(true);
                spawn(async move {
                    let result = match meter_id {
                        Some(id) => update_utility_meter_record(id, input).await,
                        None => create_utility_meter_record(input).await,
                    };
                    loading.set(false);
                    match result {
                        Ok(()) => on_done.call(()),
                        Err(message) => error.set(Some(message)),
                    }
                });
            },
            h4 { "{title}" }
            div { class: "form-grid",
                div { class: "field",
                    Label { html_for: "meter-f1", "表号" }
                    Input { id: "meter-f1", value: code(), placeholder: "如 A栋-1F-电01", oninput: move |event: FormEvent| code.set(event.value()) }
                }
                div { class: "field",
                    Label { html_for: "meter-kind", "类型" }
                    Select {
                        id: "meter-kind",
                        value: kind_value,
                        on_value_change: move |value: Option<String>| {
                            is_electric.set(value.as_deref() != Some("w"));
                        },
                        SelectOption::<String> { value: "e".to_string(), index: 0usize, text_value: "电表".to_string(), "电表" }
                        SelectOption::<String> { value: "w".to_string(), index: 1usize, text_value: "水表".to_string(), "水表" }
                    }
                }
                div { class: "field is-wide",
                    Label { html_for: "meter-location", "安装位置" }
                    Select {
                        id: "meter-location",
                        value: location_value,
                        on_value_change: move |value: Option<String>| {
                            location.set(value.unwrap_or_else(|| PUBLIC_AREA.to_string()));
                        },
                        for (index , (value , label)) in options.into_iter().enumerate() {
                            SelectOption::<String> {
                                key: "{value}",
                                value: value.clone(),
                                index,
                                text_value: label.clone(),
                                "{label}"
                            }
                        }
                    }
                }
                div { class: "field",
                    Label { html_for: "meter-f4", "倍率" }
                    Input { id: "meter-f4", inputmode: "decimal", value: multiplier(), oninput: move |event: FormEvent| multiplier.set(event.value()) }
                }
                div { class: "field",
                    // 设备号不在这里手敲：绑定要在智能水电表管理页对着合众的设备列表
                    // 选，敲错一位不会报错，只会让读数永远拉不到。这里只显示
                    // 当前绑定状态，编辑时原样带回服务端，不会被清空。
                    span { class: "field-label", "智能水电表设备" }
                    if device_id().trim().is_empty() {
                        p { class: "hint",
                            "未绑定，读数需要手工抄录。"
                            Link {
                                class: "hint-link",
                                to: Route::SmartElectricMeterPage {},
                                new_tab: true,
                                " 去智能水电表管理绑定 ↗"
                            }
                        }
                    } else {
                        p { class: "is-mono", "{device_id()}" }
                        small { class: "hint", "在智能水电表管理页可以解绑或改绑" }
                    }
                }
                if is_electric() {
                    div { class: "field is-wide",
                        label { class: "meter-picker-toggle",
                            input {
                                r#type: "checkbox",
                                checked: time_of_use(),
                                onchange: move |event: FormEvent| time_of_use.set(event.checked()),
                            }
                            span { "这块表分时计量（尖／峰／平／谷）" }
                        }
                    }
                }
                div { class: "field is-wide",
                    Label { html_for: "meter-f7", "备注" }
                    Textarea { id: "meter-f7", value: remark(), maxlength: 300, placeholder: "补充用途，如车间用电、电梯用电", oninput: move |event: FormEvent| remark.set(event.value()) }
                }
            }
            p { class: "hint", "结算单价不在这里填——同一块表换一户就换一个价，报价写在合同上。" }
            if let Some(message) = error() { p { class: "form-error", "{message}" } }
            footer { class: "form-actions",
                Button { variant: ButtonVariant::Outline, r#type: "button", disabled: loading(), onclick: move |_| on_done.call(()), "取消" }
                Button { r#type: "submit", disabled: loading(), if loading() { "保存中…" } else { "确认保存" } }
            }
        }
    }
}

/// 把安装位置编码成下拉框取值。
/// 某个园区可选的水电表安装位置：`(下拉取值, 显示名)`，首项恒为公共区域。
///
/// 园区详情的新增/编辑表单和智能水电表管理的设备导入对话框共用这一份候选——两处
/// 各建一遍的话，一边支持宿舍楼层、另一边只列厂房这种偏差迟早会出现。
pub(crate) fn meter_location_options(
    park_id: u64,
    factories: &[Factory],
    factory_floors: &[FactoryFloor],
    dormitories: &[Dormitory],
    dormitory_floors: &[DormitoryFloor],
) -> Vec<(String, String)> {
    let mut options = vec![(PUBLIC_AREA.to_string(), "园区公共区域".to_string())];
    for factory in factories
        .iter()
        .filter(|factory| !factory.is_deleted && factory.park_id == park_id)
    {
        for floor in factory_floors
            .iter()
            .filter(|floor| !floor.is_deleted && floor.factory_id == factory.factory_id)
        {
            options.push((
                encode_location(floor.floor_id, 0),
                format!("{} · {}", factory.factory_name, floor.floor_name),
            ));
        }
    }
    for dormitory in dormitories
        .iter()
        .filter(|dormitory| !dormitory.is_deleted && dormitory.park_id == park_id)
    {
        for floor in dormitory_floors
            .iter()
            .filter(|floor| !floor.is_deleted && floor.dormitory_id == dormitory.dormitory_id)
        {
            options.push((
                encode_location(0, floor.dormitory_floor_id),
                format!("{} · {} 层", dormitory.dormitory_name, floor.floor_no),
            ));
        }
    }
    options
}

pub(crate) fn encode_location(factory_floor_id: u64, dormitory_floor_id: u64) -> String {
    if factory_floor_id != 0 {
        format!("f{factory_floor_id}")
    } else if dormitory_floor_id != 0 {
        format!("d{dormitory_floor_id}")
    } else {
        PUBLIC_AREA.to_string()
    }
}

/// 解码回 `(厂房楼层, 宿舍楼层)`，未知取值一律退回公共区域。
pub(crate) fn decode_location(value: &str) -> (u64, u64) {
    match value.split_at_checked(1) {
        Some(("f", id)) => (id.parse().unwrap_or(0), 0),
        Some(("d", id)) => (0, id.parse().unwrap_or(0)),
        _ => (0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 安装位置编码解码可以往返() {
        // 厂房楼层和宿舍楼层的自增主键互相独立，同一个 7 号必须能区分开。
        assert_eq!(decode_location(&encode_location(7, 0)), (7, 0));
        assert_eq!(decode_location(&encode_location(0, 7)), (0, 7));
        assert_eq!(decode_location(&encode_location(0, 0)), (0, 0));
    }

    #[test]
    fn 未知位置取值退回公共区域() {
        assert_eq!(decode_location(PUBLIC_AREA), (0, 0));
        assert_eq!(decode_location(""), (0, 0));
        assert_eq!(decode_location("x9"), (0, 0));
    }
}
