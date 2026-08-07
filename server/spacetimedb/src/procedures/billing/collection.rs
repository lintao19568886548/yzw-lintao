//! 账单催收短信发送、模板选择和审计记录。

use spacetimedb::{ProcedureContext, SpacetimeType, Table};

use crate::{
    reducers::shared::access::{require_admin, require_amount_bill},
    sms::send_template_message,
    tables::{
        AmountBill, AmountBillCollectionSmsLog, Park, RentalTenant, SmsProviderConfig,
        amount_bill_collection_sms_log, park, rental_tenant, sms_provider_config,
    },
};

#[derive(SpacetimeType)]
pub struct CollectionSmsSendItem {
    pub bill_id: u64,
    pub success: bool,
    pub message: String,
}

#[derive(SpacetimeType)]
pub struct CollectionSmsBatchResult {
    pub results: Vec<CollectionSmsSendItem>,
}

struct CollectionCandidate {
    bill: AmountBill,
    tenant: RentalTenant,
    remaining_cents: i64,
    company_name: String,
    config: Option<SmsProviderConfig>,
}

/// 发送选中账单的催收短信。所有数据库读取和写入均位于显式事务中，HTTP 在事务外执行。
#[spacetimedb::procedure]
pub fn send_collection_sms(
    ctx: &mut ProcedureContext,
    bill_ids: Vec<u64>,
    collection_type: String,
    due_date: String,
    overdue_days: u32,
) -> CollectionSmsBatchResult {
    let collection_type = normalize_collection_type(&collection_type);
    if collection_type.is_empty() {
        return batch_error(&bill_ids, "催收类型无效");
    }
    let mut unique_ids = bill_ids;
    unique_ids.sort_unstable();
    unique_ids.dedup();
    if unique_ids.is_empty() || unique_ids.len() > 100 {
        return batch_error(&unique_ids, "单次请选择 1 到 100 份账单");
    }
    let candidates = ctx.try_with_tx(|tx| -> Result<Vec<CollectionCandidate>, String> {
        require_admin(tx)?;
        unique_ids
            .iter()
            .map(|bill_id| {
                let bill = require_amount_bill(tx, *bill_id)?;
                let tenant = tx
                    .db
                    .rental_tenant()
                    .rental_tenant_id()
                    .find(bill.tenant_id)
                    .filter(|tenant| !tenant.is_deleted)
                    .ok_or_else(|| format!("账单 #{bill_id} 未关联有效租户"))?;
                let park = tx
                    .db
                    .park()
                    .park_id()
                    .find(bill.park_id)
                    .filter(|park| !park.is_deleted)
                    .ok_or_else(|| format!("账单 #{bill_id} 未关联有效园区"))?;
                let remaining_cents = bill
                    .total_fee_cents
                    .saturating_sub(bill.receipt_amount_cents)
                    .max(0);
                let company_name = resolve_company_name(&bill, &park);
                let config = (!company_name.is_empty())
                    .then(|| collection_config_key(&company_name, &collection_type))
                    .and_then(|key| tx.db.sms_provider_config().config_key().find(key));
                Ok(CollectionCandidate {
                    bill,
                    tenant,
                    remaining_cents,
                    company_name,
                    config,
                })
            })
            .collect()
    });
    let candidates = match candidates {
        Ok(candidates) => candidates,
        Err(message) => return batch_error(&unique_ids, message),
    };

    let mut results = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let (success, message, provider_json, template_id) =
            send_candidate(ctx, &candidate, &collection_type, &due_date, overdue_days);
        record_result(
            ctx,
            &candidate,
            &collection_type,
            template_id,
            success,
            &message,
            provider_json,
        );
        results.push(CollectionSmsSendItem {
            bill_id: candidate.bill.bill_id,
            success,
            message,
        });
    }
    CollectionSmsBatchResult { results }
}

/// 判断一份账单该不该催收所需的全部输入。
///
/// 刻意收成一组普通字段而不是直接传 `CollectionCandidate`：这些值分别来自
/// 账单、租户和园区三张表，摊平之后判断逻辑不再依赖任何数据库行，可以直接
/// 断言。
struct CollectionRequest<'a> {
    remaining_cents: i64,
    phone_number: &'a str,
    tenant_name: &'a str,
    company_name: &'a str,
    project_name: &'a str,
    has_config: bool,
    collection_type: &'a str,
    due_date: &'a str,
    overdue_days: u32,
}

/// 一份账单的处置决定，含义同合同提醒的 `ReminderPlan`。
#[derive(Debug, PartialEq, Eq)]
enum CollectionPlan {
    /// 不发这条，附带给操作者看的理由。
    Skip(String),
    /// 要发，短信通道需要的四个模板参数已经算好。
    Send { phone: String, params: Vec<String> },
}

/// 决定这份账单该不该催收、短信里填什么。
///
/// 判断顺序即对外可见的行为：批量结果里操作者看到的是先命中的那条理由。
fn plan_collection(request: &CollectionRequest<'_>) -> CollectionPlan {
    if request.remaining_cents <= 0 {
        return CollectionPlan::Skip("账单已经结清，无需催收".into());
    }
    let phone = normalize_phone(request.phone_number);
    if phone.len() != 11 || !phone.starts_with('1') {
        return CollectionPlan::Skip("租户手机号格式不正确".into());
    }
    if request.company_name.is_empty() {
        return CollectionPlan::Skip("未匹配到短信企业主体".into());
    }
    if !request.has_config {
        return CollectionPlan::Skip(format!(
            "{}的{}模板尚未配置",
            request.company_name,
            collection_type_label(request.collection_type)
        ));
    }
    CollectionPlan::Send {
        phone,
        params: vec![
            request.tenant_name.to_string(),
            resolve_bill_period(request.project_name),
            format_cents(request.remaining_cents),
            if request.collection_type == "payment_reminder" {
                normalize_due_date(request.due_date)
            } else {
                // 逾期天数只兜底不封顶：真实逾期比档位长时报实际天数。
                format!(
                    "{}天",
                    request
                        .overdue_days
                        .max(if request.collection_type == "final_30" {
                            30
                        } else {
                            10
                        })
                )
            },
        ],
    }
}

/// 执行段：唯一真正把催收短信发出去的地方。
fn send_candidate(
    ctx: &mut ProcedureContext,
    candidate: &CollectionCandidate,
    collection_type: &str,
    due_date: &str,
    overdue_days: u32,
) -> (bool, String, Option<String>, Option<String>) {
    let plan = plan_collection(&CollectionRequest {
        remaining_cents: candidate.remaining_cents,
        phone_number: &candidate.tenant.phone_number,
        tenant_name: &candidate.tenant.tenant_name,
        company_name: &candidate.company_name,
        project_name: &candidate.bill.project_name,
        has_config: candidate.config.is_some(),
        collection_type,
        due_date,
        overdue_days,
    });
    let (phone, params) = match plan {
        CollectionPlan::Skip(reason) => return (false, reason, None, None),
        CollectionPlan::Send { phone, params } => (phone, params),
    };
    // 配置缺失时 `plan_collection` 已经返回 Skip，这里取不到只可能是两边判断漂移。
    let Some(config) = candidate.config.as_ref() else {
        return (false, "短信模板尚未配置".into(), None, None);
    };
    match send_template_message(ctx, config, &phone, &params) {
        Ok(provider_json) => (
            true,
            format!("已向 {} 发送催收短信", mask_phone(&phone)),
            Some(provider_json),
            Some(config.template_id.clone()),
        ),
        Err(error) => (false, error, None, Some(config.template_id.clone())),
    }
}

#[allow(clippy::too_many_arguments)]
fn record_result(
    ctx: &mut ProcedureContext,
    candidate: &CollectionCandidate,
    collection_type: &str,
    template_id: Option<String>,
    success: bool,
    message: &str,
    provider_json: Option<String>,
) {
    let phone_number = normalize_phone(&candidate.tenant.phone_number);
    let customer_id = candidate.bill.customer_id.clone();
    let bill_id = candidate.bill.bill_id;
    let collection_type = collection_type.to_string();
    let tenant_name = candidate.tenant.tenant_name.clone();
    let project_name = candidate.bill.project_name.clone();
    let remaining_cents = candidate.remaining_cents;
    let error = (!success).then(|| message.to_string());
    let timestamp = ctx.timestamp;
    ctx.with_tx(|tx| {
        tx.db
            .amount_bill_collection_sms_log()
            .insert(AmountBillCollectionSmsLog {
                log_id: 0,
                customer_id: customer_id.clone(),
                bill_id,
                collection_type: collection_type.clone(),
                template_id: template_id.clone(),
                phone_number: (!phone_number.is_empty()).then(|| phone_number.clone()),
                tenant_name: Some(tenant_name.clone()),
                project_name: Some(project_name.clone()),
                remaining_amount_cents: Some(remaining_cents),
                success,
                error: error.clone(),
                provider_result_json: provider_json.clone(),
                sent_at: timestamp,
                created_at: timestamp,
            });
    });
}

pub(crate) fn collection_config_key(company_name: &str, collection_type: &str) -> String {
    format!("collection:{company_name}:{collection_type}")
}

#[pure_function::pure]
fn normalize_collection_type(value: &str) -> String {
    match value.trim() {
        "payment_reminder" | "overdue_10" | "final_30" => value.trim().into(),
        _ => String::new(),
    }
}

fn collection_type_label(value: &str) -> &'static str {
    match value {
        "overdue_10" => "逾期提醒",
        "final_30" => "长期未结提醒",
        _ => "缴费提醒",
    }
}

#[pure_function::pure]
fn normalize_phone(value: &str) -> String {
    value.chars().filter(char::is_ascii_digit).collect()
}

fn mask_phone(value: &str) -> String {
    if value.len() == 11 {
        format!("{}****{}", &value[..3], &value[7..])
    } else {
        "租户手机".into()
    }
}

#[pure_function::pure]
fn normalize_due_date(value: &str) -> String {
    let value = value.trim();
    if value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        value.into()
    } else {
        "本月5日".into()
    }
}

fn resolve_bill_period(value: &str) -> String {
    let bytes = value.as_bytes();
    for index in 0..bytes.len().saturating_sub(5) {
        if index + 4 <= bytes.len() && bytes[index..index + 4].iter().all(u8::is_ascii_digit) {
            let tail = &value[index + 4..];
            let tail = tail.strip_prefix('年').or_else(|| tail.strip_prefix('-'));
            if let Some(tail) = tail {
                let month = tail
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .take(2)
                    .collect::<Vec<_>>();
                if !month.is_empty() {
                    let month = String::from_utf8(month).unwrap_or_default();
                    if let Ok(month) = month.parse::<u8>()
                        && (1..=12).contains(&month)
                    {
                        return format!("{}年{}月", &value[index..index + 4], month);
                    }
                }
            }
        }
    }
    value.chars().take(30).collect()
}

fn format_cents(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents.unsigned_abs() % 100)
}

fn resolve_company_name(bill: &AmountBill, park: &Park) -> String {
    if let Some(account) = bill.public_bank_account.as_deref()
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(account)
        && let Some(name) = value.get("name").and_then(serde_json::Value::as_str)
        && !name.trim().is_empty()
        && !matches!(name.trim(), "喻必胜" | "肖德利")
    {
        return name.trim().into();
    }
    match park.park_name.as_str() {
        "东莞光泰园区" => "东莞市亿胜物业管理有限公司",
        "东莞同兴园区" => "东莞市十一兄弟实业投资有限公司",
        "东莞同富园区" => "东莞市启程物业管理有限公司",
        "佛山乐从园区" => "佛山市十一智创物业管理有限公司",
        "佛山九江园区" => "佛山市十一智慧家具有限公司",
        "广州园区（西州一）" | "广州荔新" | "广州西州二园区" => {
            "广州市十一兄弟产业投资有限公司"
        }
        "深圳坪山23园区" => "深圳市十一兄弟产业服务有限公司",
        _ => "",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一份「本该催收」的账单：欠 1234.50 元、手机号合法、企业主体和模板都在。
    /// 每个用例只改自己关心的那一个字段。
    fn request<'a>() -> CollectionRequest<'a> {
        CollectionRequest {
            remaining_cents: 123_450,
            phone_number: "13800138000",
            tenant_name: "张三",
            company_name: "东莞市亿胜物业管理有限公司",
            project_name: "2025年7月租金",
            has_config: true,
            collection_type: "payment_reminder",
            due_date: "2025-07-05",
            overdue_days: 0,
        }
    }

    fn skip_reason(plan: CollectionPlan) -> String {
        match plan {
            CollectionPlan::Skip(reason) => reason,
            CollectionPlan::Send { phone, .. } => panic!("本不该发，却计划发给 {phone}"),
        }
    }

    fn send_params(plan: CollectionPlan) -> Vec<String> {
        match plan {
            CollectionPlan::Send { params, .. } => params,
            CollectionPlan::Skip(reason) => panic!("本该发，却被跳过：{reason}"),
        }
    }

    #[test]
    fn 结清和超收的账单都不再催收() {
        let mut input = request();
        for remaining in [0, -1] {
            input.remaining_cents = remaining;
            assert_eq!(
                skip_reason(plan_collection(&input)),
                "账单已经结清，无需催收"
            );
        }
    }

    #[test]
    fn 手机号不合规不发() {
        let mut input = request();
        for bad in ["1380013800", "138001380001", "23800138000", ""] {
            input.phone_number = bad;
            assert_eq!(
                skip_reason(plan_collection(&input)),
                "租户手机号格式不正确",
                "手机号 {bad:?} 应当拦下"
            );
        }
    }

    #[test]
    fn 缺企业主体或模板时理由要说清是哪一种() {
        let mut input = request();
        input.company_name = "";
        assert_eq!(skip_reason(plan_collection(&input)), "未匹配到短信企业主体");

        // 三种催收类型各自有独立模板，理由里要点明缺的是哪一个。
        let mut input = request();
        input.has_config = false;
        for (kind, label) in [
            ("payment_reminder", "缴费提醒"),
            ("overdue_10", "逾期提醒"),
            ("final_30", "长期未结提醒"),
        ] {
            input.collection_type = kind;
            assert_eq!(
                skip_reason(plan_collection(&input)),
                format!("东莞市亿胜物业管理有限公司的{label}模板尚未配置")
            );
        }
    }

    #[test]
    fn 缴费提醒填到期日非法日期回落到本月五日() {
        let mut input = request();
        assert_eq!(send_params(plan_collection(&input))[3], "2025-07-05");

        for bad in ["", "2025/07/05", "2025-7-5", "下个月初", "20250705"] {
            input.due_date = bad;
            assert_eq!(
                send_params(plan_collection(&input))[3],
                "本月5日",
                "日期 {bad:?} 应当回落"
            );
        }
    }

    #[test]
    fn 逾期类催收填天数且不低于档位() {
        let mut input = request();
        input.collection_type = "overdue_10";
        input.overdue_days = 3;
        assert_eq!(send_params(plan_collection(&input))[3], "10天");
        input.overdue_days = 17;
        assert_eq!(send_params(plan_collection(&input))[3], "17天");

        input.collection_type = "final_30";
        input.overdue_days = 17;
        assert_eq!(send_params(plan_collection(&input))[3], "30天");
        input.overdue_days = 45;
        assert_eq!(send_params(plan_collection(&input))[3], "45天");
    }

    #[test]
    fn 模板参数依次是租户账期金额与期限() {
        assert_eq!(
            send_params(plan_collection(&request())),
            vec!["张三", "2025年7月", "1234.50", "2025-07-05"]
        );
    }

    #[test]
    fn 账期从项目名里认出年月认不出就截断原文() {
        assert_eq!(resolve_bill_period("2025年7月租金"), "2025年7月");
        assert_eq!(resolve_bill_period("2025-07 水电费"), "2025年7月");
        assert_eq!(resolve_bill_period("2025年13月"), "2025年13月");
        assert_eq!(resolve_bill_period("押金"), "押金");
    }

    #[test]
    fn 金额按元角分展示() {
        assert_eq!(format_cents(123_450), "1234.50");
        assert_eq!(format_cents(5), "0.05");
        assert_eq!(format_cents(100), "1.00");
    }

    #[test]
    fn 手机号在回执里打码() {
        assert_eq!(mask_phone("13800138000"), "138****8000");
        assert_eq!(mask_phone("1380013800"), "租户手机");
    }
}

fn batch_error(bill_ids: &[u64], message: impl Into<String>) -> CollectionSmsBatchResult {
    let message = message.into();
    CollectionSmsBatchResult {
        results: bill_ids
            .iter()
            .map(|bill_id| CollectionSmsSendItem {
                bill_id: *bill_id,
                success: false,
                message: message.clone(),
            })
            .collect(),
    }
}
