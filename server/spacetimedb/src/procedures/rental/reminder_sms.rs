//! 合同到期及租金递增提醒短信。

use spacetimedb::{ProcedureContext, SpacetimeType, Timestamp};

use crate::{
    reducers::shared::access::require_admin,
    sms::send_personal_message,
    tables::{RentalTenant, SmsProviderConfig, rental_tenant, sms_provider_config},
};

const DAY_MICROS: i64 = 86_400_000_000;
const REMINDER_DAYS: i64 = 90;
const INCREASE_REMINDER_DAYS: i64 = 30;
const REMINDER_TEMPLATE: &str =
    "尊敬的{%1%}，您好！您的合同递增比例将于 {%2%} 进行变更，合同到期日是{%3%}，请留意查收。";

#[derive(SpacetimeType)]
pub struct ContractSmsSendItem {
    pub rental_tenant_id: u64,
    pub success: bool,
    pub message: String,
}

#[derive(SpacetimeType)]
pub struct ContractSmsBatchResult {
    pub results: Vec<ContractSmsSendItem>,
}

struct ContractCandidate {
    tenant: RentalTenant,
    config: Option<SmsProviderConfig>,
}

/// 发送选中合同的到期提醒；批量与单条操作共用这一条受权限保护的流程。
#[spacetimedb::procedure]
pub fn send_contract_reminder_sms(
    ctx: &mut ProcedureContext,
    rental_tenant_ids: Vec<u64>,
    manual: bool,
) -> ContractSmsBatchResult {
    let mut ids = rental_tenant_ids;
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() || ids.len() > 100 {
        return batch_error(&ids, "单次请选择 1 到 100 份合同");
    }
    let candidates = ctx.try_with_tx(|tx| -> Result<Vec<ContractCandidate>, String> {
        require_admin(tx)?;
        ids.iter()
            .map(|id| {
                let tenant = tx
                    .db
                    .rental_tenant()
                    .rental_tenant_id()
                    .find(*id)
                    .filter(|tenant| !tenant.is_deleted)
                    .ok_or_else(|| format!("合同 #{id} 不存在或已删除"))?;
                Ok(ContractCandidate {
                    tenant,
                    config: tx
                        .db
                        .sms_provider_config()
                        .config_key()
                        .find("login".to_string()),
                })
            })
            .collect()
    });
    let candidates = match candidates {
        Ok(rows) => rows,
        Err(message) => return batch_error(&ids, message),
    };

    let mut results = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let result = send_candidate(ctx, &candidate, manual);
        if result.is_ok() {
            let id = candidate.tenant.rental_tenant_id;
            let sent_at = ctx.timestamp;
            ctx.with_tx(|tx| {
                if let Some(mut tenant) = tx.db.rental_tenant().rental_tenant_id().find(id) {
                    tenant.send_message_at = Some(sent_at);
                    tenant.updated_at = Some(sent_at);
                    tx.db.rental_tenant().rental_tenant_id().update(tenant);
                }
            });
        }
        results.push(ContractSmsSendItem {
            rental_tenant_id: candidate.tenant.rental_tenant_id,
            success: result.is_ok(),
            message: result.unwrap_or_else(|message| message),
        });
    }
    ContractSmsBatchResult { results }
}

/// 一份合同的处置决定。
///
/// 判断需要的东西——合同行、供应商配置在不在、当前时间、是否手动触发——在
/// 调用之前全部已知，所以这是一份**纯数据**：算出它不碰数据库、不碰短信
/// 通道，可以直接断言。真正的运行时未知只有「短信发出去成功没有」一个，
/// 那个留给下面的执行段用 `?` 处理。
#[derive(Debug, PartialEq, Eq)]
enum ReminderPlan {
    /// 不发这条，附带给操作者看的理由。
    Skip(String),
    /// 要发，短信通道需要的参数已经算好。
    Send { phone: String, params: Vec<String> },
}

/// 决定这份合同该不该发提醒、发什么。
///
/// 判断顺序即对外可见的行为：同一份合同可能同时不满足好几条，先命中哪条就
/// 回哪条理由，操作者在批量结果里看到的就是它。调整顺序等于改变用户看到的
/// 提示，不要当成实现细节。
fn plan_reminder(
    tenant: &RentalTenant,
    has_config: bool,
    now: Timestamp,
    manual: bool,
) -> ReminderPlan {
    if !manual && !tenant.transaction_type {
        return ReminderPlan::Skip("支出合同不发送到期提醒".into());
    }
    let Some(end) = tenant.contract_end else {
        return ReminderPlan::Skip("合同结束日期未填写".into());
    };
    let remaining_days =
        (end.to_micros_since_unix_epoch() - now.to_micros_since_unix_epoch()) / DAY_MICROS;
    let increase = next_increase_timestamp(tenant, now);
    let increase_days = increase.map(|value| {
        (value.to_micros_since_unix_epoch() - now.to_micros_since_unix_epoch()) / DAY_MICROS
    });
    if !manual && let Some(reason) = auto_window_skip(remaining_days, increase_days) {
        return ReminderPlan::Skip(reason.into());
    }
    let phone = tenant
        .phone_number
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    if phone.len() != 11 || !phone.starts_with('1') {
        return ReminderPlan::Skip("合同方手机号格式不正确".into());
    }
    if !has_config {
        return ReminderPlan::Skip("短信供应商尚未配置".into());
    }
    ReminderPlan::Send {
        phone,
        params: vec![
            tenant.tenant_name.clone(),
            increase.map(format_date).unwrap_or_else(|| "暂无".into()),
            format_date(end),
        ],
    }
}

/// 自动模式下的发送时机判断：到期窗口和递增窗口任一命中就该发。
///
/// 返回 `None` 表示到点了。两条规则并排在一个 `match` 里而不是拆成两个
/// `if`，是因为它们回答的是同一个问题——「现在到该提醒的时候了吗」。
fn auto_window_skip(remaining_days: i64, increase_days: Option<i64>) -> Option<&'static str> {
    match (remaining_days, increase_days) {
        (..=0, _) => Some("合同已经到期，不再自动发送提醒"),
        (days, increase)
            if days > REMINDER_DAYS
                && !increase.is_some_and(|days| (1..=INCREASE_REMINDER_DAYS).contains(&days)) =>
        {
            Some("合同尚未进入到期或递增提醒窗口")
        }
        _ => None,
    }
}

/// 执行段：唯一真正把短信发出去的地方。
fn send_candidate(
    ctx: &mut ProcedureContext,
    candidate: &ContractCandidate,
    manual: bool,
) -> Result<String, String> {
    match plan_reminder(
        &candidate.tenant,
        candidate.config.is_some(),
        ctx.timestamp,
        manual,
    ) {
        ReminderPlan::Skip(reason) => Err(reason),
        ReminderPlan::Send { phone, params } => {
            // 配置缺失时 `plan_reminder` 已经返回 Skip，这里取不到只可能是
            // 两边判断漂移，宁可报错也不要 unwrap。
            let config = candidate.config.as_ref().ok_or("短信供应商尚未配置")?;
            send_personal_message(ctx, config, &phone, REMINDER_TEMPLATE, &params)?;
            // 手机号长度由计划保证，切片安全。
            Ok(format!(
                "已向 {}****{} 发送合同提醒",
                &phone[..3],
                &phone[7..]
            ))
        }
    }
}

fn next_increase_timestamp(tenant: &RentalTenant, now: Timestamp) -> Option<Timestamp> {
    if let Some(date) = tenant.increase_date.filter(|date| *date > now) {
        return Some(date);
    }
    let start = tenant.contract_start?;
    let values =
        serde_json::from_str::<serde_json::Value>(tenant.increase_data.as_deref()?).ok()?;
    values
        .as_array()?
        .iter()
        .filter_map(|rule| rule.get("date")?.as_u64())
        .filter_map(|years| {
            let micros = start
                .to_micros_since_unix_epoch()
                .checked_add((years as i64).checked_mul(365 * DAY_MICROS)?)?;
            let date = Timestamp::from_micros_since_unix_epoch(micros);
            (date > now).then_some(date)
        })
        .min()
}

/// Howard Hinnant 的 civil-from-days 算法，用于无 chrono 环境下格式化日期。
fn format_date(value: Timestamp) -> String {
    let days = value.to_micros_since_unix_epoch().div_euclid(DAY_MICROS);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn batch_error(ids: &[u64], message: impl Into<String>) -> ContractSmsBatchResult {
    let message = message.into();
    ContractSmsBatchResult {
        results: ids
            .iter()
            .map(|id| ContractSmsSendItem {
                rental_tenant_id: *id,
                success: false,
                message: message.clone(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2025-07-01 00:00:00 UTC，全部用例的「此刻」。
    const NOW_MICROS: i64 = 1_751_328_000_000_000;

    fn now() -> Timestamp {
        Timestamp::from_micros_since_unix_epoch(NOW_MICROS)
    }

    /// 距今 `days` 天的时间点，负数表示过去。
    fn days_from_now(days: i64) -> Timestamp {
        Timestamp::from_micros_since_unix_epoch(NOW_MICROS + days * DAY_MICROS)
    }

    /// 一份「本该发提醒」的合同：收入合同、60 天后到期、手机号合法。
    /// 每个用例只改自己关心的那一个字段，其余保持可发状态。
    fn tenant() -> RentalTenant {
        RentalTenant {
            rental_tenant_id: 1,
            customer_id: "c1".into(),
            tenant_name: "张三".into(),
            phone_number: "13800138000".into(),
            transaction_type: true,
            status: None,
            contract_start: Some(days_from_now(-300)),
            contract_end: Some(days_from_now(60)),
            rental_amount_cents: None,
            increase_date: None,
            increase_rate_basis_points: None,
            increase_data: None,
            penalty_rate_basis_points: None,
            basic_ele_capacity_centi_kw: None,
            basic_ele_price_scaled: None,
            area_centi_square_metres: None,
            remark: None,
            park_id: 1,
            send_message_at: None,
            is_deleted: false,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    fn skip_reason(plan: ReminderPlan) -> String {
        match plan {
            ReminderPlan::Skip(reason) => reason,
            ReminderPlan::Send { phone, .. } => panic!("本不该发，却计划发给 {phone}"),
        }
    }

    fn send_params(plan: ReminderPlan) -> Vec<String> {
        match plan {
            ReminderPlan::Send { params, .. } => params,
            ReminderPlan::Skip(reason) => panic!("本该发，却被跳过：{reason}"),
        }
    }

    #[test]
    fn 到期提醒窗口以九十天为界() {
        let mut row = tenant();
        row.contract_end = Some(days_from_now(89));
        assert!(matches!(
            plan_reminder(&row, true, now(), false),
            ReminderPlan::Send { .. }
        ));

        row.contract_end = Some(days_from_now(91));
        assert_eq!(
            skip_reason(plan_reminder(&row, true, now(), false)),
            "合同尚未进入到期或递增提醒窗口"
        );
    }

    #[test]
    fn 已到期的合同自动不发但可以手动补发() {
        let mut row = tenant();
        row.contract_end = Some(days_from_now(-1));
        assert_eq!(
            skip_reason(plan_reminder(&row, true, now(), false)),
            "合同已经到期，不再自动发送提醒"
        );
        assert!(matches!(
            plan_reminder(&row, true, now(), true),
            ReminderPlan::Send { .. }
        ));
    }

    #[test]
    fn 递增日临近时即使离到期还远也要发() {
        let mut row = tenant();
        row.contract_end = Some(days_from_now(200));
        // 只看到期日的话早就被窗口挡掉了，是递增提醒把它拉了回来。
        row.increase_date = Some(days_from_now(20));
        assert!(matches!(
            plan_reminder(&row, true, now(), false),
            ReminderPlan::Send { .. }
        ));

        // 递增日超出 30 天窗口，两个理由都不成立。
        row.increase_date = Some(days_from_now(40));
        assert_eq!(
            skip_reason(plan_reminder(&row, true, now(), false)),
            "合同尚未进入到期或递增提醒窗口"
        );
    }

    #[test]
    fn 支出合同不自动打扰对方但允许手动发() {
        let mut row = tenant();
        row.transaction_type = false;
        assert_eq!(
            skip_reason(plan_reminder(&row, true, now(), false)),
            "支出合同不发送到期提醒"
        );
        assert!(matches!(
            plan_reminder(&row, true, now(), true),
            ReminderPlan::Send { .. }
        ));
    }

    #[test]
    fn 手机号不合规一律不发且手动也不能绕过() {
        let mut row = tenant();
        for bad in ["1380013800", "138001380001", "23800138000", ""] {
            row.phone_number = bad.into();
            assert_eq!(
                skip_reason(plan_reminder(&row, true, now(), true)),
                "合同方手机号格式不正确",
                "手机号 {bad:?} 应当拦下"
            );
        }
        // 带分隔符的号码按数字取出后仍然合法。
        row.phone_number = "138-0013-8000".into();
        assert!(matches!(
            plan_reminder(&row, true, now(), true),
            ReminderPlan::Send { .. }
        ));
    }

    #[test]
    fn 缺少结束日期或供应商配置时不发() {
        let mut row = tenant();
        row.contract_end = None;
        assert_eq!(
            skip_reason(plan_reminder(&row, true, now(), true)),
            "合同结束日期未填写"
        );

        assert_eq!(
            skip_reason(plan_reminder(&tenant(), false, now(), true)),
            "短信供应商尚未配置"
        );
    }

    #[test]
    fn 模板参数依次是名称递增日与到期日() {
        let mut row = tenant();
        row.contract_end = Some(days_from_now(60));
        row.increase_date = Some(days_from_now(20));
        assert_eq!(
            send_params(plan_reminder(&row, true, now(), false)),
            vec!["张三", "2025-07-21", "2025-08-30"]
        );

        // 没有递增安排时占位成「暂无」，而不是留空让客户收到半截短信。
        row.increase_date = None;
        assert_eq!(
            send_params(plan_reminder(&row, true, now(), false))[1],
            "暂无"
        );
    }

    #[test]
    fn 递增计划从合同起始日按年推算() {
        let mut row = tenant();
        row.contract_start = Some(days_from_now(-355));
        row.increase_data = Some(r#"[{"date":1},{"date":3}]"#.into());
        // 起始日 + 1 年落在 10 天后，取最近的一个未来递增日。
        assert_eq!(
            next_increase_timestamp(&row, now()),
            Some(days_from_now(10))
        );
    }

    #[test]
    fn 可以格式化_unix_日期() {
        assert_eq!(
            format_date(Timestamp::from_micros_since_unix_epoch(0)),
            "1970-01-01"
        );
        assert_eq!(
            format_date(Timestamp::from_micros_since_unix_epoch(
                1_751_328_000_000_000
            )),
            "2025-07-01"
        );
    }
}
