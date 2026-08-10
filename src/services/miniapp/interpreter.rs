use std::future::Future;

use super::{
    types::{
        ConstraintKey, ConstraintLevel, ConstraintPriority, DemandDraft, DemandInterpretation,
        RentUnit, SpaceType,
    },
    validation::{sanitize_user_text, validate_demand, DONGGUAN_TOWNS, MAX_RAW_TEXT_CHARS},
};

pub trait DemandInterpreter {
    fn interpret(
        &self,
        draft: DemandDraft,
    ) -> impl Future<Output = Result<DemandInterpretation, String>> + Send;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LocalDemandInterpreter;

impl DemandInterpreter for LocalDemandInterpreter {
    async fn interpret(&self, draft: DemandDraft) -> Result<DemandInterpretation, String> {
        let demand = interpret_local(draft)?;
        Ok(DemandInterpretation {
            demand,
            provider: "local".into(),
            fallback_reason: None,
        })
    }
}

pub fn interpret_local(mut draft: DemandDraft) -> Result<DemandDraft, String> {
    draft.raw_text = sanitize_user_text(&draft.raw_text, MAX_RAW_TEXT_CHARS)?;
    let text = normalize(&draft.raw_text);
    if text.is_empty() {
        return Err("请描述找房需求".into());
    }

    if draft.constraints.space_type.is_none() {
        draft.constraints.space_type = if text.contains("仓库") || text.contains("仓储") {
            Some(SpaceType::Warehouse)
        } else if text.contains("写字楼") || text.contains("办公室") || text.contains("办公")
        {
            Some(SpaceType::Office)
        } else if text.contains("厂房") || text.contains("厂区") || text.contains("车间") {
            Some(SpaceType::Factory)
        } else {
            None
        };
    }

    for town in DONGGUAN_TOWNS {
        if text.contains(town)
            && !draft
                .constraints
                .target_towns
                .iter()
                .any(|value| value == town)
        {
            draft.constraints.target_towns.push(town.into());
        }
    }

    if draft.constraints.area_min_sqm.is_none() || draft.constraints.area_max_sqm.is_none() {
        if let Some((min, max)) = parse_area(&text) {
            draft.constraints.area_min_sqm.get_or_insert(min);
            draft.constraints.area_max_sqm.get_or_insert(max);
        }
    }

    if draft.constraints.rent_max_cents.is_none() {
        if let Some((min, max, unit)) = parse_rent(&text) {
            draft.constraints.rent_min_cents = Some(min);
            draft.constraints.rent_max_cents = Some(max);
            draft.constraints.rent_unit = Some(unit);
            classify_constraint(&text, "预算", ConstraintKey::Budget, &mut draft);
        }
    }

    if text.contains("货梯") {
        draft.constraints.needs_freight_elevator = Some(true);
        if draft.constraints.elevator_min_tons.is_none() {
            draft.constraints.elevator_min_tons =
                number_before_keyword(&text, &["吨货梯", "吨电梯"]).map(|number| number as f32);
        }
        classify_constraint(&text, "货梯", ConstraintKey::FreightElevator, &mut draft);
        if draft.constraints.elevator_min_tons.is_some() {
            classify_constraint(&text, "电梯", ConstraintKey::ElevatorCapacity, &mut draft);
        }
    }

    if draft.constraints.power_capacity_kva.is_none() {
        if let Some(value) = number_before_keyword(&text, &["kva", "千伏安", "kw", "千瓦"]) {
            draft.constraints.power_capacity_kva = Some(value.round() as u32);
            classify_constraint(&text, "用电", ConstraintKey::PowerCapacity, &mut draft);
        } else if text.contains("变压器") || text.contains("较大用电") || text.contains("大用电")
        {
            draft
                .constraints
                .other_notes
                .get_or_insert_with(|| "需要确认变压器或用电容量".into());
        }
    }

    if draft.constraints.fire_requirement.is_none() {
        for rating in ["甲类", "乙类", "丙类", "丁类", "戊类"] {
            if text.contains(rating) && text.contains("消防") {
                draft.constraints.fire_requirement = Some(format!("{rating}消防"));
                classify_constraint(&text, "消防", ConstraintKey::FireSafety, &mut draft);
                break;
            }
        }
    }

    if draft.constraints.move_in_time.is_none() {
        draft.constraints.move_in_time = parse_move_in_time(&text);
        if draft.constraints.move_in_time.is_some() {
            classify_constraint(&text, "入驻", ConstraintKey::MoveIn, &mut draft);
        }
    }
    if draft.constraints.floor_preference.is_none() {
        draft.constraints.floor_preference = ["一楼", "首层", "高楼层", "低楼层"]
            .into_iter()
            .find(|value| text.contains(value))
            .map(str::to_string);
        if draft.constraints.floor_preference.is_some() {
            classify_constraint(&text, "楼层", ConstraintKey::Floor, &mut draft);
        }
    }
    if draft.constraints.logistics_requirement.is_none()
        && (text.contains("大车") || text.contains("货车") || text.contains("物流"))
    {
        draft.constraints.logistics_requirement = Some("支持货车通行".into());
        classify_constraint(&text, "货车", ConstraintKey::TruckAccess, &mut draft);
    }
    if draft.constraints.loading_requirement.is_none()
        && (text.contains("装卸") || text.contains("卸货") || text.contains("月台"))
    {
        draft.constraints.loading_requirement = Some("需要装卸区或月台".into());
        classify_constraint(&text, "装卸", ConstraintKey::LoadingDock, &mut draft);
    }
    if draft.constraints.accepts_sublease.is_none() {
        if text.contains("不接受分租") || text.contains("必须整租") {
            draft.constraints.accepts_sublease = Some(false);
            classify_constraint(&text, "分租", ConstraintKey::Sublease, &mut draft);
        } else if text.contains("接受分租") || text.contains("可以分租") {
            draft.constraints.accepts_sublease = Some(true);
            classify_constraint(&text, "分租", ConstraintKey::Sublease, &mut draft);
        }
    }

    draft.missing_fields.clear();
    if draft.constraints.space_type.is_none() {
        draft.missing_fields.push("space_type".into());
    }
    if draft.constraints.target_towns.is_empty() {
        draft.missing_fields.push("target_towns".into());
    }
    if draft.constraints.area_min_sqm.is_none() || draft.constraints.area_max_sqm.is_none() {
        draft.missing_fields.push("area_range".into());
    }

    let filled = [
        draft.constraints.space_type.is_some(),
        !draft.constraints.target_towns.is_empty(),
        draft.constraints.area_min_sqm.is_some(),
        draft.constraints.rent_max_cents.is_some(),
        draft.constraints.move_in_time.is_some(),
        draft.constraints.floor_preference.is_some(),
        draft.constraints.needs_freight_elevator.is_some(),
        draft.constraints.power_capacity_kva.is_some(),
        draft.constraints.fire_requirement.is_some(),
        draft.constraints.logistics_requirement.is_some(),
    ]
    .into_iter()
    .filter(|value| *value)
    .count();
    draft.ai_confidence = (0.4 + filled as f32 * 0.055).min(0.95);
    dedupe(&mut draft.hard_conditions);
    dedupe(&mut draft.preference_conditions);
    draft
        .constraint_priorities
        .sort_by(|left, right| left.key.cmp(&right.key));
    draft
        .constraint_priorities
        .dedup_by(|left, right| left.key == right.key);

    let errors = validate_demand(&draft, false);
    if let Some(error) = errors.first() {
        return Err(error.message.clone());
    }
    Ok(draft)
}

fn normalize(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace(',', "")
        .replace('，', "")
        .replace('～', "-")
        .replace('—', "-")
        .replace('至', "-")
        .replace("平方米", "㎡")
        .replace("平米", "㎡")
}

fn parse_area(text: &str) -> Option<(u32, u32)> {
    let unit = text.find('㎡').or_else(|| text.find('平'))?;
    let prefix = tail_chars(&text[..unit], 28);
    let numbers = numbers_in(prefix);
    let last = *numbers.last()?;
    let (min, max) = if numbers.len() >= 2 && prefix.contains('-') {
        (numbers[numbers.len() - 2], last)
    } else if text.contains("以上") {
        (last, (last * 1.5).max(last + 1.0))
    } else if text.contains("以下") {
        ((last * 0.5).max(1.0), last)
    } else {
        (last * 0.9, last * 1.1)
    };
    Some((min.round().max(1.0) as u32, max.round().max(min) as u32))
}

fn parse_rent(text: &str) -> Option<(u64, u64, RentUnit)> {
    let per_sqm = text.contains("元/㎡/月")
        || text.contains("元每㎡每月")
        || text.contains("元/平/月")
        || text.contains("元每平每月");
    let budget_pos = text.find("预算").or_else(|| text.find("月租"))?;
    let segment = head_chars(&text[budget_pos..], 36);
    let numbers = number_tokens_in(segment);
    if numbers.is_empty() {
        return None;
    }
    let multiplier = if segment.contains('万') { 10_000 } else { 1 };
    let last = yuan_token_to_cents(&numbers[numbers.len() - 1], multiplier)?;
    let first = if numbers.len() >= 2 && segment.contains('-') {
        yuan_token_to_cents(&numbers[numbers.len() - 2], multiplier)?
    } else {
        0
    };
    Some((
        first,
        last,
        if per_sqm {
            RentUnit::YuanPerSquareMetreMonth
        } else {
            RentUnit::YuanPerMonth
        },
    ))
}

fn number_tokens_in(value: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut current = String::new();
    for character in value.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() || (character == '.' && !current.contains('.')) {
            current.push(character);
        } else if !current.is_empty() {
            output.push(std::mem::take(&mut current));
        }
    }
    output
}

fn yuan_token_to_cents(value: &str, multiplier: u64) -> Option<u64> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty() || fraction.len() > 2 || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let whole = whole.parse::<u64>().ok()?;
    let fraction = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().ok()?.checked_mul(10)?,
        2 => fraction.parse::<u64>().ok()?,
        _ => return None,
    };
    whole
        .checked_mul(100)?
        .checked_add(fraction)?
        .checked_mul(multiplier)
}

fn number_before_keyword(text: &str, keywords: &[&str]) -> Option<f64> {
    for keyword in keywords {
        if let Some(position) = text.find(keyword) {
            if let Some(number) = numbers_in(tail_chars(&text[..position], 18)).last() {
                return Some(*number);
            }
        }
    }
    None
}

fn numbers_in(value: &str) -> Vec<f64> {
    let mut output = Vec::new();
    let mut current = String::new();
    for character in value.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() || (character == '.' && !current.contains('.')) {
            current.push(character);
        } else if !current.is_empty() {
            if let Ok(number) = current.parse::<f64>() {
                output.push(number);
            }
            current.clear();
        }
    }
    output
}

fn tail_chars(value: &str, count: usize) -> &str {
    value
        .char_indices()
        .rev()
        .nth(count)
        .map_or(value, |(index, _)| &value[index..])
}

fn head_chars(value: &str, count: usize) -> &str {
    value
        .char_indices()
        .nth(count)
        .map_or(value, |(index, _)| &value[..index])
}

fn parse_move_in_time(text: &str) -> Option<String> {
    for start in 0..text.len().saturating_sub(9) {
        if !text.is_char_boundary(start) || !text.is_char_boundary(start + 10) {
            continue;
        }
        let candidate = &text[start..start + 10];
        let bytes = candidate.as_bytes();
        if bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes
                .iter()
                .enumerate()
                .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
        {
            return Some(candidate.into());
        }
    }
    if text.contains("立即") || text.contains("随时") {
        Some("immediate".into())
    } else if text.contains("一个月内") || text.contains("1个月内") {
        Some("within_30_days".into())
    } else if text.contains("三个月内") || text.contains("3个月内") {
        Some("within_90_days".into())
    } else {
        None
    }
}

fn classify_constraint(text: &str, keyword: &str, key: ConstraintKey, draft: &mut DemandDraft) {
    let preference = text.contains(&format!("最好{keyword}"))
        || text.contains(&format!("优先{keyword}"))
        || text.contains(&format!("希望{keyword}"));
    let level = if preference {
        ConstraintLevel::Preference
    } else if text.contains("必须") || text.contains("需要") || text.contains("要求") {
        ConstraintLevel::Hard
    } else {
        ConstraintLevel::Preference
    };
    if let Some(existing) = draft
        .constraint_priorities
        .iter_mut()
        .find(|priority| priority.key == key)
    {
        existing.level = level;
    } else {
        draft
            .constraint_priorities
            .push(ConstraintPriority { key, level });
    }
}

fn dedupe(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

#[cfg(feature = "server")]
pub mod bailian {
    use std::time::Duration;

    use reqwest::StatusCode;
    use serde_json::Value;

    use super::*;

    #[derive(Clone, Debug)]
    pub struct BailianConfig {
        pub key: String,
        pub model: String,
        pub timeout_seconds: u64,
        pub retry_attempts: usize,
    }

    impl BailianConfig {
        pub fn from_values(key: Option<String>, model: Option<String>) -> Result<Self, String> {
            let key = key
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "服务端未配置 ALIYUN_BAILIAN_KEY".to_string())?;
            Ok(Self {
                key,
                model: model
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "qwen3.5-plus".into()),
                timeout_seconds: 30,
                retry_attempts: 3,
            })
        }

        pub fn from_env() -> Result<Self, String> {
            dotenvy::dotenv().ok();
            Self::from_values(
                std::env::var("ALIYUN_BAILIAN_KEY").ok(),
                std::env::var("YIZU_MINIAPP_AI_MODEL").ok(),
            )
        }
    }

    pub struct BailianDemandInterpreter {
        config: BailianConfig,
        client: reqwest::Client,
    }

    impl BailianDemandInterpreter {
        pub fn new(config: BailianConfig) -> Result<Self, String> {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(config.timeout_seconds))
                .build()
                .map_err(|_| "无法创建百炼客户端".to_string())?;
            Ok(Self { config, client })
        }
    }

    impl DemandInterpreter for BailianDemandInterpreter {
        async fn interpret(&self, mut draft: DemandDraft) -> Result<DemandInterpretation, String> {
            draft.raw_text =
                strip_contact_numbers(&sanitize_user_text(&draft.raw_text, MAX_RAW_TEXT_CHARS)?);
            let payload = serde_json::json!({
                "model": self.config.model,
                "temperature": 0,
                "max_tokens": 1200,
                "response_format": { "type": "json_object" },
                "messages": [
                    { "role": "system", "content": "你是东莞工业空间需求解析器。只返回一个严格 JSON 对象，字段必须与给定 demand schema 一致；支持字段使用constraint_priorities的类型化key和hard/preference级别；无法确认的值使用null或空数组，禁止猜测；hard_conditions只保留无法映射的其他硬条件。" },
                    { "role": "user", "content": serde_json::json!({
                        "raw_text": draft.raw_text,
                        "known_draft": draft,
                        "allowed_space_types": ["factory", "warehouse", "office"],
                        "allowed_towns": &DONGGUAN_TOWNS[..],
                        "required_output": "DemandDraft JSON"
                    }).to_string() }
                ]
            });

            let mut last_error = "百炼请求失败".to_string();
            for attempt in 1..=self.config.retry_attempts {
                let response = self
                    .client
                    .post("https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions")
                    .bearer_auth(&self.config.key)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(payload.to_string())
                    .send()
                    .await;
                match response {
                    Ok(response) => {
                        let status = response.status();
                        let body = response
                            .text()
                            .await
                            .map_err(|_| "读取百炼响应失败".to_string())?;
                        if status.is_success() {
                            let demand = parse_bailian_response(&body)?;
                            return Ok(DemandInterpretation {
                                demand,
                                provider: "bailian".into(),
                                fallback_reason: None,
                            });
                        }
                        last_error = format!("百炼返回HTTP {}", status.as_u16());
                        if !is_retryable_status(status) || attempt == self.config.retry_attempts {
                            return Err(last_error);
                        }
                    }
                    Err(error) => {
                        last_error = if error.is_timeout() {
                            "百炼请求超时"
                        } else {
                            "无法连接百炼"
                        }
                        .into();
                        if !(error.is_timeout() || error.is_connect())
                            || attempt == self.config.retry_attempts
                        {
                            return Err(last_error);
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(150 * attempt as u64)).await;
            }
            Err(last_error)
        }
    }

    pub fn parse_bailian_response(body: &str) -> Result<DemandDraft, String> {
        let outer: Value =
            serde_json::from_str(body).map_err(|_| "百炼返回了非法JSON".to_string())?;
        let content = outer
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| "百炼响应缺少结构化内容".to_string())?;
        let demand: DemandDraft = serde_json::from_str(content.trim())
            .map_err(|_| "百炼结构化结果不符合DemandDraft schema".to_string())?;
        let errors = validate_demand(&demand, false);
        if !errors.is_empty() {
            return Err("百炼结构化结果未通过领域校验".into());
        }
        Ok(demand)
    }

    fn strip_contact_numbers(value: &str) -> String {
        let mut output = String::new();
        let mut digits = String::new();
        for character in value.chars().chain(std::iter::once(' ')) {
            if character.is_ascii_digit() {
                digits.push(character);
            } else {
                if digits.len() < 7 {
                    output.push_str(&digits);
                } else {
                    output.push_str("[联系方式已移除]");
                }
                digits.clear();
                output.push(character);
            }
        }
        output.trim().into()
    }

    fn is_retryable_status(status: StatusCode) -> bool {
        status == StatusCode::REQUEST_TIMEOUT
            || status == StatusCode::TOO_MANY_REQUESTS
            || status.is_server_error()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn 非法ai输出被拒绝() {
            assert!(parse_bailian_response("not-json").is_err());
            assert!(
                parse_bailian_response(r#"{"choices":[{"message":{"content":"{}"}}]}"#).is_err()
            );
        }

        #[test]
        fn 百炼缺少key时明确失败() {
            assert!(BailianConfig::from_values(None, None).is_err());
        }

        #[test]
        fn 发送模型前移除联系方式() {
            let synthetic_phone = ["139", "0000", "0000"].concat();
            let scrubbed = strip_contact_numbers(&format!("找厂房，联系{synthetic_phone}"));
            assert!(!scrubbed.contains(&synthetic_phone));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(text: &str) -> DemandDraft {
        DemandDraft {
            raw_text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn 解析中文厂房面积镇街货梯和用电() {
        let result = interpret_local(draft(
            "想在松山湖找1500平方米左右的厂房，需要3吨货梯和500kVA用电，丙类消防",
        ))
        .expect("local parser");
        assert_eq!(result.constraints.space_type, Some(SpaceType::Factory));
        assert_eq!(result.constraints.target_towns, vec!["松山湖"]);
        assert_eq!(result.constraints.area_min_sqm, Some(1350));
        assert_eq!(result.constraints.area_max_sqm, Some(1650));
        assert_eq!(result.constraints.elevator_min_tons, Some(3.0));
        assert_eq!(result.constraints.power_capacity_kva, Some(500));
        assert!(result.constraint_priorities.iter().any(|priority| {
            priority.key == ConstraintKey::FreightElevator
                && priority.level == ConstraintLevel::Hard
        }));
    }

    #[test]
    fn 解析面积范围和每平方米预算() {
        let result =
            interpret_local(draft("寮步1000-2000平仓库，预算30元/平/月")).expect("local parser");
        assert_eq!(result.constraints.area_min_sqm, Some(1000));
        assert_eq!(result.constraints.area_max_sqm, Some(2000));
        assert_eq!(result.constraints.rent_max_cents, Some(3000));
        assert_eq!(
            result.constraints.rent_unit,
            Some(RentUnit::YuanPerSquareMetreMonth)
        );
    }

    #[test]
    fn 小数元预算安全转换为整数分() {
        let result =
            interpret_local(draft("寮步1000平仓库，预算28.5元/平/月")).expect("local parser");
        assert_eq!(result.constraints.rent_max_cents, Some(2850));
    }

    #[test]
    fn 不猜测缺失的必要字段() {
        let result = interpret_local(draft("希望物流方便，最好有货梯")).expect("local parser");
        assert_eq!(
            result.missing_fields,
            vec!["space_type", "target_towns", "area_range"]
        );
        assert!(result.constraint_priorities.iter().any(|priority| {
            priority.key == ConstraintKey::FreightElevator
                && priority.level == ConstraintLevel::Preference
        }));
    }
}
