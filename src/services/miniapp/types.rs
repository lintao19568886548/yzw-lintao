use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceType {
    Factory,
    Warehouse,
    Office,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RentUnit {
    YuanPerMonth,
    YuanPerSquareMetreMonth,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct DemandConstraints {
    pub space_type: Option<SpaceType>,
    pub target_towns: Vec<String>,
    pub area_min_sqm: Option<u32>,
    pub area_max_sqm: Option<u32>,
    pub rent_min_cents: Option<u64>,
    pub rent_max_cents: Option<u64>,
    pub rent_unit: Option<RentUnit>,
    pub move_in_time: Option<String>,
    pub floor_preference: Option<String>,
    pub needs_freight_elevator: Option<bool>,
    pub elevator_min_tons: Option<f32>,
    pub power_capacity_kva: Option<u32>,
    pub fire_requirement: Option<String>,
    pub logistics_requirement: Option<String>,
    pub loading_requirement: Option<String>,
    pub accepts_sublease: Option<bool>,
    pub other_notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct DemandDraft {
    pub raw_text: String,
    pub constraints: DemandConstraints,
    pub hard_conditions: Vec<String>,
    pub preference_conditions: Vec<String>,
    pub missing_fields: Vec<String>,
    pub ai_confidence: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DemandInterpretation {
    pub demand: DemandDraft,
    pub provider: String,
    pub fallback_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListingVerificationLevel {
    L0,
    L1,
    L2,
    L3,
}

impl ListingVerificationLevel {
    pub fn rank(&self) -> u8 {
        match self {
            Self::L0 => 0,
            Self::L1 => 1,
            Self::L2 => 2,
            Self::L3 => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListingSummary {
    pub listing_id: String,
    pub listing_name: String,
    pub space_type: SpaceType,
    pub town: String,
    pub available_area_sqm: u32,
    pub rent_cents_per_sqm_month: u64,
    pub monthly_rent_cents: u64,
    pub verification_level: ListingVerificationLevel,
    pub is_self_operated: bool,
    pub source_label: String,
    pub updated_at: String,
    pub available_from: String,
    pub rental_status: String,
    pub power_capacity_kva: Option<u32>,
    pub has_freight_elevator: Option<bool>,
    pub elevator_capacity_tons: Option<f32>,
    pub fire_rating: Option<String>,
    pub truck_access: Option<bool>,
    pub loading_dock: Option<bool>,
    pub allows_sublease: Option<bool>,
    pub floor_label: Option<String>,
    pub data_gaps: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchDimensionScore {
    pub dimension: String,
    pub score: f32,
    pub weight: f32,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchResult {
    pub listing: ListingSummary,
    pub overall_score: f32,
    pub dimension_scores: Vec<MatchDimensionScore>,
    pub recommendation_reasons: Vec<String>,
    pub unmet_conditions: Vec<String>,
    pub area_relaxed: bool,
    pub data_gaps: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchResponse {
    pub matches: Vec<MatchResult>,
    pub used_area_relaxation: bool,
    pub next_step_suggestion: Option<String>,
    pub demo_data: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvisorAssignmentStatus {
    PendingAssignment,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeadSubmission {
    pub demand: DemandDraft,
    pub recommended_listing_ids: Vec<String>,
    pub source_channel: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeadRecord {
    pub demand_number: String,
    pub lead_number: String,
    pub demand_snapshot: DemandDraft,
    pub recommended_listing_ids: Vec<String>,
    pub source_channel: String,
    pub status: AdvisorAssignmentStatus,
    pub sla_minutes: u16,
    pub created_at_epoch_seconds: u64,
    pub temporary_storage: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiFieldError {
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: String,
    pub message: String,
    pub request_id: String,
    pub data: Option<T>,
    pub errors: Vec<ApiFieldError>,
}

impl<T> ApiResponse<T> {
    pub fn success(request_id: String, message: impl Into<String>, data: T) -> Self {
        Self {
            code: "OK".into(),
            message: message.into(),
            request_id,
            data: Some(data),
            errors: Vec::new(),
        }
    }

    pub fn failure(
        request_id: String,
        code: impl Into<String>,
        message: impl Into<String>,
        errors: Vec<ApiFieldError>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            request_id,
            data: None,
            errors,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevSessionRequest {
    pub phone: String,
    pub contact_confirmed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevSessionResponse {
    pub session_token: String,
    pub masked_phone: String,
    pub expires_at_epoch_seconds: u64,
    pub local_demo: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterpretDemandRequest {
    pub session_token: String,
    pub draft: DemandDraft,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchRequest {
    pub session_token: String,
    pub demand: DemandDraft,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmitLeadRequest {
    pub session_token: String,
    pub submission: LeadSubmission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataOptions {
    pub towns: Vec<String>,
    pub space_types: Vec<String>,
    pub rent_units: Vec<String>,
    pub verification_levels: Vec<String>,
    pub demo_data: bool,
}
