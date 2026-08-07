//! 合同新增、查看和编辑表单。

use dioxus::prelude::*;

use super::attachments::{
    build_ai_images, discard_attachment_previews, ContractAttachment, ContractAttachments,
};
use super::model::{
    contract_status, decimal_input, format_date, meter_location_label, money_input, parse_decimal,
    format_unit_price, parse_increase_rules, parse_optional_date, parse_unit_price,
    serialize_increase_rules, today_timestamp,
    ContractStatus, FeeDraft, IncreaseRule, MeterPriceDraft, FEE_KINDS, RATE_BASES,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        DateField,
    },
    services::{
        analyze_contract_images, create_contract_with_images_record,
        delete_business_images_from_r2, has_park, update_contract_with_images_record,
        upload_business_image, StoredR2Image, NO_PARK,
    },
    spacetime_bindings::{
        park_type::Park,
        rental_tenant_dormitory_floor_input_type::RentalTenantDormitoryFloorInput,
        rental_tenant_floor_input_type::RentalTenantFloorInput,
        rental_tenant_fee_input_type::RentalTenantFeeInput,
        rental_tenant_meter_input_type::RentalTenantMeterInput,
        rental_tenant_input_type::RentalTenantInput, rental_tenant_type::RentalTenant,
        tenant_image_preview_type::TenantImagePreview,
        uploaded_tenant_image_input_type::UploadedTenantImageInput,
    },
    router::Route,
    state::WorkspaceState,
};

/// 合同表单。三个路由（新增／编辑／查看）都落到这里。
///
/// # 为什么是整页而不是弹窗
///
/// 合同要谈的东西一路长出来了：主体、租期、租金、楼层、宿舍、水电表逐块报价、
/// 基本电费、约定费用、递增档位、备注、合同原件。塞在半屏弹窗里每一项都只有
/// 一列宽，四段电价、比例＋基数这种并排字段被挤成竖排，滚动条一拉到底还看不完。
/// 整页之后横向能铺开，字段按语义分栏，长表单该有的样子。
#[component]
pub(super) fn ContractForm(
    contract: Option<RentalTenant>,
    parks: Vec<Park>,
    image_previews: Vec<TenantImagePreview>,
    readonly: bool,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let contract_id = contract.as_ref().map(|row| row.rental_tenant_id);
    let mut tenant_name = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| row.tenant_name.clone())
            .unwrap_or_default()
    });
    let mut phone_number = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| row.phone_number.clone())
            .unwrap_or_default()
    });
    let mut transaction_type = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| row.transaction_type)
            .unwrap_or(true)
    });
    let mut contract_start = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| format_date(row.contract_start))
            .filter(|value| value != "--")
            .unwrap_or_default()
    });
    let mut contract_end = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| format_date(row.contract_end))
            .filter(|value| value != "--")
            .unwrap_or_default()
    });
    let mut rent = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| money_input(row.rental_amount_cents))
            .unwrap_or_default()
    });
    let mut area = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| decimal_input(row.area_centi_square_metres, 100))
            .unwrap_or_default()
    });
    let mut park_id = use_signal_sync(|| {
        contract
            .as_ref()
            .filter(|row| has_park(row.park_id))
            .map(|row| row.park_id.to_string())
            .unwrap_or_default()
    });
    let mut basic_capacity = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| decimal_input(row.basic_ele_capacity_centi_kw, 100))
            .unwrap_or_default()
    });
    let mut basic_price = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| format_unit_price(row.basic_ele_price_scaled))
            .unwrap_or_default()
    });
    // 楼层归属：floor_id -> 该层占用面积的输入文本。用有序表让选中项在
    // 提交时顺序稳定，方便和服务端返回的关联行对照。
    let workspace = use_context::<WorkspaceState>();
    let mut selected_floors = use_signal_sync(|| {
        let Some(id) = contract_id else {
            return std::collections::BTreeMap::<u64, String>::new();
        };
        (workspace.rental_tenant_floors)()
            .iter()
            .filter(|link| link.rental_tenant_id == id)
            .map(|link| {
                (
                    link.floor_id,
                    decimal_input(Some(link.area_centi_square_metres), 100),
                )
            })
            .collect()
    });
    // 宿舍归属：dormitory_floor_id -> 该层占用的房间数文本。
    let mut selected_dorm_floors = use_signal_sync(|| {
        let Some(id) = contract_id else {
            return std::collections::BTreeMap::<u64, String>::new();
        };
        (workspace.rental_tenant_dormitory_floors)()
            .iter()
            .filter(|link| link.rental_tenant_id == id)
            .map(|link| (link.dormitory_floor_id, link.room_count.to_string()))
            .collect()
    });
    // 费用约定：fee_kind -> 这一项的草稿。三种费用固定存在，未启用的不提交。
    let mut fee_drafts = use_signal_sync(|| {
        let saved = contract_id
            .map(|id| {
                (workspace.rental_tenant_fees)()
                    .iter()
                    .filter(|row| row.rental_tenant_id == id)
                    .map(|row| (row.fee_kind.clone(), FeeDraft::from_row(row)))
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        FEE_KINDS
            .iter()
            .map(|(kind, _)| {
                let draft = saved.get(*kind).cloned().unwrap_or_default();
                ((*kind).to_string(), draft)
            })
            .collect::<std::collections::BTreeMap<String, FeeDraft>>()
    });
    // 用表关系：meter_id -> 这份合同对该表的报价草稿。
    let mut selected_meters = use_signal_sync(|| {
        let Some(id) = contract_id else {
            return std::collections::BTreeMap::<u64, MeterPriceDraft>::new();
        };
        (workspace.rental_tenant_meters)()
            .iter()
            .filter(|link| link.rental_tenant_id == id)
            .map(|link| (link.meter_id, MeterPriceDraft::from_link(link)))
            .collect()
    });
    let mut remark = use_signal_sync(|| {
        contract
            .as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    let mut increase_rules = use_signal_sync(|| {
        contract
            .as_ref()
            .map(|row| parse_increase_rules(row.increase_data.as_deref()))
            .unwrap_or_default()
    });
    let mut loading = use_signal_sync(|| false);
    let mut recognizing = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let mut recognition_notice = use_signal_sync(|| None::<String>);
    let attachments = use_signal_sync(|| {
        image_previews
            .iter()
            .map(|image| ContractAttachment::Existing {
                img_id: image.img_id,
                url: image.img_url.clone(),
                removed: false,
            })
            .collect::<Vec<_>>()
    });
    let mut completed = use_signal_sync(|| false);

    use_effect(move || {
        if completed() {
            on_saved.call(());
        }
    });

    let title = if readonly {
        "查看合同"
    } else if contract_id.is_some() {
        "编辑合同"
    } else {
        "新增合同"
    };
    let existing_for_submit = contract.clone();
    let transaction_value: ReadSignal<Option<String>> = use_memo(move || {
        Some(
            if transaction_type() {
                "income"
            } else {
                "expense"
            }
                .to_string(),
        )
    })
        .into();
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();
    let rule_count_value: ReadSignal<Option<String>> =
        use_memo(move || Some(increase_rules().len().to_string())).into();

    rsx! {
        main { class: "page contract-form-page",
            div { class: "page-header",
                div { class: "stack-tight",
                    h2 { "{title}" }
                    p { class: "hint", "维护合同主体、租期、租金、水电报价与约定费用。" }
                }
                Link { class: "hint-link", to: Route::ContractManagementPage {}, "← 返回合同管理" }
            }
            {
                let form = rsx! {
                form {
                    onsubmit: move |event| {
                        event.prevent_default();
                        if readonly { on_close.call(()); return; }
                        if loading() { return; }
                        let name = tenant_name().trim().to_string();
                        let phone = phone_number().trim().to_string();
                        if name.is_empty() { error.set(Some("请输入合同方名称".into())); return; }
                        if phone.is_empty() { error.set(Some("请输入联系电话".into())); return; }
                        if remark().chars().count() > 200 { error.set(Some("备注不能超过 200 个字符".into())); return; }
                        let start = match parse_optional_date(&contract_start(), "合同开始日期") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let end = match parse_optional_date(&contract_end(), "合同结束日期") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        if start.is_none() || end.is_none() { error.set(Some("请选择完整的合同起止日期".into())); return; }
                        if end < start { error.set(Some("合同结束日期不能早于开始日期".into())); return; }
                        let rental_amount_cents = match parse_decimal(&rent(), 100, "月租金") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let area_centi_square_metres = match parse_decimal(&area(), 100, "租赁面积") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let park = match park_id().parse::<u64>() { Ok(value) if value > 0 => value, _ => { error.set(Some("请选择所属园区".into())); return; } };
                        let mut floor_inputs = Vec::new();
                        for (floor_id, text) in selected_floors() {
                            match parse_decimal(&text, 100, "楼层占用面积") {
                                Ok(Some(value)) => floor_inputs.push(RentalTenantFloorInput { floor_id, area_centi_square_metres: value }),
                                Ok(None) => { error.set(Some("请填写已勾选楼层的占用面积".into())); return; }
                                Err(message) => { error.set(Some(message)); return; }
                            }
                        }
                        let mut dorm_inputs: Vec<RentalTenantDormitoryFloorInput> = Vec::new();
                        for (dormitory_floor_id, text) in selected_dorm_floors() {
                            match text.trim().parse::<i32>() {
                                Ok(value) if value > 0 => dorm_inputs.push(RentalTenantDormitoryFloorInput { dormitory_floor_id, room_count: value }),
                                _ => { error.set(Some("请为已勾选的宿舍楼层填写大于零的占用间数".into())); return; }
                            }
                        }
                        let meter_catalog = (workspace.utility_meters)();
                        let mut meter_inputs: Vec<RentalTenantMeterInput> = Vec::new();
                        for (meter_id, draft) in selected_meters() {
                            let code = meter_catalog
                                .iter()
                                .find(|meter| meter.meter_id == meter_id)
                                .map(|meter| meter.meter_code.clone())
                                .unwrap_or_else(|| meter_id.to_string());
                            match draft.to_input(meter_id, &code) {
                                Ok(value) => meter_inputs.push(value),
                                Err(message) => { error.set(Some(message)); return; }
                            }
                        }
                        let mut fee_inputs: Vec<RentalTenantFeeInput> = Vec::new();
                        for (fee_kind, draft) in fee_drafts() {
                            match draft.to_input(&fee_kind) {
                                Ok(Some(value)) => fee_inputs.push(value),
                                Ok(None) => {}
                                Err(message) => { error.set(Some(message)); return; }
                            }
                        }
                        let basic_ele_capacity_centi_kw = match parse_decimal(&basic_capacity(), 100, "基本电费计费容量") {
                            Ok(value) => value, Err(message) => { error.set(Some(message)); return; }
                        };
                        let basic_ele_price_scaled = match parse_unit_price(&basic_price(), "基本电费单价") {
                            Ok(value) => value, Err(message) => { error.set(Some(message)); return; }
                        };
                        // 容量和单价缺一不可：只填一个算不出金额，存进去就是一份签了却收不上来的条款。
                        if basic_ele_capacity_centi_kw.is_some() != basic_ele_price_scaled.is_some() {
                            error.set(Some("基本电费的计费容量和单价要么都填、要么都不填".into()));
                            return;
                        }
                        let rules = increase_rules();
                        if rules.iter().any(|rule| rule.date == 0 || rule.rate < 0.0) { error.set(Some("递增年数必须大于 0，递增比例不能为负数".into())); return; }
                        let increase_data = match serialize_increase_rules(&rules) { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let increase_rate_basis_points = rules.first().map(|rule| (rule.rate * 100.0).round() as i64);
                        let status = end.map(|end| {
                            let probe = RentalTenant {
                                rental_tenant_id: 0, customer_id: String::new(), tenant_name: String::new(), phone_number: String::new(), transaction_type: true,
                                status: None, contract_start: start, contract_end: Some(end), rental_amount_cents: None, increase_date: None,
                                increase_rate_basis_points: None, increase_data: None, penalty_rate_basis_points: None,
                                basic_ele_capacity_centi_kw: None, basic_ele_price_scaled: None, area_centi_square_metres: None,
                                remark: None, park_id: 0, send_message_at: None, is_deleted: false,
                                created_at: today_timestamp(), updated_at: None,
                            };
                            match contract_status(&probe, today_timestamp()) { ContractStatus::Expired => "expired", _ => "active" }.to_string()
                        });
                        let input = RentalTenantInput {
                            tenant_name: name,
                            phone_number: phone,
                            transaction_type: transaction_type(),
                            status,
                            contract_start: start,
                            contract_end: end,
                            rental_amount_cents,
                            increase_date: existing_for_submit.as_ref().and_then(|row| row.increase_date),
                            increase_rate_basis_points,
                            increase_data,
                            penalty_rate_basis_points: existing_for_submit.as_ref().and_then(|row| row.penalty_rate_basis_points),
                            area_centi_square_metres,
                            remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                            park_id: Some(park),
                            send_message_at: existing_for_submit.as_ref().and_then(|row| row.send_message_at),
                            floors: floor_inputs,
                            dormitory_floors: dorm_inputs,
                            meters: meter_inputs,
                            fees: fee_inputs,
                            basic_ele_capacity_centi_kw,
                            basic_ele_price_scaled,
                        };
                        let attachment_values = attachments();
                        let existing_image_ids = attachment_values.iter().filter_map(|attachment| match attachment {
                            ContractAttachment::Existing { img_id, removed: false, .. } => Some(*img_id),
                            _ => None,
                        }).collect::<Vec<_>>();
                        let removed_images = attachment_values.iter().filter_map(|attachment| match attachment {
                            ContractAttachment::Existing { img_id, url, removed: true } => Some(StoredR2Image { img_id: *img_id, public_url: url.clone() }),
                            _ => None,
                        }).collect::<Vec<_>>();
                        let pending_files = attachment_values.into_iter().filter_map(|attachment| match attachment {
                            ContractAttachment::Pending { file, .. } => Some(file),
                            _ => None,
                        }).collect::<Vec<_>>();
                        error.set(None); loading.set(true);
                        spawn(async move {
                            let mut uploads = Vec::with_capacity(pending_files.len());
                            for file in pending_files {
                                match upload_business_image(file).await {
                                    Ok(image) => uploads.push(UploadedTenantImageInput { img_url: image.public_url, hash: image.sha256 }),
                                    Err(message) => { loading.set(false); error.set(Some(message)); return; }
                                }
                            }
                            let result = if let Some(id) = contract_id {
                                update_contract_with_images_record(id, input, existing_image_ids, uploads).await
                            } else {
                                create_contract_with_images_record(input, uploads).await
                            };
                            match result {
                                Ok(()) => match delete_business_images_from_r2(removed_images).await {
                                    Ok(_) => {
                                        loading.set(false);
                                        discard_attachment_previews(&attachments());
                                        completed.set(true);
                                    }
                                    Err(message) => {
                                        loading.set(false);
                                        error.set(Some(format!("合同已经保存，但{message}；请再次保存重试清理")));
                                    }
                                },
                                Err(message) => { loading.set(false); error.set(Some(message)); }
                            }
                        });
                    },
                    section { class: "subsection",
                        h4 { "合同主体" }
                        div { class: "form-grid",
                            div { class: "field",
                                Label { html_for: "contract-tenant-name", "合同方名称" }
                                Input {
                                    id: "contract-tenant-name",
                                    value: tenant_name(),
                                    readonly,
                                    placeholder: "请输入企业或个人名称",
                                    oninput: move |event: FormEvent| tenant_name.set(event.value()),
                                }
                            }
                            div { class: "field",
                                Label { html_for: "contract-phone", "联系电话" }
                                Input {
                                    id: "contract-phone",
                                    inputmode: "tel",
                                    value: phone_number(),
                                    readonly,
                                    placeholder: "请输入联系电话",
                                    oninput: move |event: FormEvent| phone_number.set(event.value()),
                                }
                            }
                            div { class: "field",
                                Label { html_for: "contract-transaction", "交易类型" }
                                Select {
                                    id: "contract-transaction",
                                    value: Some(transaction_value),
                                    disabled: readonly,
                                    on_value_change: move |value: Option<String>| {
                                        transaction_type.set(value.as_deref() == Some("income"));
                                    },
                                    SelectOption::<String> { value: "income".to_string(), index: 0usize, text_value: "收入合同".to_string(), "收入合同" }
                                    SelectOption::<String> { value: "expense".to_string(), index: 1usize, text_value: "支出合同".to_string(), "支出合同" }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "contract-park-select", "所属园区" }
                                Select {
                                    id: "contract-park-select",
                                    value: Some(park_value),
                                    disabled: readonly,
                                    on_value_change: move |value: Option<String>| park_id.set(value.unwrap_or_default()),
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                                    for (index , park) in parks.iter().enumerate() {
                                        SelectOption::<String> {
                                            key: "form-park-{park.park_id}",
                                            value: park.park_id.to_string(),
                                            index: index + 1,
                                            text_value: park.park_name.to_string(), "{park.park_name}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    section { class: "subsection",
                        h4 { "租期与经营信息" }
                        div { class: "form-grid",
                            div { class: "field",
                                span { class: "field-label", "开始日期" }
                                DateField {
                                    value: contract_start(),
                                    disabled: readonly,
                                    max: (!contract_end().is_empty()).then(|| contract_end()),
                                    on_change: move |value: String| contract_start.set(value),
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "结束日期" }
                                DateField {
                                    value: contract_end(),
                                    disabled: readonly,
                                    min: (!contract_start().is_empty()).then(|| contract_start()),
                                    on_change: move |value: String| contract_end.set(value),
                                }
                            }
                            div { class: "field",
                                Label { html_for: "contract-rent", "月租金（元/月）" }
                                Input {
                                    id: "contract-rent",
                                    inputmode: "decimal",
                                    value: rent(),
                                    readonly,
                                    placeholder: "0.00",
                                    oninput: move |event: FormEvent| rent.set(event.value()),
                                }
                            }
                            div { class: "field",
                                Label { html_for: "contract-area", "租赁面积（㎡）" }
                                Input {
                                    id: "contract-area",
                                    inputmode: "decimal",
                                    value: area(),
                                    readonly,
                                    placeholder: "0.00",
                                    oninput: move |event: FormEvent| area.set(event.value()),
                                }
                            }
                        }
                    }
                    section { class: "subsection",
                        div { class: "section-header",
                            h4 { "租用楼层" }
                            span { class: "hint", "可跨多层" }
                        }
                        {
                            let selected_park = park_id().parse::<u64>().unwrap_or(NO_PARK);
                            let factories = (workspace.factories)();
                            let floors = (workspace.factory_floors)();
                            // 只列出所选园区名下、未删除厂房的未删除楼层——
                            // 跨园区选楼层在业务上没有意义，也会让列表长到不可用。
                            let mut options = factories
                                .iter()
                                .filter(|factory| !factory.is_deleted && factory.park_id == selected_park)
                                .flat_map(|factory| {
                                    floors
                                        .iter()
                                        .filter(move |floor| !floor.is_deleted && floor.factory_id == factory.factory_id)
                                        .map(move |floor| (factory.factory_name.clone(), floor.clone()))
                                })
                                .collect::<Vec<_>>();
                            options.sort_by(|left, right| {
                                left.0.cmp(&right.0).then(left.1.floor_name.cmp(&right.1.floor_name))
                            });
                            let picked = selected_floors();
                            let picked_total: i64 = picked
                                .values()
                                .filter_map(|text| parse_decimal(text, 100, "面积").ok().flatten())
                                .sum();
                            rsx! {
                                if selected_park == 0 {
                                    p { class: "hint", "请先选择所属园区，再勾选租用的楼层。" }
                                } else if options.is_empty() {
                                    LedgerHint {
                                        park_id: selected_park,
                                        message: "该园区还没有维护厂房楼层台账，无法选择楼层。".to_string(),
                                    }
                                } else {
                                    ul { class: "floor-picker",
                                        for (factory_name, floor) in options {
                                            {
                                                let floor_id = floor.floor_id;
                                                let checked = picked.contains_key(&floor_id);
                                                let total_text = decimal_input(Some(floor.total_area_centi_square_metres), 100);
                                                let default_area = total_text.clone();
                                                rsx! {
                                                    li { key: "floor-{floor_id}", class: if checked { "floor-picker-row is-picked" } else { "floor-picker-row" },
                                                        label { class: "floor-picker-main",
                                                            input {
                                                                r#type: "checkbox",
                                                                checked,
                                                                disabled: readonly,
                                                                onchange: move |event: FormEvent| {
                                                                    let mut next = selected_floors();
                                                                    if event.checked() {
                                                                        // 默认按整层出租，分租时用户改小即可。
                                                                        next.insert(floor_id, default_area.clone());
                                                                    } else {
                                                                        next.remove(&floor_id);
                                                                    }
                                                                    selected_floors.set(next);
                                                                },
                                                            }
                                                            span { "{factory_name} · {floor.floor_name}" }
                                                            small { class: "hint", "整层 {total_text} ㎡" }
                                                        }
                                                        if checked {
                                                            div { class: "floor-picker-area",
                                                                Input {
                                                                    value: picked.get(&floor_id).cloned().unwrap_or_default(),
                                                                    inputmode: "decimal",
                                                                    readonly,
                                                                    placeholder: "占用面积",
                                                                    oninput: move |event: FormEvent| {
                                                                        let mut next = selected_floors();
                                                                        next.insert(floor_id, event.value());
                                                                        selected_floors.set(next);
                                                                    },
                                                                }
                                                                span { class: "hint", "㎡" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    p { class: "hint",
                                        "已选 {picked.len()} 层，合计 {decimal_input(Some(picked_total), 100)} ㎡"
                                        if !area().trim().is_empty() {
                                            "（合同填报面积 {area()} ㎡）"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    section { class: "subsection",
                        div { class: "section-header",
                            h4 { "租用宿舍" }
                            span { class: "hint", "按间出租" }
                        }
                        {
                            let selected_park = park_id().parse::<u64>().unwrap_or(NO_PARK);
                            let dormitories = (workspace.dormitories)();
                            let dorm_floors = (workspace.dormitory_floors)();
                            let all_links = (workspace.rental_tenant_dormitory_floors)();
                            // 只列所选园区名下、未删除宿舍的未删除楼层。
                            let mut options = dormitories
                                .iter()
                                .filter(|dormitory| !dormitory.is_deleted && dormitory.park_id == selected_park)
                                .flat_map(|dormitory| {
                                    dorm_floors
                                        .iter()
                                        .filter(move |floor| !floor.is_deleted && floor.dormitory_id == dormitory.dormitory_id)
                                        .map(move |floor| (dormitory.dormitory_name.clone(), floor.clone()))
                                })
                                .collect::<Vec<_>>();
                            options.sort_by(|left, right| {
                                left.0.cmp(&right.0).then(left.1.floor_no.cmp(&right.1.floor_no))
                            });
                            let picked = selected_dorm_floors();
                            let picked_rooms: i32 = picked
                                .values()
                                .filter_map(|text| text.trim().parse::<i32>().ok())
                                .sum();
                            rsx! {
                                if selected_park == 0 {
                                    p { class: "hint", "请先选择所属园区，再勾选租用的宿舍楼层。" }
                                } else if options.is_empty() {
                                    LedgerHint {
                                        park_id: selected_park,
                                        message: "该园区还没有维护宿舍楼层台账，只租厂房的合同可以跳过这一项。".to_string(),
                                    }
                                } else {
                                    ul { class: "floor-picker",
                                        for (dormitory_name , floor) in options {
                                            {
                                                let floor_id = floor.dormitory_floor_id;
                                                let checked = picked.contains_key(&floor_id);
                                                // 别的合同已占的间数不能再选，这里先算出上限，
                                                // 避免用户填完才被服务端拒绝。
                                                let taken: i32 = all_links
                                                    .iter()
                                                    .filter(|link| link.dormitory_floor_id == floor_id)
                                                    .filter(|link| Some(link.rental_tenant_id) != contract_id)
                                                    .map(|link| link.room_count)
                                                    .sum();
                                                let free = (floor.room_count - taken).max(0);
                                                let default_rooms = free.to_string();
                                                rsx! {
                                                    li { key: "dorm-floor-{floor_id}", class: if checked { "floor-picker-row is-picked" } else { "floor-picker-row" },
                                                        label { class: "floor-picker-main",
                                                            input {
                                                                r#type: "checkbox",
                                                                checked,
                                                                disabled: readonly || (free == 0 && !checked),
                                                                onchange: move |event: FormEvent| {
                                                                    let mut next = selected_dorm_floors();
                                                                    if event.checked() {
                                                                        next.insert(floor_id, default_rooms.clone());
                                                                    } else {
                                                                        next.remove(&floor_id);
                                                                    }
                                                                    selected_dorm_floors.set(next);
                                                                },
                                                            }
                                                            span { "{dormitory_name} · {floor.floor_no} 层" }
                                                            small { class: "hint", "共 {floor.room_count} 间 · 可租 {free} 间" }
                                                        }
                                                        if checked {
                                                            div { class: "floor-picker-area",
                                                                Input {
                                                                    value: picked.get(&floor_id).cloned().unwrap_or_default(),
                                                                    inputmode: "numeric",
                                                                    readonly,
                                                                    placeholder: "占用间数",
                                                                    oninput: move |event: FormEvent| {
                                                                        let mut next = selected_dorm_floors();
                                                                        next.insert(floor_id, event.value());
                                                                        selected_dorm_floors.set(next);
                                                                    },
                                                                }
                                                                span { class: "hint", "间" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    p { class: "hint", "已选 {picked.len()} 层，合计 {picked_rooms} 间" }
                                }
                            }
                        }
                    }
                    section { class: "subsection",
                        div { class: "section-header",
                            h4 { "水电表与结算单价" }
                            span { class: "hint", "水电定价是合同条款" }
                        }
                        {
                            let selected_park = park_id().parse::<u64>().unwrap_or(NO_PARK);
                            let factories = (workspace.factories)();
                            let factory_floors = (workspace.factory_floors)();
                            let dormitories = (workspace.dormitories)();
                            let dormitory_floors = (workspace.dormitory_floors)();
                            let mut meters = (workspace.utility_meters)()
                                .into_iter()
                                .filter(|meter| !meter.is_deleted && meter.park_id == selected_park)
                                .collect::<Vec<_>>();
                            // 电表在前、水表在后，同类按表号排——抄表和对账都按表号认表。
                            meters.sort_by(|left, right| {
                                right.is_electric.cmp(&left.is_electric)
                                    .then(left.meter_code.cmp(&right.meter_code))
                            });
                            let picked = selected_meters();
                            rsx! {
                                if selected_park == 0 {
                                    p { class: "hint", "请先选择所属园区，再勾选这份合同用到的水电表。" }
                                } else if meters.is_empty() {
                                    LedgerHint {
                                        park_id: selected_park,
                                        message: "该园区还没有维护水电表台账，约定单价前要先把表建进台账。".to_string(),
                                    }
                                } else {
                                    ul { class: "floor-picker meter-picker",
                                        for meter in meters {
                                            {
                                                let meter_id = meter.meter_id;
                                                let code = meter.meter_code.clone();
                                                let draft = picked.get(&meter_id).cloned();
                                                let checked = draft.is_some();
                                                let draft = draft.unwrap_or_default();
                                                let kind = if meter.is_electric { "电表" } else { "水表" };
                                                let unit = if meter.is_electric { "元/度" } else { "元/吨" };
                                                let location = meter_location_label(
                                                    &meter, &factories, &factory_floors, &dormitories, &dormitory_floors,
                                                );
                                                let tiered = draft.tiered;
                                                let can_tier = meter.is_time_of_use;
                                                // 倍率 1 是绝大多数表的情况，标出来只会变成噪音。
                                                let multiplier_note = (meter.multiplier_centi != 100).then(|| {
                                                    format!("倍率 ×{}", decimal_input(Some(meter.multiplier_centi), 100))
                                                });
                                                rsx! {
                                                    li { key: "meter-{meter_id}", class: if checked { "floor-picker-row is-picked" } else { "floor-picker-row" },
                                                        label { class: "floor-picker-main",
                                                            input {
                                                                r#type: "checkbox",
                                                                checked,
                                                                disabled: readonly,
                                                                onchange: move |event: FormEvent| {
                                                                    let mut next = selected_meters();
                                                                    if event.checked() {
                                                                        // 分时表默认就按分时报价，省得用户每块表都点一次。
                                                                        next.insert(meter_id, MeterPriceDraft { tiered: can_tier, ..Default::default() });
                                                                    } else {
                                                                        next.remove(&meter_id);
                                                                    }
                                                                    selected_meters.set(next);
                                                                },
                                                            }
                                                            span { "{code}" }
                                                            small { class: "hint", "{kind} · {location}" }
                                                            if multiplier_note.is_some() {
                                                                // 互感器表的读数要乘倍率才是实际用量。倍率是表的物理属性，
                                                                // 谈判改不了，所以这里只显示不可改——但必须显示：×1000 的表
                                                                // 按 ×1 结算，一个月能差出三个数量级。
                                                                Badge { variant: BadgeVariant::Outline, "{multiplier_note.clone().unwrap_or_default()}" }
                                                            }
                                                        }
                                                        if checked {
                                                            div { class: if tiered { "meter-picker-price is-tiered" } else { "meter-picker-price" },
                                                                if can_tier {
                                                                    label { class: "meter-picker-toggle",
                                                                        input {
                                                                            r#type: "checkbox",
                                                                            checked: tiered,
                                                                            disabled: readonly,
                                                                            onchange: move |event: FormEvent| {
                                                                                let mut next = selected_meters();
                                                                                if let Some(entry) = next.get_mut(&meter_id) {
                                                                                    entry.tiered = event.checked();
                                                                                }
                                                                                selected_meters.set(next);
                                                                            },
                                                                        }
                                                                        span { "分时电价" }
                                                                    }
                                                                }
                                                                if tiered {
                                                                    div { class: "meter-picker-tiers",
                                                                        for (name , value) in [
                                                                            ("尖", draft.tip.clone()),
                                                                            ("峰", draft.peak.clone()),
                                                                            ("平", draft.flat.clone()),
                                                                            ("谷", draft.valley.clone()),
                                                                        ] {
                                                                            label { key: "tier-{meter_id}-{name}", class: "meter-picker-tier",
                                                                                span { "{name}" }
                                                                                Input {
                                                                                    value,
                                                                                    inputmode: "decimal",
                                                                                    readonly,
                                                                                    placeholder: "0.00",
                                                                                    oninput: move |event: FormEvent| {
                                                                                        let mut next = selected_meters();
                                                                                        if let Some(entry) = next.get_mut(&meter_id) {
                                                                                            match name {
                                                                                                "尖" => entry.tip = event.value(),
                                                                                                "峰" => entry.peak = event.value(),
                                                                                                "平" => entry.flat = event.value(),
                                                                                                _ => entry.valley = event.value(),
                                                                                            }
                                                                                        }
                                                                                        selected_meters.set(next);
                                                                                    },
                                                                                }
                                                                            }
                                                                        }
                                                                        span { class: "hint", "{unit}" }
                                                                    }
                                                                } else {
                                                                    div { class: "floor-picker-area",
                                                                        Input {
                                                                            value: draft.unit_price.clone(),
                                                                            inputmode: "decimal",
                                                                            readonly,
                                                                            placeholder: "结算单价",
                                                                            oninput: move |event: FormEvent| {
                                                                                let mut next = selected_meters();
                                                                                if let Some(entry) = next.get_mut(&meter_id) {
                                                                                    entry.unit_price = event.value();
                                                                                }
                                                                                selected_meters.set(next);
                                                                            },
                                                                        }
                                                                        span { class: "hint", "{unit}" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    p { class: "hint",
                                        "已选 {picked.len()} 块表。单价在这里谈定一次，之后每月开账单直接引用，不必重复录入。"
                                    }
                                }
                            }
                        }
                    }
                    section { class: "subsection",
                        div { class: "section-header",
                            h4 { "基本电费" }
                            span { class: "hint", "按变压器容量收，与用量无关" }
                        }
                        div { class: "form-grid",
                            div { class: "field",
                                Label { html_for: "contract-basic-capacity", "计费容量（kW）" }
                                Input {
                                    id: "contract-basic-capacity",
                                    value: basic_capacity(),
                                    inputmode: "decimal",
                                    readonly,
                                    placeholder: "不收基本电费可留空",
                                    oninput: move |event: FormEvent| basic_capacity.set(event.value()),
                                }
                            }
                            div { class: "field",
                                Label { html_for: "contract-basic-price", "基本电费单价（元/kW）" }
                                Input {
                                    id: "contract-basic-price",
                                    value: basic_price(),
                                    inputmode: "decimal",
                                    readonly,
                                    placeholder: "如 23.00",
                                    oninput: move |event: FormEvent| basic_price.set(event.value()),
                                }
                            }
                        }
                        p { class: "hint", "两项都填才会计入账单；开账单时会连同当期算式一起存进账单，之后改合同不影响已开出去的账单。" }
                    }
                    section { class: "subsection",
                        div { class: "section-header",
                            h4 { "约定费用" }
                            span { class: "hint", "签一次，每月账单自动算" }
                        }
                        for (fee_kind , fee_label) in FEE_KINDS {
                            {
                                let draft = fee_drafts().get(fee_kind).cloned().unwrap_or_default();
                                let enabled = draft.enabled;
                                let by_rate = draft.by_rate;
                                // 这两个 memo 必须在闭包里读 `fee_drafts` 信号。捕获外面那个
                                // 已经取好值的 `by_rate`／`rate_base` 的话，闭包里没有任何信号
                                // 依赖，memo 建一次就再也不重算——下拉框会永远停在首次渲染的
                                // 选项上，而下面的分支用的是新值，于是出现「选了固定金额、却
                                // 显示比例和计费基数」。
                                let mode_value: ReadSignal<Option<String>> = use_memo(move || {
                                    let by_rate = fee_drafts()
                                        .get(fee_kind)
                                        .is_some_and(|draft| draft.by_rate);
                                    Some(if by_rate { "rate" } else { "fixed" }.to_string())
                                })
                                .into();
                                let base_value: ReadSignal<Option<String>> = use_memo(move || {
                                    Some(
                                        fee_drafts()
                                            .get(fee_kind)
                                            .map(|draft| draft.rate_base.clone())
                                            .unwrap_or_default(),
                                    )
                                })
                                .into();
                                rsx! {
                                    div { key: "fee-{fee_kind}", class: if enabled { "fee-row is-picked" } else { "fee-row" },
                                        label { class: "fee-row-main",
                                            input {
                                                r#type: "checkbox",
                                                checked: enabled,
                                                disabled: readonly,
                                                onchange: move |event: FormEvent| {
                                                    let mut next = fee_drafts();
                                                    if let Some(entry) = next.get_mut(fee_kind) {
                                                        entry.enabled = event.checked();
                                                    }
                                                    fee_drafts.set(next);
                                                },
                                            }
                                            span { "{fee_label}" }
                                        }
                                        if enabled {
                                            div { class: "fee-row-terms",
                                                div { class: "field",
                                                    Label { html_for: "fee-mode-{fee_kind}", "计费方式" }
                                                    Select {
                                                        id: "fee-mode-{fee_kind}",
                                                        value: Some(mode_value),
                                                        disabled: readonly,
                                                        on_value_change: move |value: Option<String>| {
                                                            let mut next = fee_drafts();
                                                            if let Some(entry) = next.get_mut(fee_kind) {
                                                                entry.by_rate = value.as_deref() == Some("rate");
                                                            }
                                                            fee_drafts.set(next);
                                                        },
                                                        SelectOption::<String> { value: "fixed".to_string(), index: 0usize, text_value: "固定金额".to_string(), "固定金额" }
                                                        SelectOption::<String> { value: "rate".to_string(), index: 1usize, text_value: "按比例".to_string(), "按比例" }
                                                    }
                                                }
                                                if by_rate {
                                                    div { class: "field",
                                                        Label { html_for: "fee-rate-{fee_kind}", "比例（%）" }
                                                        Input {
                                                            id: "fee-rate-{fee_kind}",
                                                            value: draft.rate.clone(),
                                                            inputmode: "decimal",
                                                            readonly,
                                                            placeholder: "如 5.00",
                                                            oninput: move |event: FormEvent| {
                                                                let mut next = fee_drafts();
                                                                if let Some(entry) = next.get_mut(fee_kind) {
                                                                    entry.rate = event.value();
                                                                }
                                                                fee_drafts.set(next);
                                                            },
                                                        }
                                                    }
                                                    div { class: "field",
                                                        Label { html_for: "fee-base-{fee_kind}", "计费基数" }
                                                        Select {
                                                            id: "fee-base-{fee_kind}",
                                                            value: Some(base_value),
                                                            disabled: readonly,
                                                            on_value_change: move |value: Option<String>| {
                                                                let mut next = fee_drafts();
                                                                if let Some(entry) = next.get_mut(fee_kind) {
                                                                    entry.rate_base = value.unwrap_or_default();
                                                                }
                                                                fee_drafts.set(next);
                                                            },
                                                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择计费基数".to_string(), "请选择计费基数" }
                                                            for (index , (base , base_label)) in RATE_BASES.iter().enumerate() {
                                                                SelectOption::<String> {
                                                                    key: "fee-base-{fee_kind}-{base}",
                                                                    value: (*base).to_string(),
                                                                    index: index + 1,
                                                                    text_value: (*base_label).to_string(),
                                                                    "{base_label}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    div { class: "field",
                                                        Label { html_for: "fee-amount-{fee_kind}", "金额（元/月）" }
                                                        Input {
                                                            id: "fee-amount-{fee_kind}",
                                                            value: draft.amount.clone(),
                                                            inputmode: "decimal",
                                                            readonly,
                                                            placeholder: "如 800.00",
                                                            oninput: move |event: FormEvent| {
                                                                let mut next = fee_drafts();
                                                                if let Some(entry) = next.get_mut(fee_kind) {
                                                                    entry.amount = event.value();
                                                                }
                                                                fee_drafts.set(next);
                                                            },
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        p { class: "hint",
                            "「厂房」「宿舍」按电表装在哪一层自动区分，不用逐条选。开账单时按当期实际用量算，不是把某个月的数字写死在合同里。"
                        }
                    }
                    section { class: "subsection",
                        div { class: "section-header",
                            h4 { "租金递增" }
                            span { class: "hint", "最多三档" }
                        }
                        div { class: "field",
                            Label { html_for: "contract-rule-count", "递增档数" }
                            Select {
                                id: "contract-rule-count",
                                value: Some(rule_count_value),
                                disabled: readonly,
                                on_value_change: move |value: Option<String>| {
                                    let count = value
                                        .and_then(|value| value.parse::<usize>().ok())
                                        .unwrap_or_default()
                                        .min(3);
                                    increase_rules
                                        .with_mut(|rules| {
                                            rules.resize(count, IncreaseRule { date: 1, rate: 0.0 })
                                        });
                                },
                                SelectOption::<String> { value: "0".to_string(), index: 0usize, text_value: "无递增".to_string(), "无递增" }
                                SelectOption::<String> { value: "1".to_string(), index: 1usize, text_value: "一档".to_string(), "一档" }
                                SelectOption::<String> { value: "2".to_string(), index: 2usize, text_value: "两档".to_string(), "两档" }
                                SelectOption::<String> { value: "3".to_string(), index: 3usize, text_value: "三档".to_string(), "三档" }
                            }
                        }
                        if !increase_rules().is_empty() {
                            div { class: "stack",
                                for (index , rule) in increase_rules().into_iter().enumerate() {
                                    div { key: "rule-{index}", class: "panel is-tight",
                                        strong { "第 {index + 1} 档" }
                                        div { class: "form-grid",
                                            div { class: "field",
                                                Label { html_for: "rule-year-{index}", "合同第几年" }
                                                Input {
                                                    id: "rule-year-{index}",
                                                    inputmode: "numeric",
                                                    value: rule.date.to_string(),
                                                    readonly,
                                                    oninput: move |event: FormEvent| {
                                                        increase_rules
                                                            .with_mut(|rules| {
                                                                if let Some(rule) = rules.get_mut(index) {
                                                                    rule.date = event.value().parse().unwrap_or_default();
                                                                }
                                                            })
                                                    },
                                                }
                                            }
                                            div { class: "field",
                                                Label { html_for: "rule-rate-{index}", "递增比例（%）" }
                                                Input {
                                                    id: "rule-rate-{index}",
                                                    inputmode: "decimal",
                                                    value: rule.rate.to_string(),
                                                    readonly,
                                                    oninput: move |event: FormEvent| {
                                                        increase_rules
                                                            .with_mut(|rules| {
                                                                if let Some(rule) = rules.get_mut(index) {
                                                                    rule.rate = event.value().parse().unwrap_or_default();
                                                                }
                                                            })
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    section { class: "subsection",
                        div { class: "field",
                            Label { html_for: "contract-remark", "合同备注" }
                            Textarea {
                                id: "contract-remark",
                                value: remark(),
                                readonly,
                                maxlength: 200,
                                rows: 3,
                                placeholder: "填写付款、交付或其他特殊约定",
                                oninput: move |event: FormEvent| remark.set(event.value()),
                            }
                        }
                    }
                    section { class: "subsection",
                        div { class: "section-header",
                            h4 { "合同原件与 AI 识别" }
                            span { class: "hint", "本地预览、确认后上传，识别结果只补空字段" }
                        }
                        ContractAttachments {
                            attachments,
                            readonly,
                            loading: loading(),
                            recognizing: recognizing(),
                            on_error: move |message| { recognition_notice.set(None); error.set(Some(message)); },
                            on_recognize: move |_| {
                                if recognizing() || loading() { return; }
                                recognizing.set(true);
                                error.set(None);
                                recognition_notice.set(Some("正在读取合同图片并调用 AI…".into()));
                                spawn(async move {
                                    let result = match build_ai_images(attachments()).await {
                                        Ok(images) => analyze_contract_images(images).await,
                                        Err(message) => Err(message),
                                    };
                                    recognizing.set(false);
                                    match result {
                                        Ok(draft) => {
                                            let mut count = 0usize;
                                            if tenant_name().trim().is_empty() && !draft.tenant_name.is_empty() { tenant_name.set(draft.tenant_name); count += 1; }
                                            if phone_number().trim().is_empty() && !draft.phone_number.is_empty() { phone_number.set(draft.phone_number); count += 1; }
                                            if contract_start().trim().is_empty() && !draft.contract_start.is_empty() { contract_start.set(draft.contract_start); count += 1; }
                                            if contract_end().trim().is_empty() && !draft.contract_end.is_empty() { contract_end.set(draft.contract_end); count += 1; }
                                            if rent().trim().is_empty() && !draft.rent.is_empty() { rent.set(draft.rent); count += 1; }
                                            if area().trim().is_empty() && !draft.area.is_empty() { area.set(draft.area); count += 1; }
                                            // 合同已经没有自由文本地址字段，识别出的地址不能直接落库；
                                            // 但它正是用来对照勾选楼层的线索，所以原样提示出来而不是丢掉。
                                            let hint = (!draft.address.trim().is_empty())
                                                .then(|| format!("图片里的租赁地址是「{}」，请据此在下方勾选租用楼层。", draft.address.trim()));
                                            let base = if count == 0 { "AI 已完成识别，但当前字段已有内容或图片中未识别到新字段。".to_string() } else { format!("AI 识别完成，已补全 {count} 个空字段，请核对后再保存。") };
                                            recognition_notice.set(Some(match hint { Some(hint) => format!("{base}{hint}"), None => base }));
                                        }
                                        Err(message) => { recognition_notice.set(None); error.set(Some(message)); }
                                    }
                                });
                            },
                        }
                        if let Some(message) = recognition_notice() {
                            p { class: "notice", "{message}" }
                        }
                    }
                    if let Some(message) = error() {
                        p { class: "form-error", role: "alert", "{message}" }
                    }
                    div { class: "form-actions",
                        Button {
                            variant: ButtonVariant::Outline,
                            r#type: "button",
                            disabled: loading() || recognizing(),
                            onclick: move |_| {
                                discard_attachment_previews(&attachments());
                                on_close.call(());
                            },
                            if readonly {
                                "关闭"
                            } else {
                                "取消"
                            }
                        }
                        if !readonly {
                            Button {
                                r#type: "submit",
                                disabled: loading() || recognizing(),
                                if loading() {
                                    "正在上传并保存…"
                                } else {
                                    "确认保存"
                                }
                            }
                        }
                    }
                }
                };
                form
            }
        }
    }
}

/// 台账缺数据时的补录入口。
///
/// 在新标签页打开而不是原地跳转：跳走会把用户已经填好的租期、租金、递增规则
/// 连同选好的图片一起丢掉——表单状态在内存里，路由一换就没了。补完之后回到
/// 这个标签页，候选项会随订阅推送自动出现，一个字都不用重填。
#[component]
fn LedgerHint(park_id: u64, message: String) -> Element {
    rsx! {
        p { class: "hint",
            "{message}"
            Link {
                class: "hint-link",
                to: Route::RentalParkDetailPage { id: park_id },
                new_tab: true,
                " 打开园区详情补录 ↗"
            }
        }
    }
}
