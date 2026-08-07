//! 账单新增、查看、编辑、新增下月和删除弹窗。

use dioxus::prelude::*;

use super::fees::{
    basic_ele_fee_cents as fee_basic_ele_cents, basic_ele_note, collect_bases,
    contract_fee_amounts,
};
use super::model::{
    amount_input_value, available_carryover, carryover_item_label, format_money,
    next_month_project_name, optional_amount_input_value, parse_amount_to_cents,
    parse_optional_date, selected_carryover_cents,
};
use super::worksheet::{
    blank_utility_rows, drafts_from_contract, has_utility_rows, utility_inputs,
    utility_total_cents, BankAccountDraft, UtilityDraft, UtilityKind, UtilityWorksheet,
};
use crate::{
    components::{
        button::{Button, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        ConfirmDialog, DateField,
    },
    pages::billing::tenant_combobox::TenantCombobox,
    services::{
        confirm_bill_collected_record, confirm_bill_shortfall_record, create_amount_bill_record,
        delete_amount_bill_record, has_park, update_amount_bill_record, AiAmountBillDraft,
    },
    spacetime_bindings::{
        amount_bill_input_type::AmountBillInput, amount_bill_type::AmountBill, park_type::Park,
        rental_tenant_type::RentalTenant,
    },
    state::WorkspaceState,
};

#[component]
fn SheetMoneyInput(
    label: String,
    mut value: Signal<String, SyncStorage>,
    readonly: bool,
) -> Element {
    rsx! {
        // 用组件库的 `Input` 而不是裸 `<input>`：裸的没有 `.dx-input` 那套边框、
        // 高度和聚焦样式，和同一行里的日期选择器、按钮完全对不上。账单表单上
        // 十几个金额字段全走这一个组件，所以这里错一处就是整页错。
        Input {
            aria_label: label,
            value: value(),
            readonly,
            inputmode: "decimal",
            placeholder: "0.00",
            oninput: move |event: FormEvent| value.set(event.value()),
        }
    }
}

#[component]
fn BankAccountRow(
    account_type: String,
    mut account: Signal<BankAccountDraft, SyncStorage>,
    readonly: bool,
) -> Element {
    rsx! {
        div { class: "panel is-tight is-plain",
            strong { "{account_type}" }
            div { class: "form-grid",
                div { class: "field",
                    span { class: "field-label", "户名" }
                    Input {
                        value: account().name,
                        readonly,
                        placeholder: "请输入账户户名",
                        oninput: move |event: FormEvent| account.with_mut(|value| value.name = event.value()),
                    }
                }
                div { class: "field",
                    span { class: "field-label", "账号" }
                    Input {
                        inputmode: "numeric",
                        value: account().number,
                        readonly,
                        placeholder: "请输入银行账号",
                        oninput: move |event: FormEvent| account.with_mut(|value| value.number = event.value()),
                    }
                }
                div { class: "field",
                    span { class: "field-label", "开户行" }
                    Input {
                        value: account().bank,
                        readonly,
                        placeholder: "请输入开户行",
                        oninput: move |event: FormEvent| account.with_mut(|value| value.bank = event.value()),
                    }
                }
            }
        }
    }
}

#[component]
pub(super) fn BillFormDialog(
    bill: Option<AmountBill>,
    draft: Option<AiAmountBillDraft>,
    review_label: Option<String>,
    tenants: Vec<RentalTenant>,
    parks: Vec<Park>,
    create_as_new: bool,
    readonly: bool,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let state = use_context::<WorkspaceState>();
    let bill_id = (!create_as_new)
        .then(|| bill.as_ref().map(|row| row.bill_id))
        .flatten();
    let source = bill.clone();
    let initial_project = bill
        .as_ref()
        .map(|row| {
            if create_as_new {
                next_month_project_name(&row.project_name)
            } else {
                row.project_name.clone()
            }
        })
        .or_else(|| draft.as_ref().map(|row| row.project_name.clone()))
        .unwrap_or_default();
    let initial_tenant = bill
        .as_ref()
        .filter(|row| row.tenant_id != 0)
        .map(|row| row.tenant_id.to_string())
        .or_else(|| {
            draft.as_ref().and_then(|draft| {
                tenants
                    .iter()
                    .find(|tenant| tenant.tenant_name.trim() == draft.tenant_name.trim())
                    .map(|tenant| tenant.rental_tenant_id.to_string())
            })
        })
        .unwrap_or_default();
    let initial_park = bill
        .as_ref()
        .filter(|row| has_park(row.park_id))
        .map(|row| row.park_id.to_string())
        .or_else(|| {
            draft.as_ref().and_then(|draft| {
                parks
                    .iter()
                    .find(|park| park.park_name.trim() == draft.park_name.trim())
                    .map(|park| park.park_id.to_string())
            })
        })
        .unwrap_or_default();

    // 原系统的电表、水表明细属于账单子表；编辑和“新增下月”时一起带入。
    let source_bill_id = bill.as_ref().map(|row| row.bill_id);
    let initial_ele_rows = source_bill_id
        .map(|source_id| {
            (state.ele_bills)()
                .iter()
                .filter(|row| row.bill_id == source_id)
                .map(UtilityDraft::from_ele)
                .collect::<Vec<_>>()
        })
        .filter(|rows| !rows.is_empty())
        .unwrap_or_else(blank_utility_rows);
    let initial_water_rows = source_bill_id
        .map(|source_id| {
            (state.water_bills)()
                .iter()
                .filter(|row| row.bill_id == source_id)
                .map(UtilityDraft::from_water)
                .collect::<Vec<_>>()
        })
        .filter(|rows| !rows.is_empty())
        .unwrap_or_else(blank_utility_rows);

    let tenant_id = use_signal_sync(|| initial_tenant);
    let mut park_id = use_signal_sync(|| initial_park);
    let mut project_name = use_signal_sync(|| initial_project);
    let public_account = use_signal_sync(|| {
        BankAccountDraft::from_stored(
            bill.as_ref()
                .and_then(|row| row.public_bank_account.as_deref())
                .or_else(|| draft.as_ref().map(|row| row.public_bank_account.as_str())),
        )
    });
    let private_account = use_signal_sync(|| {
        BankAccountDraft::from_stored(
            bill.as_ref()
                .and_then(|row| row.private_bank_account.as_deref())
                .or_else(|| draft.as_ref().map(|row| row.private_bank_account.as_str())),
        )
    });
    let mut ele_rows = use_signal_sync(|| initial_ele_rows);
    let mut water_rows = use_signal_sync(|| initial_water_rows);
    let mut rent = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.factory_rent_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.factory_rent_cents))
            })
            .unwrap_or_default()
    });
    let management = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.management_fee_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.management_fee_cents))
            })
            .unwrap_or_default()
    });
    let mut garbage = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.garbage_fee_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.garbage_fee_cents))
            })
            .unwrap_or_default()
    });
    let mut service = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.service_fee_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.service_fee_cents))
            })
            .unwrap_or_default()
    });
    let mut extra_ele = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.extra_ele_fee_cents))
            .unwrap_or_default()
    });
    let mut basic_ele = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.basic_ele_fee_cents))
            .unwrap_or_default()
    });
    // 基本电费的算式随账单存一份快照：合同以后改了容量或单价，已开出去的账单
    // 不能跟着变——账单是对外的凭据，必须自带算式。
    let mut basic_capacity = use_signal_sync(|| bill.as_ref().and_then(|row| row.basic_ele_capacity_centi_kw));
    let mut basic_price = use_signal_sync(|| bill.as_ref().and_then(|row| row.basic_ele_price_scaled));
    let electricity = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.ele_fee_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.ele_fee_cents))
            })
            .unwrap_or_default()
    });
    let water = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.water_fee_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.water_fee_cents))
            })
            .unwrap_or_default()
    });
    let other_receivable = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.receive_fee_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.receive_fee_cents))
            })
            .unwrap_or_default()
    });
    let invoice_tax = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.invoice_tax_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.invoice_tax_cents))
            })
            .unwrap_or_default()
    });
    let penalty = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| optional_amount_input_value(row.penalty_fee_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.penalty_fee_cents))
            })
            .unwrap_or_default()
    });
    let mut total = use_signal_sync(|| {
        bill.as_ref()
            .map(|row| amount_input_value(row.total_fee_cents))
            .or_else(|| {
                draft
                    .as_ref()
                    .map(|row| amount_input_value(row.total_fee_cents))
            })
            .unwrap_or_default()
    });
    let receipt = use_signal_sync(|| {
        if create_as_new {
            String::new()
        } else {
            bill.as_ref()
                .map(|row| amount_input_value(row.receipt_amount_cents))
                .or_else(|| {
                    draft
                        .as_ref()
                        .map(|row| amount_input_value(row.receipt_amount_cents))
                })
                .unwrap_or_default()
        }
    });
    let mut receipt_date = use_signal_sync(|| {
        if create_as_new {
            String::new()
        } else {
            bill.as_ref()
                .map(|row| super::model::format_date(row.receipt_time))
                .filter(|value| value != "--")
                .or_else(|| draft.as_ref().map(|row| row.receipt_date.clone()))
                .unwrap_or_default()
        }
    });
    let mut remark = use_signal_sync(|| {
        bill.as_ref()
            .and_then(|row| row.remark.clone())
            .or_else(|| draft.as_ref().map(|row| row.remark.clone()))
            .unwrap_or_default()
    });
    let mut loading = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let mut completed = use_signal_sync(|| false);
    let tenants_for_effect = tenants.clone();
    let tenants_for_basic = tenants.clone();

    use_effect(move || {
        if completed() {
            on_saved.call(());
        }
    });

    // 所选合同约定的水电条款：表号、倍率、单价都从这里带出，用户只填读数。
    let contract_meters = use_memo(move || {
        let Ok(id) = tenant_id().parse::<u64>() else {
            return Vec::new();
        };
        (state.rental_tenant_meters)()
            .into_iter()
            .filter(|link| link.rental_tenant_id == id)
            .collect::<Vec<_>>()
    });
    let ele_prefill = use_memo(move || {
        drafts_from_contract(
            UtilityKind::Electricity,
            &(state.utility_meters)(),
            &contract_meters(),
        )
    });
    // 所选合同约定的周期性费用（电损／服务／垃圾）与基本电费条款。
    let contract_fees = use_memo(move || {
        let Ok(id) = tenant_id().parse::<u64>() else {
            return Vec::new();
        };
        (state.rental_tenant_fees)()
            .into_iter()
            .filter(|row| row.rental_tenant_id == id)
            .collect::<Vec<_>>()
    });
    let contract_basic = use_memo(move || {
        tenant_id()
            .parse::<u64>()
            .ok()
            .and_then(|id| {
                tenants_for_basic
                    .iter()
                    .find(|tenant| tenant.rental_tenant_id == id)
            })
            .map(|tenant| {
                (
                    tenant.basic_ele_capacity_centi_kw,
                    tenant.basic_ele_price_scaled,
                )
            })
            .unwrap_or((None, None))
    });
    let water_prefill = use_memo(move || {
        drafts_from_contract(UtilityKind::Water, &(state.utility_meters)(), &contract_meters())
    });
    // 合同约定了哪几项费用。约定了的项在账单上只读，由下面的 effect 算出来。
    let agreed_kinds = use_memo(move || {
        contract_fees()
            .iter()
            .map(|fee| fee.fee_kind.clone())
            .collect::<std::collections::BTreeSet<_>>()
    });
    // 选中合同后把合同里已经写着的东西全部带出来：园区、厂房租金、用表与单价。
    //
    // 「合同里有的，账单上不该再敲一遍」——租金是账单上最大的一个数，原来每个月
    // 都要重新手填，而它在合同里一直躺着。
    //
    // 用 `auto_filled_tenant` 记住已经带过哪一份合同：换合同要重新带，同一份合同
    // 不重复带，否则用户改完的数字会被下一次渲染冲掉。
    let mut auto_filled_tenant = use_signal_sync(|| 0_u64);
    use_effect(move || {
        let Some(tenant) = tenant_id().parse::<u64>().ok().and_then(|id| {
            tenants_for_effect
                .iter()
                .find(|tenant| tenant.rental_tenant_id == id)
        }) else {
            return;
        };
        park_id.set(tenant.park_id.to_string());
        if auto_filled_tenant() == tenant.rental_tenant_id {
            return;
        }
        auto_filled_tenant.set(tenant.rental_tenant_id);

        // 只填空字段：编辑旧账单或用户已经改过的数字不能被冲掉。
        if rent().trim().is_empty() {
            if let Some(amount) = tenant.rental_amount_cents.filter(|value| *value > 0) {
                rent.set(amount_input_value(amount));
            }
        }
        // 明细一行都没填时才带，已经抄了读数就不动——一次误覆盖等于整月白抄。
        let ele = ele_prefill();
        if !ele.is_empty() && !has_utility_rows(&ele_rows()) {
            ele_rows.set(ele);
        }
        let water = water_prefill();
        if !water.is_empty() && !has_utility_rows(&water_rows()) {
            water_rows.set(water);
        }
    });
    // 合同约定的费用随当期电费明细自动重算。
    //
    // 原来是一个「按合同带入费用」按钮，但服务端保存时会按同一套规则核对，不点
    // 那个按钮就必定被拒——留一个能填出必然失败的值的输入框没有意义。约定项因此
    // 改成只读 + 自动算，按钮去掉。
    //
    // 只写约定了的项：合同没约定的服务费、垃圾费仍然是手工项，被这里清零就等于
    // 把用户填的数字抹掉。
    use_effect(move || {
        let (capacity, price) = contract_basic();
        let basic_cents = fee_basic_ele_cents(capacity, price);
        if capacity.is_some() && price.is_some() {
            basic_ele.set(amount_input_value(basic_cents));
            basic_capacity.set(capacity);
            basic_price.set(price);
        }
        let fees = contract_fees();
        if fees.is_empty() {
            return;
        }
        let bases = collect_bases(&ele_rows(), &(state.utility_meters)(), basic_cents);
        let Ok(amounts) = contract_fee_amounts(&fees, &bases) else {
            return;
        };
        if let Some(value) = amounts.get("loss") {
            extra_ele.set(amount_input_value(*value));
        }
        if let Some(value) = amounts.get("service") {
            service.set(amount_input_value(*value));
        }
        if let Some(value) = amounts.get("garbage") {
            garbage.set(amount_input_value(*value));
        }
    });

    // 有明细就以明细为准，没有明细才认手填的那个数。
    let effective_ele_total = use_memo(move || {
        if has_utility_rows(&ele_rows()) {
            utility_total_cents(&ele_rows())
        } else {
            parse_amount_to_cents(&electricity(), "电费").unwrap_or_default()
        }
    });
    let effective_water_total = use_memo(move || {
        if has_utility_rows(&water_rows()) {
            utility_total_cents(&water_rows())
        } else {
            parse_amount_to_cents(&water(), "水费").unwrap_or_default()
        }
    });
    // 电损费和基本电费也要计入——它们是这次新加的收费项，漏在这里的话「项目明细
    // 上期结转：勾中的结转项 id。编辑既有账单时预填它已并入的那几笔，否则
    // 保存一次就把自己并入的结转弄丢了。
    let mut carryover_selected = use_signal_sync(|| {
        let editing = bill.as_ref().map(|row| row.bill_id).unwrap_or(0);
        if editing == 0 {
            return Vec::<u64>::new();
        }
        state
            .carryover_items
            .read()
            .iter()
            .filter(|item| item.consumed_by_bill_id == Some(editing))
            .map(|item| item.item_id)
            .collect()
    });
    let editing_bill_id = bill.as_ref().map(|row| row.bill_id).unwrap_or(0);
    let carryover_options = use_memo(move || {
        let Ok(id) = tenant_id().parse::<u64>() else {
            return Vec::new();
        };
        available_carryover(
            &state.carryover_items.read(),
            &state.carryover_batches.read(),
            id,
            editing_bill_id,
        )
    });
    let carryover_cents =
        use_memo(move || selected_carryover_cents(&carryover_options(), &carryover_selected()));

    // 合计」会比实际少收，而这个数正是「本月收费金额」的来源。
    let detail_total = use_memo(move || {
        [
            (rent(), "厂房租金"),
            (management(), "基本管理费"),
            (garbage(), "垃圾管理费"),
            (service(), "服务费"),
            (extra_ele(), "电损费"),
            (basic_ele(), "基本电费"),
            (other_receivable(), "其他应收"),
            (invoice_tax(), "开票税金"),
            (penalty(), "滞纳金"),
        ]
            .into_iter()
            .filter_map(|(value, field)| parse_amount_to_cents(&value, field).ok())
            .fold(0i64, i64::saturating_add)
            .saturating_add(effective_ele_total())
            .saturating_add(effective_water_total())
            // 结转额必须计入应收：不计的话对账时 receivable 少了这一块，
            // 上期欠的钱永远收不齐，循环还是闭不上。
            .saturating_add(carryover_cents())
    });

    // 「本月收费金额」空着时直接跟明细合计。各项都是合同带出来或明细算出来的，
    // 结果就摆在下面，还要人点一次「采用明细合计」没有道理。
    //
    // 只在空着时写：一旦用户手改过（谈了折扣、抹零），就不再覆盖。按钮留着，
    // 改完某一项之后可以再点一次重新同步。
    use_effect(move || {
        let value = detail_total();
        if readonly || value <= 0 || !total().trim().is_empty() {
            return;
        }
        total.set(amount_input_value(value));
    });
    let title = if readonly {
        "查看账单"
    } else if create_as_new && source.is_some() {
        "新增下月账单"
    } else if bill_id.is_some() {
        "编辑账单"
    } else if draft.is_some() {
        "AI 导入账单核对"
    } else {
        "新增账单"
    };

    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();

    rsx! {
        Dialog {
            class: "is-wide",
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { "{title}" }
            DialogDescription {
                if let Some(label) = review_label.as_deref() {
                    "{label} · 请按原收款通知单逐项核对。"
                } else {
                    "按收款通知单结构填写，水电明细会与账单原子保存。"
                }
            }
            {
                let form = rsx! {
                form {
                    onsubmit: move |event| {
                        event.prevent_default();
                        if readonly { on_close.call(()); return; }
                        if loading() { return; }
                        let Some(selected_tenant_id) = tenant_id().parse::<u64>().ok() else {
                            error.set(Some("请选择租赁客户".into())); return;
                        };
                        let Some(tenant) = tenants.iter().find(|row| row.rental_tenant_id == selected_tenant_id) else {
                            error.set(Some("选择的租赁客户不存在".into())); return;
                        };
                        let selected_park_id = park_id().parse::<u64>().unwrap_or(tenant.park_id);
                        if project_name().trim().is_empty() {
                            error.set(Some("请输入账单项目名称".into())); return;
                        }
                        let parse = |value: String, field: &str| parse_amount_to_cents(&value, field);
                        let receipt_time = match parse_optional_date(&receipt_date()) { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let ele_bills = match utility_inputs(&ele_rows(), receipt_time, "电费") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let water_bills = match utility_inputs(&water_rows(), receipt_time, "水费") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let ele_fee_cents = if has_utility_rows(&ele_rows()) { utility_total_cents(&ele_rows()) } else { match parse(electricity(), "电费") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } } };
                        let water_fee_cents = if has_utility_rows(&water_rows()) { utility_total_cents(&water_rows()) } else { match parse(water(), "水费") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } } };
                        let receive_fee_cents = match parse(other_receivable(), "其他应收") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let factory_rent_cents = match parse(rent(), "厂房租金") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let management_fee_cents = match parse(management(), "基本管理费") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let invoice_tax_cents = match parse(invoice_tax(), "开票税金") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let total_fee_cents = match parse(total(), "本月收费金额") { Ok(value) if value > 0 => value, Ok(_) => { error.set(Some("本月收费金额必须大于 0".into())); return; }, Err(message) => { error.set(Some(message)); return; } };
                        let service_fee_cents = match parse(service(), "服务费") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let garbage_fee_cents = match parse(garbage(), "垃圾管理费") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let extra_ele_fee_cents = match parse(extra_ele(), "电损费") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let basic_ele_fee_cents = match parse(basic_ele(), "基本电费") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let receipt_amount_cents = match parse(receipt(), "收款金额") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let penalty_fee_cents = if penalty().trim().is_empty() { None } else { match parse(penalty(), "滞纳金") { Ok(value) => Some(value), Err(message) => { error.set(Some(message)); return; } } };
                        if receipt_amount_cents > 0 && receipt_time.is_none() {
                            error.set(Some("录入收款金额后必须选择收款日期".into())); return;
                        }
                        if remark().chars().count() > 500 {
                            error.set(Some("备注不能超过 500 个字符".into())); return;
                        }
                        let public_bank_account = match public_account().to_stored("对公") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let private_bank_account = match private_account().to_stored("对私") { Ok(value) => value, Err(message) => { error.set(Some(message)); return; } };
                        let input = AmountBillInput {
                            project_name: project_name().trim().to_string(),
                            tenant_name: Some(tenant.tenant_name.clone()),
                            public_bank_account,
                            private_bank_account,
                            ele_fee_cents, water_fee_cents, receive_fee_cents, factory_rent_cents,
                            management_fee_cents, invoice_tax_cents, total_fee_cents, service_fee_cents,
                            garbage_fee_cents, receipt_amount_cents, penalty_fee_cents,
                            extra_ele_fee_cents, basic_ele_fee_cents,
                            basic_ele_capacity_centi_kw: basic_capacity(),
                            basic_ele_price_scaled: basic_price(),
                            service_rate_basis_points: source.as_ref().and_then(|row| row.service_rate_basis_points),
                            garbage_rate_basis_points: source.as_ref().and_then(|row| row.garbage_rate_basis_points),
                            penalty_rate_basis_points: source.as_ref().and_then(|row| row.penalty_rate_basis_points),
                            extra_ele_rate_basis_points: source.as_ref().and_then(|row| row.extra_ele_rate_basis_points),
                            penalty_item: source.as_ref().and_then(|row| row.penalty_item.clone()),
                            extra_ele_item: source.as_ref().and_then(|row| row.extra_ele_item.clone()),
                            ele_item: source.as_ref().and_then(|row| row.ele_item.clone()),
                            water_item: source.as_ref().and_then(|row| row.water_item.clone()),
                            extra_project_item: source.as_ref().and_then(|row| row.extra_project_item.clone()),
                            carryover_fee_cents: carryover_cents(),
                            carryover_item: carryover_item_label(&carryover_options(), &carryover_selected()),
                            consumed_carryover_item_ids: carryover_selected(),
                            tax_rate_json: source.as_ref().and_then(|row| row.tax_rate_json.clone()),
                            remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                            receipt_time,
                            tenant_id: Some(selected_tenant_id), park_id: Some(selected_park_id),
                            ele_bills, water_bills,
                        };
                        error.set(None); loading.set(true);
                        spawn(async move {
                            let result = if let Some(id) = bill_id {
                                update_amount_bill_record(id, input).await
                            } else {
                                create_amount_bill_record(input).await
                            };
                            loading.set(false);
                            match result { Ok(()) => completed.set(true), Err(message) => error.set(Some(message)) }
                        });
                    },
                    div { class: "filters",
                        div { class: "field",
                            span { class: "field-label", "收款时间" }
                            DateField {
                                value: receipt_date(),
                                disabled: readonly,
                                on_change: move |value: String| receipt_date.set(value),
                            }
                        }
                        if !readonly {
                            div { class: "field",
                                Button {
                                    variant: ButtonVariant::Outline,
                                    r#type: "button",
                                    onclick: move |_| receipt_date.set(crate::pages::smart_meter::date::today()),
                                    "同步今日"
                                }
                            }
                        }
                        div { class: "field",
                            span { class: "field-label", "收款金额（元）" }
                            SheetMoneyInput { label: "收款金额", value: receipt, readonly }
                        }
                    }
                    div { class: "stack",
                        section { class: "subsection",
                            h4 { "收款通知单" }
                            p { class: "hint", "先确认租户、园区和本期账单项目" }
                            div { class: "form-grid",
                                div { class: "field",
                                    span { class: "field-label", "租户名称" }
                                    TenantCombobox { tenants: tenants.clone(), selected_id: tenant_id, readonly }
                                }
                                div { class: "field",
                                    Label { html_for: "bill-form-park", "所属园区" }
                                    Select {
                                        id: "bill-form-park",
                                        value: Some(park_value),
                                        disabled: readonly,
                                        on_value_change: move |value: Option<String>| park_id.set(value.unwrap_or_default()),
                                        SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                                        for (index , park) in parks.iter().enumerate() {
                                            SelectOption::<String> {
                                                key: "bill-form-park-{park.park_id}",
                                                value: park.park_id.to_string(),
                                                index: index + 1,
                                                text_value: park.park_name.to_string(),
                                                "{park.park_name}"
                                            }
                                        }
                                    }
                                }
                                div { class: "field is-wide",
                                    Label { html_for: "bill-project-name", "项目名称" }
                                    Input {
                                        id: "bill-project-name",
                                        value: project_name(),
                                        readonly,
                                        placeholder: "例如：2026年7月租金账单",
                                        oninput: move |event: FormEvent| project_name.set(event.value()),
                                    }
                                }
                            }
                        }
                        UtilityWorksheet {
                            kind: UtilityKind::Electricity,
                            rows: ele_rows,
                            readonly,
                            prefill: ele_prefill(),
                            meters: (state.utility_meters)(),
                        }
                        UtilityWorksheet {
                            kind: UtilityKind::Water,
                            rows: water_rows,
                            readonly,
                            prefill: water_prefill(),
                            meters: (state.utility_meters)(),
                        }
                        section { class: "subsection",
                            h4 { "项目合计" }
                            p { class: "hint", "水电费用自动汇总，其余项目按实际金额填写" }
                            div { class: "form-grid",
                                div { class: "field",
                                    span { class: "field-label", "电费合计" }
                                    span { class: "field-static is-mono", "{format_money(effective_ele_total())}" }
                                    small { class: "hint", "由电费明细自动计算" }
                                }
                                div { class: "field",
                                    span { class: "field-label", "水费合计" }
                                    span { class: "field-static is-mono", "{format_money(effective_water_total())}" }
                                    small { class: "hint", "由水费明细自动计算" }
                                }
                                div { class: "field",
                                    span { class: "field-label", "厂房租金" }
                                    SheetMoneyInput { label: "厂房租金", value: rent, readonly }
                                }
                                div { class: "field",
                                    span { class: "field-label", "基本管理费" }
                                    SheetMoneyInput { label: "基本管理费", value: management, readonly }
                                }
                                div { class: "field",
                                    span { class: "field-label",
                                        "垃圾管理费"
                                        if agreed_kinds().contains("garbage") { " · 合同" }
                                    }
                                    SheetMoneyInput {
                                        label: "垃圾管理费",
                                        value: garbage,
                                        readonly: readonly || agreed_kinds().contains("garbage"),
                                    }
                                }
                                div { class: "field",
                                    span { class: "field-label",
                                        "服务费"
                                        if agreed_kinds().contains("service") { " · 合同" }
                                    }
                                    SheetMoneyInput {
                                        label: "服务费",
                                        value: service,
                                        readonly: readonly || agreed_kinds().contains("service"),
                                    }
                                }
                                div { class: "field",
                                    span { class: "field-label",
                                        "电损费"
                                        if agreed_kinds().contains("loss") { " · 合同" }
                                    }
                                    SheetMoneyInput {
                                        label: "电损费",
                                        value: extra_ele,
                                        readonly: readonly || agreed_kinds().contains("loss"),
                                    }
                                }
                                div { class: "field",
                                    span { class: "field-label",
                                        "基本电费"
                                        if contract_basic().0.is_some() { " · 合同" }
                                    }
                                    SheetMoneyInput {
                                        label: "基本电费",
                                        value: basic_ele,
                                        readonly: readonly || contract_basic().0.is_some(),
                                    }
                                    if let Some(note) = basic_ele_note(basic_capacity(), basic_price()) {
                                        small { class: "hint", "{note}" }
                                    }
                                }
                                div { class: "field",
                                    span { class: "field-label", "其他应收" }
                                    SheetMoneyInput { label: "其他应收", value: other_receivable, readonly }
                                }
                                div { class: "field",
                                    span { class: "field-label", "开票税金" }
                                    SheetMoneyInput { label: "开票税金", value: invoice_tax, readonly }
                                }
                                div { class: "field",
                                    span { class: "field-label", "滞纳金" }
                                    SheetMoneyInput { label: "滞纳金", value: penalty, readonly }
                                }
                                div { class: "field",
                                    span { class: "field-label", "本月收费金额" }
                                    SheetMoneyInput { label: "本月收费金额", value: total, readonly }
                                }
                            }
                            // 上期结转：勾一笔，本期应收就多一笔，并在保存时
                            // 与账单同事务标记为已消费。不勾就不消费——宁可这
                            // 个月没结转，也不能让同一笔差额被收两遍。
                            if !carryover_options().is_empty() {
                                div { class: "panel is-tight",
                                    div { class: "row is-between",
                                        strong { "上期结转" }
                                        span { class: "is-mono",
                                            "已选 {carryover_selected().len()} 笔 · {format_money(carryover_cents())}"
                                        }
                                    }
                                    p { class: "hint",
                                        "勾选后单独计入本期应收，不并进租金。账单上会显示为独立一项，租户能看出哪部分是上期欠的。"
                                    }
                                    for option in carryover_options() {
                                        {
                                            let item_id = option.item_id;
                                            let checked = carryover_selected().contains(&item_id);
                                            rsx! {
                                                label {
                                                    key: "carryover-{item_id}",
                                                    class: "row is-between is-clickable",
                                                    div { class: "row",
                                                        input {
                                                            r#type: "checkbox",
                                                            checked,
                                                            disabled: readonly,
                                                            onchange: move |_| {
                                                                let mut next = carryover_selected();
                                                                if let Some(at) = next.iter().position(|id| *id == item_id) {
                                                                    next.remove(at);
                                                                } else {
                                                                    next.push(item_id);
                                                                }
                                                                carryover_selected.set(next);
                                                            }
                                                        }
                                                        div { class: "stack-tight",
                                                            span { "{option.period_label} · {option.project_name}" }
                                                            small { class: "hint", "上期未收齐" }
                                                        }
                                                    }
                                                    span { class: "is-mono", "{format_money(option.shortfall_cents)}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if !agreed_kinds().is_empty() || contract_basic().0.is_some() {
                                p { class: "hint",
                                    "带「合同」标记的费用由合同约定和当期电费明细算出，不能在账单上改——服务端保存时会按同一套规则再算一遍核对。要改金额请去改合同约定。"
                                }
                            }
                            // 明细合计和手填的总额可能对不上，给出对照并支持一键采用
                            div { class: "panel is-tight is-plain",
                                div { class: "section-header",
                                    span { class: "field-label", "项目明细合计" }
                                    strong { class: "is-mono", "{format_money(detail_total())}" }
                                }
                                if !readonly {
                                    div { class: "row",
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            size: crate::components::button::ButtonSize::Sm,
                                            r#type: "button",
                                            onclick: move |_| total.set(amount_input_value(detail_total())),
                                            "采用明细合计"
                                        }
                                    }
                                }
                            }
                        }
                        section { class: "subsection",
                            h4 { "收款账户" }
                            p { class: "hint", "户名、账号和开户行分别保存，可按实际需要填写" }
                            div { class: "stack",
                                BankAccountRow { account_type: "对公账户", account: public_account, readonly }
                                BankAccountRow { account_type: "对私账户", account: private_account, readonly }
                            }
                            div { class: "field",
                                Label { html_for: "bill-remark", "账单备注" }
                                Textarea {
                                    id: "bill-remark",
                                    value: remark(),
                                    readonly,
                                    rows: 3,
                                    placeholder: "填写账单说明、收款备注或异常信息",
                                    oninput: move |event: FormEvent| remark.set(event.value()),
                                }
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
                            if readonly {
                                "关闭"
                            } else {
                                "取消"
                            }
                        }
                        if !readonly {
                            Button {
                                r#type: "submit",
                                disabled: loading(),
                                if loading() {
                                    "正在保存…"
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

/// 收缴确认弹窗，两个确认动作共用——`collected` 决定是确认收齐还是确认差额。
///
/// 描述里把应收、实收、差额摆出来，让经理确认的是**数字**而不是一句空话；
/// 快照与权限由服务端 Reducer 落实，这里只是入口。
#[component]
pub(super) fn BillConfirmDialog(
    bill: AmountBill,
    collected: bool,
    on_close: EventHandler<()>,
    on_confirmed: EventHandler<()>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut completed = use_signal(|| false);
    let bill_id = bill.bill_id;
    let receivable = format_money(bill.total_fee_cents);
    let received = format_money(bill.receipt_amount_cents);
    let shortfall = format_money(
        bill.total_fee_cents
            .saturating_sub(bill.receipt_amount_cents)
            .max(0),
    );

    use_effect(move || {
        if completed() {
            on_confirmed.call(());
        }
    });

    let (title, description, confirm_label) = if collected {
        (
            "确认已收齐",
            format!(
                "「{}」应收 {receivable}，实收 {received}。确认后本张账单收缴闭环，确认人、时间与金额快照将一并记录。",
                bill.project_name
            ),
            "确认已收齐",
        )
    } else {
        (
            "确认差额",
            format!(
                "「{}」应收 {receivable}，实收 {received}，差额 {shortfall}。确认仅表示知悉，不会闭环；确认后请继续向租户催交。",
                bill.project_name
            ),
            "确认差额",
        )
    };

    rsx! {
        ConfirmDialog {
            title,
            description,
            confirm_label,
            busy: loading(),
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() {
                    return;
                }
                loading.set(true);
                error.set(None);
                spawn(async move {
                    let result = if collected {
                        confirm_bill_collected_record(bill_id).await
                    } else {
                        confirm_bill_shortfall_record(bill_id).await
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
        }
    }
}

#[component]
pub(super) fn BillDeleteDialog(
    bill: AmountBill,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut completed = use_signal(|| false);
    let bill_id = bill.bill_id;
    let project_name = bill.project_name.clone();

    use_effect(move || {
        if completed() {
            on_deleted.call(());
        }
    });

    rsx! {
        ConfirmDialog {
            title: "确认删除账单",
            description: format!(
                "将永久删除“{project_name}”及其水电明细和催收日志，关联财务流水会被归档。此操作不可撤销。",
            ),
            confirm_label: "确认删除",
            busy: loading(),
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() {
                    return;
                }
                loading.set(true);
                error.set(None);
                spawn(async move {
                    match delete_amount_bill_record(bill_id).await {
                        Ok(()) => completed.set(true),
                        Err(message) => {
                            loading.set(false);
                            error.set(Some(message));
                        }
                    }
                });
            },
        }
    }
}
