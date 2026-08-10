use std::cmp::Ordering;

use super::{
    repository::ListingRepository,
    types::{
        DemandDraft, ListingSummary, MatchDimensionScore, MatchResponse, MatchResult, RentUnit,
    },
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

    let mut matches = candidates
        .into_iter()
        .map(|listing| score_listing(demand, listing, relaxed))
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

pub fn hard_condition_violations(demand: &DemandDraft, result: &MatchResult) -> Vec<String> {
    demand
        .hard_conditions
        .iter()
        .filter(|hard| {
            result.unmet_conditions.iter().any(|unmet| {
                [
                    "货梯", "用电", "消防", "预算", "物流", "货车", "装卸", "分租", "楼层",
                ]
                .into_iter()
                .any(|keyword| hard.contains(keyword) && unmet.contains(keyword))
            })
        })
        .cloned()
        .collect()
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
            ((min as f32) * 0.8).floor() as u32,
            ((max as f32) * 1.2).ceil() as u32,
        )
    } else {
        (min, max)
    };
    (min..=max).contains(&area)
}

fn score_listing(demand: &DemandDraft, listing: ListingSummary, area_relaxed: bool) -> MatchResult {
    let mut dimensions = Vec::with_capacity(7);
    let mut unmet = Vec::new();

    dimensions.push(dimension("location", 100.0, "位于目标镇街"));

    let area_score = area_score(demand, listing.available_area_sqm, area_relaxed);
    dimensions.push(dimension(
        "space",
        area_score,
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

    let (move_in_score, move_in_reason, move_in_unmet) = score_move_in(demand, &listing);
    dimensions.push(dimension("move_in", move_in_score, move_in_reason));
    if let Some(reason) = move_in_unmet {
        unmet.push(reason);
    }

    if demand.constraints.accepts_sublease == Some(false) && listing.allows_sublease == Some(true) {
        unmet.push("房源允许分租，需确认可否整租".into());
    }
    if let Some(floor) = &demand.constraints.floor_preference {
        if listing
            .floor_label
            .as_deref()
            .is_some_and(|actual| !actual.contains(floor))
        {
            unmet.push(format!("楼层偏好“{floor}”未完全满足"));
        }
    }

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
        area_relaxed,
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
        let score = (100.0 - (ratio - 1.0) * 200.0).clamp(0.0, 75.0);
        (
            score,
            "租金高于预算上限".into(),
            Some("预算条件需要放宽".into()),
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
            unmet.push("货梯或电梯吨位条件未满足".into());
        }
    }
    if let Some(needed) = demand.constraints.power_capacity_kva {
        let ok = listing
            .power_capacity_kva
            .is_some_and(|actual| actual >= needed);
        scores.push(if ok { 100.0 } else { 0.0 });
        if !ok {
            unmet.push("用电容量条件未满足或数据待确认".into());
        }
    }
    if scores.is_empty() {
        (85.0, "未提出额外生产设施硬指标".into(), unmet)
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
            unmet.push("物流货车通行条件未满足".into());
        }
    }
    if wants_loading {
        values.push(if listing.loading_dock == Some(true) {
            100.0
        } else {
            0.0
        });
        if listing.loading_dock != Some(true) {
            unmet.push("装卸月台条件未满足或待确认".into());
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
    if listing.fire_rating.as_deref() == Some(required.as_str()) {
        (100.0, "消防等级与需求一致".into(), Vec::new())
    } else {
        (
            20.0,
            "消防等级不一致或数据待确认".into(),
            vec!["消防要求未满足或待确认".into()],
        )
    }
}

fn score_move_in(demand: &DemandDraft, listing: &ListingSummary) -> (f32, String, Option<String>) {
    let Some(required) = &demand.constraints.move_in_time else {
        return (85.0, "未指定入驻时间".into(), None);
    };
    let ok = match required.as_str() {
        "immediate" => listing.available_from.as_str() <= "2026-08-18",
        "within_30_days" => listing.available_from.as_str() <= "2026-09-09",
        "within_90_days" => listing.available_from.as_str() <= "2026-11-08",
        date if date.len() == 10 => listing.available_from.as_str() <= date,
        _ => true,
    };
    if ok {
        (100.0, "可入驻时间满足需求".into(), None)
    } else {
        (
            20.0,
            "可入驻时间晚于需求".into(),
            Some("入驻时间条件需要放宽".into()),
        )
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
    use super::*;
    use crate::services::miniapp::{
        repository::FixtureListingRepository,
        types::{DemandConstraints, ListingVerificationLevel, SpaceType},
    };

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

    #[test]
    fn l0和l1不进入推荐且出租状态先过滤() {
        let response = match_listings(&demand(1400, 1650), &FixtureListingRepository::default());
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
        let response = match_listings(&demand(1800, 1900), &FixtureListingRepository::default());
        assert!(response.used_area_relaxation);
        assert!(response.matches.iter().all(|item| item.area_relaxed));
        assert!(response
            .matches
            .iter()
            .all(|item| (1440..=2280).contains(&item.listing.available_area_sqm)));
    }

    #[test]
    fn 每个结果都有七维得分且总分在范围内() {
        let result = match_listings(&demand(1400, 1650), &FixtureListingRepository::default())
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
        let response = match_listings(
            &demand(1, 10000),
            &FixtureListingRepository::from_listings(listings),
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
        let response = match_listings(
            &demand(1400, 1650),
            &FixtureListingRepository::from_listings(vec![right, left]),
        );
        assert_eq!(response.matches[0].listing.listing_id, "demo-a");
    }
}
