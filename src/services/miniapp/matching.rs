use std::cmp::Ordering;

use time::Date;

use super::{
    clock::{move_in_deadline, parse_iso_date, Clock, SystemClock},
    repository::ListingRepository,
    types::{
        ConstraintAssessment, ConstraintKey, ConstraintLevel, DemandDraft, ListingSummary,
        MatchDimensionScore, MatchResponse, MatchResult, RentUnit,
    },
    validation::constraint_has_value,
};

const WEIGHTS: [(&str, f32); 7] = [
    ("location", 0.20),
    ("space", 0.20),
    ("cost", 0.20),
    ("production", 0.15),
    ("logistics", 0.10),
    ("compliance", 0.10),
    ("move_in", 0.05),
];

pub fn match_listings(demand: &DemandDraft, repository: &impl ListingRepository) -> MatchResponse {
    match_listings_with_clock(demand, repository, &SystemClock)
}

pub fn match_listings_with_clock(
    demand: &DemandDraft,
    repository: &impl ListingRepository,
    clock: &dyn Clock,
) -> MatchResponse {
    let eligible = repository
        .list()
        .into_iter()
        .filter(is_ai_eligible)
        .filter(|listing| demand.constraints.space_type.as_ref() == Some(&listing.space_type))
        .filter(|listing| demand.constraints.target_towns.contains(&listing.town))
        .collect::<Vec<_>>();

    let strict = eligible
        .iter()
        .filter(|listing| area_matches(demand, listing.available_area_sqm, false))
        .cloned()
        .collect::<Vec<_>>();
    let (candidates, relaxed) = if strict.is_empty() {
        (
            eligible
                .into_iter()
                .filter(|listing| area_matches(demand, listing.available_area_sqm, true))
                .collect::<Vec<_>>(),
            true,
        )
    } else {
        (strict, false)
    };

    let reference_date = clock.today_china();
    let mut matches = candidates
        .into_iter()
        .map(|listing| score_listing(demand, listing, relaxed, reference_date))
        .collect::<Vec<_>>();
    matches.sort_by(compare_matches);
    matches.truncate(10);

    MatchResponse {
        next_step_suggestion: matches
            .is_empty()
            .then(|| "当前条件下没有可推荐房源，是否接受相邻镇街？确认前不会自动扩展。".into()),
        matches,
        used_area_relaxation: relaxed,
        demo_data: true,
    }
}

fn is_ai_eligible(listing: &ListingSummary) -> bool {
    listing.rental_status == "available"
        && (listing.is_self_operated || listing.verification_level.rank() >= 2)
}

fn area_matches(demand: &DemandDraft, area: u32, relaxed: bool) -> bool {
    let (Some(min), Some(max)) = (
        demand.constraints.area_min_sqm,
        demand.constraints.area_max_sqm,
    ) else {
        return false;
    };
    let (min, max) = if relaxed {
        (
            min.saturating_mul(80) / 100,
            max.saturating_mul(120).div_ceil(100),
        )
    } else {
        (min, max)
    };
    (min..=max).contains(&area)
}

fn score_listing(
    demand: &DemandDraft,
    listing: ListingSummary,
    area_relaxed: bool,
    reference_date: Date,
) -> MatchResult {
    let mut dimensions = Vec::with_capacity(7);
    let mut unmet = Vec::new();

    dimensions.push(dimension("location", 100.0, "位于目标镇街"));
    dimensions.push(dimension(
        "space",
        area_score(demand, listing.available_area_sqm, area_relaxed),
        if area_relaxed {
            "类型匹配，面积使用±20%放宽"
        } else {
            "空间类型和面积范围匹配"
        },
    ));

    let (cost_score, cost_reason, cost_unmet) = score_cost(demand, &listing);
    dimensions.push(dimension("cost", cost_score, cost_reason));
    if let Some(reason) = cost_unmet {
        unmet.push(reason);
    }

    let (production_score, production_reason, production_unmet) =
        score_production(demand, &listing);
    dimensions.push(dimension("production", production_score, production_reason));
    unmet.extend(production_unmet);

    let (logistics_score, logistics_reason, logistics_unmet) = score_logistics(demand, &listing);
    dimensions.push(dimension("logistics", logistics_score, logistics_reason));
    unmet.extend(logistics_unmet);

    let (compliance_score, compliance_reason, compliance_unmet) =
        score_compliance(demand, &listing);
    dimensions.push(dimension("compliance", compliance_score, compliance_reason));
    unmet.extend(compliance_unmet);

    let (move_in_score, move_in_reason, move_in_unmet) =
        score_move_in(demand, &listing, reference_date);
    dimensions.push(dimension("move_in", move_in_score, move_in_reason));
    if let Some(reason) = move_in_unmet {
        unmet.push(reason);
    }

    let assessments = assess_constraints(demand, &listing, reference_date);
    unmet.extend(
        assessments
            .unmet_hard
            .iter()
            .chain(&assessments.unverified_hard)
            .chain(&assessments.unmet_preferences)
            .map(|item| item.detail.clone()),
    );
    unmet.sort();
    unmet.dedup();

    let overall_score = dimensions
        .iter()
        .map(|item| item.score * item.weight)
        .sum::<f32>()
        .clamp(0.0, 100.0);
    let mut reasons = dimensions.clone();
    reasons.sort_by(|left, right| {
        (right.score * right.weight)
            .partial_cmp(&(left.score * left.weight))
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.dimension.cmp(&right.dimension))
    });

    MatchResult {
        data_gaps: listing.data_gaps.clone(),
        listing,
        overall_score: round_one(overall_score),
        dimension_scores: dimensions,
        recommendation_reasons: reasons
            .into_iter()
            .take(3)
            .map(|item| item.reason)
            .collect(),
        unmet_conditions: unmet,
        satisfied_hard_constraints: assessments.satisfied_hard,
        unmet_hard_constraints: assessments.unmet_hard,
        unverified_hard_constraints: assessments.unverified_hard,
        unmet_preferences: assessments.unmet_preferences,
        area_relaxed,
    }
}

#[derive(Default)]
struct ConstraintAssessments {
    satisfied_hard: Vec<ConstraintAssessment>,
    unmet_hard: Vec<ConstraintAssessment>,
    unverified_hard: Vec<ConstraintAssessment>,
    unmet_preferences: Vec<ConstraintAssessment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AssessmentState {
    Met,
    Unmet,
    Unverified,
}

fn assess_constraints(
    demand: &DemandDraft,
    listing: &ListingSummary,
    reference_date: Date,
) -> ConstraintAssessments {
    let mut output = ConstraintAssessments::default();
    for key in all_constraint_keys() {
        let Some(level) = effective_level(demand, &key) else {
            continue;
        };
        let (state, detail) = assess_constraint(demand, listing, &key, reference_date);
        let assessment = ConstraintAssessment {
            label: constraint_label(&key).into(),
            key: Some(key),
            detail,
        };
        match (level, state) {
            (ConstraintLevel::Hard, AssessmentState::Met) => output.satisfied_hard.push(assessment),
            (ConstraintLevel::Hard, AssessmentState::Unmet) => output.unmet_hard.push(assessment),
            (ConstraintLevel::Hard, AssessmentState::Unverified) => {
                output.unverified_hard.push(assessment)
            }
            (ConstraintLevel::Preference, AssessmentState::Met) => {}
            (ConstraintLevel::Preference, AssessmentState::Unmet | AssessmentState::Unverified) => {
                output.unmet_preferences.push(assessment)
            }
            (ConstraintLevel::Unspecified, _) => {}
        }
    }

    output
        .unverified_hard
        .extend(
            demand
                .hard_conditions
                .iter()
                .map(|condition| ConstraintAssessment {
                    key: None,
                    label: condition.clone(),
                    detail: format!("其他硬条件“{condition}”尚未映射为可自动验证字段"),
                }),
        );
    output
        .unmet_preferences
        .extend(
            demand
                .preference_conditions
                .iter()
                .map(|condition| ConstraintAssessment {
                    key: None,
                    label: condition.clone(),
                    detail: format!("其他偏好“{condition}”需要顾问确认"),
                }),
        );
    output
}

fn effective_level(demand: &DemandDraft, key: &ConstraintKey) -> Option<ConstraintLevel> {
    if let Some(priority) = demand
        .constraint_priorities
        .iter()
        .find(|priority| &priority.key == key)
    {
        return match priority.level {
            ConstraintLevel::Hard => Some(ConstraintLevel::Hard),
            ConstraintLevel::Preference => Some(ConstraintLevel::Preference),
            ConstraintLevel::Unspecified => None,
        };
    }
    if *key == ConstraintKey::FreightElevator
        && demand.constraints.needs_freight_elevator == Some(true)
    {
        return Some(ConstraintLevel::Hard);
    }
    constraint_has_value(demand, key).then_some(ConstraintLevel::Preference)
}

fn all_constraint_keys() -> [ConstraintKey; 10] {
    [
        ConstraintKey::Budget,
        ConstraintKey::FreightElevator,
        ConstraintKey::ElevatorCapacity,
        ConstraintKey::PowerCapacity,
        ConstraintKey::FireSafety,
        ConstraintKey::TruckAccess,
        ConstraintKey::LoadingDock,
        ConstraintKey::Sublease,
        ConstraintKey::Floor,
        ConstraintKey::MoveIn,
    ]
}

fn constraint_label(key: &ConstraintKey) -> &'static str {
    match key {
        ConstraintKey::Budget => "预算",
        ConstraintKey::FreightElevator => "货梯",
        ConstraintKey::ElevatorCapacity => "电梯吨位",
        ConstraintKey::PowerCapacity => "用电容量",
        ConstraintKey::FireSafety => "消防要求",
        ConstraintKey::TruckAccess => "货车通行",
        ConstraintKey::LoadingDock => "装卸条件",
        ConstraintKey::Sublease => "分租条件",
        ConstraintKey::Floor => "楼层",
        ConstraintKey::MoveIn => "入驻时间",
    }
}

fn assess_constraint(
    demand: &DemandDraft,
    listing: &ListingSummary,
    key: &ConstraintKey,
    reference_date: Date,
) -> (AssessmentState, String) {
    let constraints = &demand.constraints;
    match key {
        ConstraintKey::Budget => {
            let Some(maximum) = constraints.rent_max_cents else {
                return (AssessmentState::Unverified, "预算上限未填写".into());
            };
            let actual = match constraints.rent_unit {
                Some(RentUnit::YuanPerSquareMetreMonth) => listing.rent_cents_per_sqm_month,
                Some(RentUnit::YuanPerMonth) => listing.monthly_rent_cents,
                None => return (AssessmentState::Unverified, "预算单位未填写".into()),
            };
            if actual <= maximum {
                (AssessmentState::Met, "房源租金不高于预算上限".into())
            } else {
                (AssessmentState::Unmet, "房源租金高于预算上限".into())
            }
        }
        ConstraintKey::FreightElevator => match listing.has_freight_elevator {
            Some(true) => (AssessmentState::Met, "房源已确认配有货梯".into()),
            Some(false) => (AssessmentState::Unmet, "房源没有货梯".into()),
            None => (AssessmentState::Unverified, "房源货梯数据缺失".into()),
        },
        ConstraintKey::ElevatorCapacity => {
            let Some(required) = constraints.elevator_min_tons else {
                return (AssessmentState::Unverified, "最低电梯吨位未填写".into());
            };
            match listing.elevator_capacity_tons {
                Some(actual) if actual >= required => {
                    (AssessmentState::Met, "电梯吨位满足要求".into())
                }
                Some(_) => (AssessmentState::Unmet, "电梯吨位低于要求".into()),
                None => (AssessmentState::Unverified, "房源电梯吨位数据缺失".into()),
            }
        }
        ConstraintKey::PowerCapacity => {
            let Some(required) = constraints.power_capacity_kva else {
                return (AssessmentState::Unverified, "用电容量未填写".into());
            };
            match listing.power_capacity_kva {
                Some(actual) if actual >= required => {
                    (AssessmentState::Met, "用电容量满足要求".into())
                }
                Some(_) => (AssessmentState::Unmet, "用电容量低于要求".into()),
                None => (AssessmentState::Unverified, "房源用电容量数据缺失".into()),
            }
        }
        ConstraintKey::FireSafety => {
            let Some(required) = constraints.fire_requirement.as_deref() else {
                return (AssessmentState::Unverified, "消防要求未填写".into());
            };
            match listing.fire_rating.as_deref() {
                Some(actual) if actual.contains(required) || required.contains(actual) => {
                    (AssessmentState::Met, "消防信息满足要求".into())
                }
                Some(_) => (AssessmentState::Unmet, "消防信息不满足要求".into()),
                None => (AssessmentState::Unverified, "房源消防数据缺失".into()),
            }
        }
        ConstraintKey::TruckAccess => match listing.truck_access {
            Some(true) => (AssessmentState::Met, "房源支持货车通行".into()),
            Some(false) => (AssessmentState::Unmet, "房源不支持货车通行".into()),
            None => (AssessmentState::Unverified, "房源货车通行数据缺失".into()),
        },
        ConstraintKey::LoadingDock => match listing.loading_dock {
            Some(true) => (AssessmentState::Met, "房源具备装卸条件".into()),
            Some(false) => (AssessmentState::Unmet, "房源不具备要求的装卸条件".into()),
            None => (AssessmentState::Unverified, "房源装卸数据缺失".into()),
        },
        ConstraintKey::Sublease => match constraints.accepts_sublease {
            Some(true) => (AssessmentState::Met, "用户接受分租，不构成房源阻塞".into()),
            Some(false) => match listing.allows_sublease {
                Some(false) => (AssessmentState::Met, "房源可按不分租条件洽谈".into()),
                Some(true) => (AssessmentState::Unmet, "房源为可分租方案".into()),
                None => (AssessmentState::Unverified, "房源分租数据缺失".into()),
            },
            None => (AssessmentState::Unverified, "是否接受分租未填写".into()),
        },
        ConstraintKey::Floor => {
            let Some(required) = constraints.floor_preference.as_deref() else {
                return (AssessmentState::Unverified, "楼层要求未填写".into());
            };
            match listing.floor_label.as_deref() {
                Some(actual) if actual.contains(required) || required.contains(actual) => {
                    (AssessmentState::Met, "楼层满足要求".into())
                }
                Some(_) => (AssessmentState::Unmet, "楼层不满足要求".into()),
                None => (AssessmentState::Unverified, "房源楼层数据缺失".into()),
            }
        }
        ConstraintKey::MoveIn => assess_move_in(demand, listing, reference_date),
    }
}

fn assess_move_in(
    demand: &DemandDraft,
    listing: &ListingSummary,
    reference_date: Date,
) -> (AssessmentState, String) {
    let Some(required) = demand.constraints.move_in_time.as_deref() else {
        return (AssessmentState::Unverified, "入驻时间未填写".into());
    };
    let Ok(deadline) = move_in_deadline(required, reference_date) else {
        return (AssessmentState::Unverified, "入驻时间格式无效".into());
    };
    let Ok(available) = parse_iso_date(&listing.available_from) else {
        return (
            AssessmentState::Unverified,
            "房源可入住日期无效或缺失".into(),
        );
    };
    if available <= deadline {
        (AssessmentState::Met, "可入驻日期不晚于需求截止日".into())
    } else {
        (AssessmentState::Unmet, "可入驻日期晚于需求截止日".into())
    }
}

fn dimension(name: &str, score: f32, reason: impl Into<String>) -> MatchDimensionScore {
    let weight = WEIGHTS
        .iter()
        .find(|(dimension, _)| *dimension == name)
        .map(|(_, value)| *value)
        .unwrap_or_default();
    MatchDimensionScore {
        dimension: name.into(),
        score: round_one(score),
        weight,
        reason: reason.into(),
    }
}

fn area_score(demand: &DemandDraft, area: u32, relaxed: bool) -> f32 {
    let min = demand.constraints.area_min_sqm.unwrap_or(area);
    let max = demand.constraints.area_max_sqm.unwrap_or(area);
    let midpoint = (min + max) as f32 / 2.0;
    let deviation = ((area as f32 - midpoint).abs() / midpoint.max(1.0)).min(1.0);
    (100.0 - deviation * 40.0 - if relaxed { 8.0 } else { 0.0 }).max(40.0)
}

fn score_cost(demand: &DemandDraft, listing: &ListingSummary) -> (f32, String, Option<String>) {
    let Some(max) = demand.constraints.rent_max_cents else {
        return (80.0, "未提供预算，成本维度采用中性分".into(), None);
    };
    let actual = match demand.constraints.rent_unit {
        Some(RentUnit::YuanPerSquareMetreMonth) => listing.rent_cents_per_sqm_month,
        _ => listing.monthly_rent_cents,
    };
    if actual <= max {
        (100.0, "租金在预算上限内".into(), None)
    } else {
        let ratio = actual as f32 / max.max(1) as f32;
        (
            (100.0 - (ratio - 1.0) * 200.0).clamp(0.0, 75.0),
            "租金高于预算上限".into(),
            Some("预算条件未满足".into()),
        )
    }
}

fn score_production(demand: &DemandDraft, listing: &ListingSummary) -> (f32, String, Vec<String>) {
    let mut scores = Vec::new();
    let mut unmet = Vec::new();
    if demand.constraints.needs_freight_elevator == Some(true) {
        let ok = listing.has_freight_elevator == Some(true)
            && demand.constraints.elevator_min_tons.map_or(true, |needed| {
                listing
                    .elevator_capacity_tons
                    .is_some_and(|actual| actual >= needed)
            });
        scores.push(if ok { 100.0 } else { 0.0 });
        if !ok {
            unmet.push("货梯或电梯吨位条件未满足或无法验证".into());
        }
    }
    if let Some(needed) = demand.constraints.power_capacity_kva {
        let ok = listing
            .power_capacity_kva
            .is_some_and(|actual| actual >= needed);
        scores.push(if ok { 100.0 } else { 0.0 });
        if !ok {
            unmet.push("用电容量条件未满足或无法验证".into());
        }
    }
    if scores.is_empty() {
        (85.0, "未提出额外生产设施指标".into(), unmet)
    } else {
        let score = scores.iter().sum::<f32>() / scores.len() as f32;
        (
            score,
            if unmet.is_empty() {
                "货梯与用电配置满足需求"
            } else {
                "部分生产设施需要放宽或确认"
            }
            .into(),
            unmet,
        )
    }
}

fn score_logistics(demand: &DemandDraft, listing: &ListingSummary) -> (f32, String, Vec<String>) {
    let wants_truck = demand.constraints.logistics_requirement.is_some();
    let wants_loading = demand.constraints.loading_requirement.is_some();
    if !wants_truck && !wants_loading {
        return (85.0, "未提出额外物流装卸指标".into(), Vec::new());
    }
    let mut values = Vec::new();
    let mut unmet = Vec::new();
    if wants_truck {
        values.push(if listing.truck_access == Some(true) {
            100.0
        } else {
            0.0
        });
        if listing.truck_access != Some(true) {
            unmet.push("物流货车通行条件未满足或无法验证".into());
        }
    }
    if wants_loading {
        values.push(if listing.loading_dock == Some(true) {
            100.0
        } else {
            0.0
        });
        if listing.loading_dock != Some(true) {
            unmet.push("装卸条件未满足或无法验证".into());
        }
    }
    let score = values.iter().sum::<f32>() / values.len() as f32;
    (
        score,
        if unmet.is_empty() {
            "货车通行与装卸条件匹配"
        } else {
            "物流装卸条件部分不匹配"
        }
        .into(),
        unmet,
    )
}

fn score_compliance(demand: &DemandDraft, listing: &ListingSummary) -> (f32, String, Vec<String>) {
    let Some(required) = &demand.constraints.fire_requirement else {
        return (90.0, "未指定消防等级，待顾问复核用途".into(), Vec::new());
    };
    if listing
        .fire_rating
        .as_deref()
        .is_some_and(|actual| actual.contains(required) || required.contains(actual))
    {
        (100.0, "消防等级与需求一致".into(), Vec::new())
    } else {
        (
            20.0,
            "消防等级不一致或数据待确认".into(),
            vec!["消防要求未满足或无法验证".into()],
        )
    }
}

fn score_move_in(
    demand: &DemandDraft,
    listing: &ListingSummary,
    reference_date: Date,
) -> (f32, String, Option<String>) {
    if demand.constraints.move_in_time.is_none() {
        return (85.0, "未指定入驻时间".into(), None);
    }
    match assess_move_in(demand, listing, reference_date).0 {
        AssessmentState::Met => (100.0, "可入驻时间满足需求".into(), None),
        AssessmentState::Unmet => (
            20.0,
            "可入驻时间晚于需求".into(),
            Some("入驻时间条件未满足".into()),
        ),
        AssessmentState::Unverified => (
            20.0,
            "可入驻时间无法验证".into(),
            Some("入驻时间条件无法验证".into()),
        ),
    }
}

fn compare_matches(left: &MatchResult, right: &MatchResult) -> Ordering {
    right
        .overall_score
        .partial_cmp(&left.overall_score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            right
                .listing
                .is_self_operated
                .cmp(&left.listing.is_self_operated)
        })
        .then_with(|| {
            right
                .listing
                .verification_level
                .rank()
                .cmp(&left.listing.verification_level.rank())
        })
        .then_with(|| right.listing.updated_at.cmp(&left.listing.updated_at))
        .then_with(|| left.listing.listing_id.cmp(&right.listing.listing_id))
}

fn round_one(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use time::{Date, Month};

    use super::*;
    use crate::services::miniapp::{
        clock::FixedClock,
        repository::FixtureListingRepository,
        types::{ConstraintPriority, DemandConstraints, ListingVerificationLevel, SpaceType},
    };

    fn fixed_clock() -> Arc<FixedClock> {
        Arc::new(FixedClock::new(
            1_723_305_600,
            Date::from_calendar_date(2024, Month::August, 10).expect("date"),
        ))
    }

    fn demand(min: u32, max: u32) -> DemandDraft {
        DemandDraft {
            raw_text: "松山湖厂房".into(),
            constraints: DemandConstraints {
                space_type: Some(SpaceType::Factory),
                target_towns: vec!["松山湖".into()],
                area_min_sqm: Some(min),
                area_max_sqm: Some(max),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn priority(key: ConstraintKey, level: ConstraintLevel) -> ConstraintPriority {
        ConstraintPriority { key, level }
    }

    fn score_one(demand: &DemandDraft, mut listing: ListingSummary) -> MatchResult {
        listing.space_type = demand.constraints.space_type.clone().expect("space type");
        listing.town = demand.constraints.target_towns[0].clone();
        listing.available_area_sqm = demand.constraints.area_min_sqm.expect("area");
        score_listing(demand, listing, false, fixed_clock().today_china())
    }

    #[test]
    fn l0和l1不进入推荐且出租状态先过滤() {
        let response = match_listings_with_clock(
            &demand(1400, 1650),
            &FixtureListingRepository::default(),
            fixed_clock().as_ref(),
        );
        assert!(response.matches.len() >= 3);
        assert!(response.matches.iter().all(|item| {
            item.listing.is_self_operated || item.listing.verification_level.rank() >= 2
        }));
        assert!(response
            .matches
            .iter()
            .all(|item| item.listing.rental_status == "available"));
    }

    #[test]
    fn 无严格面积结果时只放宽百分之二十() {
        let response = match_listings_with_clock(
            &demand(1800, 1900),
            &FixtureListingRepository::default(),
            fixed_clock().as_ref(),
        );
        assert!(response.used_area_relaxation);
        assert!(response.matches.iter().all(|item| item.area_relaxed));
        assert!(response
            .matches
            .iter()
            .all(|item| (1440..=2280).contains(&item.listing.available_area_sqm)));
    }

    #[test]
    fn 每个结果都有七维得分且总分在范围内() {
        let result = match_listings_with_clock(
            &demand(1400, 1650),
            &FixtureListingRepository::default(),
            fixed_clock().as_ref(),
        )
        .matches
        .remove(0);
        assert_eq!(result.dimension_scores.len(), 7);
        assert!((0.0..=100.0).contains(&result.overall_score));
        assert_eq!(result.recommendation_reasons.len(), 3);
    }

    #[test]
    fn 所有房源未核验时返回空结果() {
        let mut listings = FixtureListingRepository::default().list();
        for listing in &mut listings {
            listing.verification_level = ListingVerificationLevel::L0;
            listing.is_self_operated = false;
        }
        let response = match_listings_with_clock(
            &demand(1, 10000),
            &FixtureListingRepository::from_listings(listings),
            fixed_clock().as_ref(),
        );
        assert!(response.matches.is_empty());
        assert!(response.next_step_suggestion.is_some());
    }

    #[test]
    fn 同分最终按稳定id排序() {
        let source = FixtureListingRepository::default().list().remove(0);
        let mut left = source.clone();
        left.listing_id = "demo-a".into();
        let mut right = source;
        right.listing_id = "demo-b".into();
        let response = match_listings_with_clock(
            &demand(1400, 1650),
            &FixtureListingRepository::from_listings(vec![right, left]),
            fixed_clock().as_ref(),
        );
        assert_eq!(response.matches[0].listing.listing_id, "demo-a");
    }

    #[test]
    fn 货梯默认硬条件且不依赖自由文本() {
        let mut request = demand(900, 1100);
        request.constraints.needs_freight_elevator = Some(true);
        let mut listing = FixtureListingRepository::default().list().remove(4);
        listing.has_freight_elevator = Some(false);
        let result = score_one(&request, listing);
        assert!(request.hard_conditions.is_empty());
        assert_eq!(
            result.unmet_hard_constraints[0].key,
            Some(ConstraintKey::FreightElevator)
        );
    }

    #[test]
    fn 货梯显式偏好不阻断且显式不指定完全跳过() {
        let mut request = demand(900, 1100);
        request.constraints.needs_freight_elevator = Some(true);
        let mut listing = FixtureListingRepository::default().list().remove(4);
        listing.has_freight_elevator = Some(false);
        request.constraint_priorities = vec![priority(
            ConstraintKey::FreightElevator,
            ConstraintLevel::Preference,
        )];
        let preference = score_one(&request, listing.clone());
        assert!(preference.unmet_hard_constraints.is_empty());
        assert_eq!(preference.unmet_preferences.len(), 1);

        request.constraint_priorities[0].level = ConstraintLevel::Unspecified;
        let unspecified = score_one(&request, listing);
        assert!(unspecified.unmet_hard_constraints.is_empty());
        assert!(unspecified.unverified_hard_constraints.is_empty());
        assert!(unspecified.unmet_preferences.is_empty());
    }

    #[test]
    fn 货梯显式硬条件无法核验时阻断() {
        let mut request = demand(900, 1100);
        request.constraints.needs_freight_elevator = Some(true);
        request.constraint_priorities = vec![priority(
            ConstraintKey::FreightElevator,
            ConstraintLevel::Hard,
        )];
        let mut listing = FixtureListingRepository::default().list().remove(0);
        listing.has_freight_elevator = None;
        let result = score_one(&request, listing);
        assert!(result.unmet_hard_constraints.is_empty());
        assert_eq!(result.unverified_hard_constraints.len(), 1);
    }

    #[test]
    fn 用电硬条件不足拒绝而偏好仅展示() {
        let mut request = demand(1400, 1650);
        request.constraints.power_capacity_kva = Some(700);
        let listing = FixtureListingRepository::default().list().remove(1);
        request.constraint_priorities = vec![priority(
            ConstraintKey::PowerCapacity,
            ConstraintLevel::Hard,
        )];
        let hard = score_one(&request, listing.clone());
        assert_eq!(hard.unmet_hard_constraints.len(), 1);

        request.constraint_priorities[0].level = ConstraintLevel::Preference;
        let preference = score_one(&request, listing);
        assert!(preference.unmet_hard_constraints.is_empty());
        assert_eq!(preference.unmet_preferences.len(), 1);
    }

    #[test]
    fn 硬条件数据缺失标记无法验证() {
        let mut request = demand(1400, 1650);
        request.constraints.power_capacity_kva = Some(500);
        request.constraint_priorities = vec![priority(
            ConstraintKey::PowerCapacity,
            ConstraintLevel::Hard,
        )];
        let mut listing = FixtureListingRepository::default().list().remove(0);
        listing.power_capacity_kva = None;
        let result = score_one(&request, listing);
        assert_eq!(result.unverified_hard_constraints.len(), 1);
    }

    #[test]
    fn 预算硬条件超出时拒绝() {
        let mut request = demand(1400, 1650);
        request.constraints.rent_max_cents = Some(4_000_000);
        request.constraints.rent_unit = Some(RentUnit::YuanPerMonth);
        request.constraint_priorities =
            vec![priority(ConstraintKey::Budget, ConstraintLevel::Hard)];
        let listing = FixtureListingRepository::default().list().remove(0);
        let result = score_one(&request, listing);
        assert_eq!(result.unmet_hard_constraints.len(), 1);
    }

    #[test]
    fn 未知自由文本硬条件不能伪装已验证() {
        let mut request = demand(1400, 1650);
        request.hard_conditions = vec!["必须临近指定供应商".into()];
        let listing = FixtureListingRepository::default().list().remove(0);
        let result = score_one(&request, listing);
        assert_eq!(result.unverified_hard_constraints.len(), 1);
        assert!(result.unverified_hard_constraints[0].key.is_none());
    }

    #[test]
    fn 满足全部硬条件时没有阻塞() {
        let mut request = demand(1400, 1650);
        request.constraints.needs_freight_elevator = Some(true);
        request.constraints.power_capacity_kva = Some(500);
        request.constraint_priorities = vec![
            priority(ConstraintKey::FreightElevator, ConstraintLevel::Hard),
            priority(ConstraintKey::PowerCapacity, ConstraintLevel::Hard),
        ];
        let listing = FixtureListingRepository::default().list().remove(0);
        let result = score_one(&request, listing);
        assert!(result.unmet_hard_constraints.is_empty());
        assert!(result.unverified_hard_constraints.is_empty());
        assert_eq!(result.satisfied_hard_constraints.len(), 2);
    }

    #[test]
    fn 动态日期覆盖立即三十天九十天和明确日期边界() {
        let reference = Date::from_calendar_date(2024, Month::January, 31).expect("date");
        let mut request = demand(1400, 1650);
        let mut listing = FixtureListingRepository::default().list().remove(0);

        request.constraints.move_in_time = Some("immediate".into());
        listing.available_from = "2024-01-31".into();
        assert_eq!(
            assess_move_in(&request, &listing, reference).0,
            AssessmentState::Met
        );
        listing.available_from = "2024-02-01".into();
        assert_eq!(
            assess_move_in(&request, &listing, reference).0,
            AssessmentState::Unmet
        );

        request.constraints.move_in_time = Some("within_30_days".into());
        listing.available_from = "2024-03-01".into();
        assert_eq!(
            assess_move_in(&request, &listing, reference).0,
            AssessmentState::Met
        );
        listing.available_from = "2024-03-02".into();
        assert_eq!(
            assess_move_in(&request, &listing, reference).0,
            AssessmentState::Unmet
        );

        request.constraints.move_in_time = Some("within_90_days".into());
        listing.available_from = "2024-04-30".into();
        assert_eq!(
            assess_move_in(&request, &listing, reference).0,
            AssessmentState::Met
        );
        listing.available_from = "2024-05-01".into();
        assert_eq!(
            assess_move_in(&request, &listing, reference).0,
            AssessmentState::Unmet
        );

        request.constraints.move_in_time = Some("2024-02-29".into());
        listing.available_from = "2024-02-29".into();
        assert_eq!(
            assess_move_in(&request, &listing, reference).0,
            AssessmentState::Met
        );
    }

    #[test]
    fn 动态日期跨年边界() {
        let reference = Date::from_calendar_date(2024, Month::December, 20).expect("date");
        let mut request = demand(1400, 1650);
        request.constraints.move_in_time = Some("within_30_days".into());
        let mut listing = FixtureListingRepository::default().list().remove(0);
        listing.available_from = "2025-01-19".into();
        assert_eq!(
            assess_move_in(&request, &listing, reference).0,
            AssessmentState::Met
        );
        listing.available_from = "2025-01-20".into();
        assert_eq!(
            assess_move_in(&request, &listing, reference).0,
            AssessmentState::Unmet
        );
    }
}
