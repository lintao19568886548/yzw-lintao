export type SpaceType = 'factory' | 'warehouse' | 'office'
export type RentUnit = 'yuan_per_month' | 'yuan_per_square_metre_month'
export type ListingVerificationLevel = 'l0' | 'l1' | 'l2' | 'l3'
export type AdvisorAssignmentStatus = 'pending_assignment'
export type ConstraintKey = 'budget' | 'freight_elevator' | 'elevator_capacity' | 'power_capacity' | 'fire_safety' | 'truck_access' | 'loading_dock' | 'sublease' | 'floor' | 'move_in'
export type ConstraintLevel = 'hard' | 'preference'

export interface ConstraintPriority {
  key: ConstraintKey
  level: ConstraintLevel
}

export interface DemandConstraints {
  space_type: SpaceType | null
  target_towns: string[]
  area_min_sqm: number | null
  area_max_sqm: number | null
  rent_min_cents: number | null
  rent_max_cents: number | null
  rent_unit: RentUnit | null
  move_in_time: string | null
  floor_preference: string | null
  needs_freight_elevator: boolean | null
  elevator_min_tons: number | null
  power_capacity_kva: number | null
  fire_requirement: string | null
  logistics_requirement: string | null
  loading_requirement: string | null
  accepts_sublease: boolean | null
  other_notes: string | null
}

export interface DemandDraft {
  raw_text: string
  constraints: DemandConstraints
  hard_conditions: string[]
  preference_conditions: string[]
  constraint_priorities: ConstraintPriority[]
  missing_fields: string[]
  ai_confidence: number
}

export interface DemandInterpretation {
  demand: DemandDraft
  provider: string
  fallback_reason: string | null
}

export interface ListingSummary {
  listing_id: string
  listing_name: string
  space_type: SpaceType
  town: string
  available_area_sqm: number
  rent_cents_per_sqm_month: number
  monthly_rent_cents: number
  verification_level: ListingVerificationLevel
  is_self_operated: boolean
  source_label: string
  updated_at: string
  available_from: string
  rental_status: string
  power_capacity_kva: number | null
  has_freight_elevator: boolean | null
  elevator_capacity_tons: number | null
  fire_rating: string | null
  truck_access: boolean | null
  loading_dock: boolean | null
  allows_sublease: boolean | null
  floor_label: string | null
  data_gaps: string[]
}

export interface MatchDimensionScore {
  dimension: string
  score: number
  weight: number
  reason: string
}

export interface MatchResult {
  listing: ListingSummary
  overall_score: number
  dimension_scores: MatchDimensionScore[]
  recommendation_reasons: string[]
  unmet_conditions: string[]
  satisfied_hard_constraints: ConstraintAssessment[]
  unmet_hard_constraints: ConstraintAssessment[]
  unverified_hard_constraints: ConstraintAssessment[]
  unmet_preferences: ConstraintAssessment[]
  area_relaxed: boolean
  data_gaps: string[]
}

export interface ConstraintAssessment {
  key: ConstraintKey | null
  label: string
  detail: string
}

export interface MatchResponse {
  matches: MatchResult[]
  used_area_relaxation: boolean
  next_step_suggestion: string | null
  demo_data: boolean
}

export interface LeadSubmission {
  demand: DemandDraft
  recommended_listing_ids: string[]
  source_channel: 'miniapp_ai_demand'
  idempotency_key: string
}

export interface LeadRecord {
  demand_number: string
  lead_number: string
  demand_snapshot: DemandDraft
  recommended_listing_ids: string[]
  source_channel: string
  status: AdvisorAssignmentStatus
  sla_minutes: number
  created_at_epoch_seconds: number
  temporary_storage: boolean
}

export interface ApiFieldError {
  field: string
  message: string
}

export interface ApiResponse<T> {
  code: string
  message: string
  request_id: string
  data: T | null
  errors: ApiFieldError[]
}

export interface DevSessionResponse {
  session_token: string
  masked_phone: string
  expires_at_epoch_seconds: number
  local_demo: boolean
}

export interface MetadataOptions {
  towns: string[]
  space_types: string[]
  rent_units: string[]
  verification_levels: string[]
  constraint_keys: ConstraintKey[]
  business_timezone: 'Asia/Shanghai'
  currency_storage_unit: 'cents'
  currency_display_unit: 'yuan'
  demo_data: boolean
}
