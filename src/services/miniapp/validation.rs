use serde::Serialize;

use super::{
    clock::parse_iso_date,
    types::{ApiFieldError, ConstraintKey, DemandDraft},
};

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_RAW_TEXT_CHARS: usize = 1_000;

pub const DONGGUAN_TOWNS: [&str; 33] = [
    "莞城",
    "东城",
    "南城",
    "万江",
    "石碣",
    "石龙",
    "茶山",
    "石排",
    "企石",
    "横沥",
    "桥头",
    "谢岗",
    "东坑",
    "常平",
    "寮步",
    "樟木头",
    "大朗",
    "黄江",
    "清溪",
    "塘厦",
    "凤岗",
    "大岭山",
    "长安",
    "虎门",
    "厚街",
    "沙田",
    "道滘",
    "洪梅",
    "麻涌",
    "望牛墩",
    "中堂",
    "高埗",
    "松山湖",
];

pub fn validate_phone(phone: &str) -> bool {
    let bytes = phone.as_bytes();
    bytes.len() == 11
        && bytes[0] == b'1'
        && (b'3'..=b'9').contains(&bytes[1])
        && bytes.iter().all(u8::is_ascii_digit)
}

pub fn mask_phone(phone: &str) -> String {
    if !validate_phone(phone) {
        return "***".into();
    }
    format!("{}****{}", &phone[..3], &phone[7..])
}

pub fn validate_request_size<T: Serialize>(value: &T) -> Result<(), ApiFieldError> {
    let size = serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(MAX_REQUEST_BYTES + 1);
    if size > MAX_REQUEST_BYTES {
        return Err(ApiFieldError {
            field: "request".into(),
            message: "请求体不能超过64KB".into(),
        });
    }
    Ok(())
}

pub fn sanitize_user_text(value: &str, max_chars: usize) -> Result<String, String> {
    let value = value.trim();
    if value.chars().count() > max_chars {
        return Err(format!("内容不能超过{max_chars}个字符"));
    }
    let lower = value.to_ascii_lowercase();
    if value.contains('<')
        || value.contains('>')
        || lower.contains("javascript:")
        || lower.contains("data:text/html")
    {
        return Err("内容不能包含HTML或脚本标记".into());
    }
    Ok(value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect())
}

pub fn validate_demand(demand: &DemandDraft, require_complete: bool) -> Vec<ApiFieldError> {
    let mut errors = Vec::new();
    if demand.raw_text.trim().is_empty() {
        errors.push(field_error("raw_text", "请描述找房需求"));
    } else if let Err(message) = sanitize_user_text(&demand.raw_text, MAX_RAW_TEXT_CHARS) {
        errors.push(field_error("raw_text", message));
    }

    let constraints = &demand.constraints;
    if require_complete && constraints.space_type.is_none() {
        errors.push(field_error("constraints.space_type", "请选择空间类型"));
    }
    if require_complete && constraints.target_towns.is_empty() {
        errors.push(field_error(
            "constraints.target_towns",
            "请至少选择一个目标镇街",
        ));
    }
    if constraints.target_towns.len() > 8 {
        errors.push(field_error(
            "constraints.target_towns",
            "目标镇街最多选择8个",
        ));
    }
    for town in &constraints.target_towns {
        if !DONGGUAN_TOWNS.contains(&town.as_str()) {
            errors.push(field_error(
                "constraints.target_towns",
                format!("不支持的东莞镇街：{town}"),
            ));
        }
    }

    match (constraints.area_min_sqm, constraints.area_max_sqm) {
        (Some(min), Some(max)) if min > 0 && min <= max && max <= 1_000_000 => {}
        (None, None) if !require_complete => {}
        _ => errors.push(field_error(
            "constraints.area_min_sqm",
            "面积上下限必须完整，且满足0 < 下限 ≤ 上限 ≤ 1000000平方米",
        )),
    }

    if constraints
        .rent_min_cents
        .is_some_and(|value| value > 10_000_000_000)
        || constraints
            .rent_max_cents
            .is_some_and(|value| value > 10_000_000_000)
    {
        errors.push(field_error(
            "constraints.rent_max_cents",
            "租金预算不能超过100000000元",
        ));
    }
    if let (Some(min), Some(max)) = (constraints.rent_min_cents, constraints.rent_max_cents) {
        if min > max {
            errors.push(field_error(
                "constraints.rent_max_cents",
                "租金预算下限不能高于上限",
            ));
        }
    }
    if constraints.rent_min_cents.is_some() || constraints.rent_max_cents.is_some() {
        if constraints.rent_unit.is_none() {
            errors.push(field_error(
                "constraints.rent_unit",
                "填写租金预算时必须选择计价单位",
            ));
        }
    }
    if constraints
        .elevator_min_tons
        .is_some_and(|value| !(0.1..=100.0).contains(&value))
    {
        errors.push(field_error(
            "constraints.elevator_min_tons",
            "电梯吨位必须在0.1到100吨之间",
        ));
    }
    if constraints
        .power_capacity_kva
        .is_some_and(|value| value == 0 || value > 100_000)
    {
        errors.push(field_error(
            "constraints.power_capacity_kva",
            "用电容量必须在1到100000kVA之间",
        ));
    }
    for (field, value, max_chars) in [
        (
            "constraints.move_in_time",
            constraints.move_in_time.as_deref(),
            64,
        ),
        (
            "constraints.floor_preference",
            constraints.floor_preference.as_deref(),
            80,
        ),
        (
            "constraints.fire_requirement",
            constraints.fire_requirement.as_deref(),
            120,
        ),
        (
            "constraints.logistics_requirement",
            constraints.logistics_requirement.as_deref(),
            200,
        ),
        (
            "constraints.loading_requirement",
            constraints.loading_requirement.as_deref(),
            200,
        ),
        (
            "constraints.other_notes",
            constraints.other_notes.as_deref(),
            500,
        ),
    ] {
        if let Some(value) = value {
            if let Err(message) = sanitize_user_text(value, max_chars) {
                errors.push(field_error(field, message));
            }
        }
    }
    if let Some(value) = constraints.move_in_time.as_deref() {
        if !matches!(value, "immediate" | "within_30_days" | "within_90_days")
            && parse_iso_date(value).is_err()
        {
            errors.push(field_error(
                "constraints.move_in_time",
                "入驻时间必须为immediate、within_30_days、within_90_days或有效YYYY-MM-DD日期",
            ));
        }
    }
    if !demand.ai_confidence.is_finite() || !(0.0..=1.0).contains(&demand.ai_confidence) {
        errors.push(field_error("ai_confidence", "AI置信度必须在0到1之间"));
    }
    if demand.missing_fields.len() > 20 {
        errors.push(field_error("missing_fields", "缺失字段标识最多20项"));
    }
    for value in &demand.missing_fields {
        if let Err(message) = sanitize_user_text(value, 80) {
            errors.push(field_error("missing_fields", message));
        }
    }
    if demand.hard_conditions.len() > 20 || demand.preference_conditions.len() > 20 {
        errors.push(field_error("conditions", "硬条件和偏好条件分别最多20项"));
    }
    for (field, values) in [
        ("hard_conditions", &demand.hard_conditions),
        ("preference_conditions", &demand.preference_conditions),
    ] {
        for value in values {
            if let Err(message) = sanitize_user_text(value, 120) {
                errors.push(field_error(field, message));
            }
        }
    }
    if demand.constraint_priorities.len() > 10 {
        errors.push(field_error(
            "constraint_priorities",
            "结构化条件优先级最多10项",
        ));
    }
    let mut priority_keys = std::collections::BTreeSet::new();
    for priority in &demand.constraint_priorities {
        if !priority_keys.insert(priority.key.clone()) {
            errors.push(field_error(
                "constraint_priorities",
                format!("结构化条件重复：{:?}", priority.key),
            ));
        }
        if !constraint_has_value(demand, &priority.key) {
            errors.push(field_error(
                "constraint_priorities",
                format!("结构化条件没有对应值：{:?}", priority.key),
            ));
        }
    }
    errors
}

pub fn constraint_has_value(demand: &DemandDraft, key: &ConstraintKey) -> bool {
    let constraints = &demand.constraints;
    match key {
        ConstraintKey::Budget => {
            constraints.rent_max_cents.is_some() && constraints.rent_unit.is_some()
        }
        ConstraintKey::FreightElevator => constraints.needs_freight_elevator == Some(true),
        ConstraintKey::ElevatorCapacity => constraints.elevator_min_tons.is_some(),
        ConstraintKey::PowerCapacity => constraints.power_capacity_kva.is_some(),
        ConstraintKey::FireSafety => constraints.fire_requirement.is_some(),
        ConstraintKey::TruckAccess => constraints.logistics_requirement.is_some(),
        ConstraintKey::LoadingDock => constraints.loading_requirement.is_some(),
        ConstraintKey::Sublease => constraints.accepts_sublease.is_some(),
        ConstraintKey::Floor => constraints.floor_preference.is_some(),
        ConstraintKey::MoveIn => constraints.move_in_time.is_some(),
    }
}

pub fn normalize_constraint_priorities(demand: &mut DemandDraft) {
    demand
        .constraint_priorities
        .sort_by(|left, right| left.key.cmp(&right.key));
}

pub fn validate_idempotency_key(value: &str) -> Result<(), ApiFieldError> {
    if !(16..=128).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(field_error(
            "submission.idempotency_key",
            "幂等键必须为16到128位字母、数字、短横线或下划线",
        ));
    }
    Ok(())
}

pub fn redact_for_log(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut digits = String::new();
    let mut iterator = value.split_whitespace().peekable();
    while let Some(part) = iterator.next() {
        if part.eq_ignore_ascii_case("bearer") {
            if !output.is_empty() {
                output.push(' ');
            }
            output.push_str("Bearer [redacted]");
            let _ = iterator.next();
            continue;
        }
        if !output.is_empty() {
            output.push(' ');
        }
        for character in part.chars().chain(std::iter::once(' ')) {
            if character.is_ascii_digit() {
                digits.push(character);
            } else {
                if digits.len() >= 7 {
                    output.push_str("[redacted-number]");
                } else {
                    output.push_str(&digits);
                }
                digits.clear();
                if character != ' ' {
                    output.push(character);
                }
            }
        }
    }
    output
}

fn field_error(field: impl Into<String>, message: impl Into<String>) -> ApiFieldError {
    ApiFieldError {
        field: field.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::miniapp::types::{ConstraintKey, ConstraintLevel, ConstraintPriority};

    #[test]
    fn 手机号格式严格校验() {
        let synthetic_phone = ["139", "0000", "0000"].concat();
        assert!(validate_phone(&synthetic_phone));
        assert!(!validate_phone("12912345678"));
        assert!(!validate_phone("1391234567x"));
    }

    #[test]
    fn 恶意html被拒绝() {
        assert!(sanitize_user_text("<script>alert(1)</script>", 100).is_err());
        let mut demand = DemandDraft {
            raw_text: "需要厂房".into(),
            ..Default::default()
        };
        demand.constraints.other_notes = Some("<img src=x>".into());
        assert!(validate_demand(&demand, false)
            .iter()
            .any(|error| error.field == "constraints.other_notes"));
    }

    #[test]
    fn 非法入驻日期被拒绝() {
        let mut demand = DemandDraft {
            raw_text: "需要厂房".into(),
            ..Default::default()
        };
        demand.constraints.move_in_time = Some("2025-02-29".into());
        assert!(validate_demand(&demand, false)
            .iter()
            .any(|error| error.field == "constraints.move_in_time"));
    }

    #[test]
    fn 重复结构化优先级被拒绝() {
        let mut demand = DemandDraft {
            raw_text: "需要带货梯厂房".into(),
            ..Default::default()
        };
        demand.constraints.needs_freight_elevator = Some(true);
        demand.constraint_priorities = vec![
            ConstraintPriority {
                key: ConstraintKey::FreightElevator,
                level: ConstraintLevel::Preference,
            },
            ConstraintPriority {
                key: ConstraintKey::FreightElevator,
                level: ConstraintLevel::Hard,
            },
        ];
        assert!(validate_demand(&demand, false)
            .iter()
            .any(|error| error.field == "constraint_priorities"));
    }

    #[test]
    fn 合法结构化优先级按key规范化() {
        let mut demand = DemandDraft::default();
        demand.constraint_priorities = vec![
            ConstraintPriority {
                key: ConstraintKey::PowerCapacity,
                level: ConstraintLevel::Preference,
            },
            ConstraintPriority {
                key: ConstraintKey::Budget,
                level: ConstraintLevel::Hard,
            },
        ];
        normalize_constraint_priorities(&mut demand);
        assert_eq!(demand.constraint_priorities[0].key, ConstraintKey::Budget);
        assert_eq!(
            demand.constraint_priorities[1].key,
            ConstraintKey::PowerCapacity
        );
    }

    #[test]
    fn 非法优先级字符串无法反序列化() {
        let invalid = serde_json::json!({"key":"freight_elevator","level":"maybe"});
        assert!(serde_json::from_value::<ConstraintPriority>(invalid).is_err());
        let unspecified = serde_json::json!({"key":"freight_elevator","level":"unspecified"});
        assert_eq!(
            serde_json::from_value::<ConstraintPriority>(unspecified)
                .expect("valid three-state level")
                .level,
            ConstraintLevel::Unspecified
        );
    }

    #[test]
    fn 日志会脱敏手机号和令牌() {
        let synthetic_phone = ["139", "0000", "0000"].concat();
        let synthetic_credential = ["secret", "marker"].join("-");
        let input = format!("phone={synthetic_phone} Authorization Bearer {synthetic_credential}");
        let output = redact_for_log(&input);
        assert!(!output.contains(&synthetic_phone));
        assert!(!output.contains(&synthetic_credential));
    }
}
