import { defineStore } from 'pinia'
import type {
  ConstraintAssessment, ConstraintKey, ConstraintLevel, DemandConstraints, DemandDraft,
  DemandInterpretation, LeadRecord, ListingSummary, MatchDimensionScore, MatchResponse, MatchResult,
} from '@/types/domain'
import { CONSTRAINT_KEYS, CONSTRAINT_LEVELS, materializeLegacyPriorities } from '@/utils/constraint-priority'
import { readJson, readVersionedJson, removeStored, writeVersionedJson } from '@/utils/storage'

export const DEMAND_STORAGE_KEY = 'yizu-miniapp-demand-v3'
export const V2_DEMAND_STORAGE_KEY = 'yizu-miniapp-demand-v2'
const V1_DEMAND_STORAGE_KEY = 'yizu-miniapp-demand-v1'
const DEMAND_STORAGE_VERSION = 3

export function emptyDemand(): DemandDraft {
  return {
    raw_text: '',
    constraints: {
      space_type: null, target_towns: [], area_min_sqm: null, area_max_sqm: null,
      rent_min_cents: null, rent_max_cents: null, rent_unit: null, move_in_time: null,
      floor_preference: null, needs_freight_elevator: null, elevator_min_tons: null,
      power_capacity_kva: null, fire_requirement: null, logistics_requirement: null,
      loading_requirement: null, accepts_sublease: null, other_notes: null,
    },
    hard_conditions: [], preference_conditions: [], constraint_priorities: [], missing_fields: [], ai_confidence: 0,
  }
}

export interface DemandSnapshot {
  demand: DemandDraft
  interpretation: DemandInterpretation | null
  match_response: MatchResponse | null
  lead: LeadRecord | null
  idempotency_key: string
}

interface DemandState extends DemandSnapshot {
  interpreting: boolean
  matching: boolean
  submitting: boolean
}

type RecordValue = Record<string, unknown>
const SPACE_TYPES = ['factory', 'warehouse', 'office'] as const
const RENT_UNITS = ['yuan_per_month', 'yuan_per_square_metre_month'] as const
const VERIFICATION_LEVELS = ['l0', 'l1', 'l2', 'l3'] as const

function isRecord(value: unknown): value is RecordValue {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isOneOf<T extends string>(value: unknown, allowed: readonly T[]): value is T {
  return typeof value === 'string' && allowed.includes(value as T)
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function isNullableFiniteNumber(value: unknown): value is number | null {
  return value === null || isFiniteNumber(value)
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string'
}

function isNullableBoolean(value: unknown): value is boolean | null {
  return value === null || typeof value === 'boolean'
}

function isConstraints(value: unknown): value is DemandConstraints {
  if (!isRecord(value)) return false
  return (value.space_type === null || isOneOf(value.space_type, SPACE_TYPES))
    && isStringArray(value.target_towns)
    && isNullableFiniteNumber(value.area_min_sqm) && isNullableFiniteNumber(value.area_max_sqm)
    && isNullableFiniteNumber(value.rent_min_cents) && isNullableFiniteNumber(value.rent_max_cents)
    && (value.rent_unit === null || isOneOf(value.rent_unit, RENT_UNITS))
    && isNullableString(value.move_in_time) && isNullableString(value.floor_preference)
    && isNullableBoolean(value.needs_freight_elevator)
    && isNullableFiniteNumber(value.elevator_min_tons) && isNullableFiniteNumber(value.power_capacity_kva)
    && isNullableString(value.fire_requirement) && isNullableString(value.logistics_requirement)
    && isNullableString(value.loading_requirement) && isNullableBoolean(value.accepts_sublease)
    && isNullableString(value.other_notes)
}

function isPriorities(value: unknown, levels: readonly ConstraintLevel[]): boolean {
  if (!Array.isArray(value) || value.length > CONSTRAINT_KEYS.length) return false
  const seen = new Set<ConstraintKey>()
  return value.every((item) => {
    if (!isRecord(item) || !isOneOf(item.key, CONSTRAINT_KEYS) || !isOneOf(item.level, levels) || seen.has(item.key)) return false
    seen.add(item.key)
    return true
  })
}

function isDemand(value: unknown, levels: readonly ConstraintLevel[] = CONSTRAINT_LEVELS): value is DemandDraft {
  if (!isRecord(value)) return false
  return typeof value.raw_text === 'string' && isConstraints(value.constraints)
    && isStringArray(value.hard_conditions) && isStringArray(value.preference_conditions)
    && isPriorities(value.constraint_priorities, levels) && isStringArray(value.missing_fields)
    && isFiniteNumber(value.ai_confidence)
}

function isAssessment(value: unknown): value is ConstraintAssessment {
  return isRecord(value) && (value.key === null || isOneOf(value.key, CONSTRAINT_KEYS))
    && typeof value.label === 'string' && typeof value.detail === 'string'
}

function isScore(value: unknown): value is MatchDimensionScore {
  return isRecord(value) && typeof value.dimension === 'string' && isFiniteNumber(value.score)
    && isFiniteNumber(value.weight) && typeof value.reason === 'string'
}

function isListing(value: unknown): value is ListingSummary {
  if (!isRecord(value)) return false
  return typeof value.listing_id === 'string' && typeof value.listing_name === 'string'
    && isOneOf(value.space_type, SPACE_TYPES) && typeof value.town === 'string'
    && isFiniteNumber(value.available_area_sqm) && isFiniteNumber(value.rent_cents_per_sqm_month)
    && isFiniteNumber(value.monthly_rent_cents) && isOneOf(value.verification_level, VERIFICATION_LEVELS)
    && typeof value.is_self_operated === 'boolean' && typeof value.source_label === 'string'
    && typeof value.updated_at === 'string' && typeof value.available_from === 'string'
    && typeof value.rental_status === 'string' && isNullableFiniteNumber(value.power_capacity_kva)
    && isNullableBoolean(value.has_freight_elevator) && isNullableFiniteNumber(value.elevator_capacity_tons)
    && isNullableString(value.fire_rating) && isNullableBoolean(value.truck_access)
    && isNullableBoolean(value.loading_dock) && isNullableBoolean(value.allows_sublease)
    && isNullableString(value.floor_label) && isStringArray(value.data_gaps)
}

function isAssessmentArray(value: unknown): value is ConstraintAssessment[] {
  return Array.isArray(value) && value.every(isAssessment)
}

function isMatchResult(value: unknown): value is MatchResult {
  if (!isRecord(value)) return false
  return isListing(value.listing) && isFiniteNumber(value.overall_score)
    && Array.isArray(value.dimension_scores) && value.dimension_scores.every(isScore)
    && isStringArray(value.recommendation_reasons) && isStringArray(value.unmet_conditions)
    && isAssessmentArray(value.satisfied_hard_constraints) && isAssessmentArray(value.unmet_hard_constraints)
    && isAssessmentArray(value.unverified_hard_constraints) && isAssessmentArray(value.unmet_preferences)
    && typeof value.area_relaxed === 'boolean' && isStringArray(value.data_gaps)
}

function isMatchResponse(value: unknown): value is MatchResponse {
  return isRecord(value) && Array.isArray(value.matches) && value.matches.every(isMatchResult)
    && typeof value.used_area_relaxation === 'boolean' && isNullableString(value.next_step_suggestion)
    && typeof value.demo_data === 'boolean'
}

function isInterpretation(value: unknown, levels: readonly ConstraintLevel[]): value is DemandInterpretation | null {
  return value === null || (isRecord(value) && isDemand(value.demand, levels)
    && typeof value.provider === 'string' && isNullableString(value.fallback_reason))
}

function isLead(value: unknown, levels: readonly ConstraintLevel[]): value is LeadRecord | null {
  return value === null || (isRecord(value) && typeof value.demand_number === 'string'
    && typeof value.lead_number === 'string' && isDemand(value.demand_snapshot, levels)
    && isStringArray(value.recommended_listing_ids) && typeof value.source_channel === 'string'
    && value.status === 'pending_assignment' && isFiniteNumber(value.sla_minutes)
    && isFiniteNumber(value.created_at_epoch_seconds) && typeof value.temporary_storage === 'boolean')
}

function isDemandSnapshotWithLevels(value: unknown, levels: readonly ConstraintLevel[]): value is DemandSnapshot {
  if (!isRecord(value)) return false
  return isDemand(value.demand, levels) && isInterpretation(value.interpretation, levels)
    && (value.match_response === null || isMatchResponse(value.match_response))
    && isLead(value.lead, levels) && typeof value.idempotency_key === 'string'
}

export function isDemandSnapshot(value: unknown): value is DemandSnapshot {
  return isDemandSnapshotWithLevels(value, CONSTRAINT_LEVELS)
}

function migrateV2Snapshot(value: unknown): DemandSnapshot | null {
  const v2Levels: readonly ConstraintLevel[] = ['hard', 'preference']
  if (!isDemandSnapshotWithLevels(value, v2Levels)) return null
  const migrated = JSON.parse(JSON.stringify(value)) as DemandSnapshot
  migrated.demand = materializeLegacyPriorities(migrated.demand)
  if (migrated.interpretation) migrated.interpretation.demand = materializeLegacyPriorities(migrated.interpretation.demand)
  if (migrated.lead) migrated.lead.demand_snapshot = materializeLegacyPriorities(migrated.lead.demand_snapshot)
  return isDemandSnapshot(migrated) ? migrated : null
}

export function newIdempotencyKey(now = Date.now(), random = Math.random()): string {
  return `lead_${now}_${Math.floor(random * 1_000_000_000).toString(36)}`
}

export const useDemandStore = defineStore('demand', {
  state: (): DemandState => ({
    demand: emptyDemand(), interpretation: null, match_response: null, lead: null, idempotency_key: '',
    interpreting: false, matching: false, submitting: false,
  }),
  actions: {
    setInterpretation(value: DemandInterpretation): void {
      this.interpretation = value
      this.demand = value.demand
      this.match_response = null
      this.lead = null
      this.idempotency_key = ''
      this.persist()
    },
    setMatches(value: MatchResponse): void {
      this.match_response = value
      this.idempotency_key = newIdempotencyKey()
      this.persist()
    },
    setLead(value: LeadRecord): void { this.lead = value; this.persist() },
    beginLeadSubmission(): boolean {
      if (this.submitting) return false
      this.submitting = true
      return true
    },
    finishLeadSubmission(): void { this.submitting = false },
    hydrate(): void {
      try {
        removeStored(V1_DEMAND_STORAGE_KEY)
        const saved = readVersionedJson(DEMAND_STORAGE_KEY, DEMAND_STORAGE_VERSION, isDemandSnapshot)
        if (saved) { this.$patch(saved); removeStored(V2_DEMAND_STORAGE_KEY); return }
        const v2 = readJson<{ version: number; data: unknown }>(V2_DEMAND_STORAGE_KEY)
        removeStored(V2_DEMAND_STORAGE_KEY)
        const migrated = v2?.version === 2 ? migrateV2Snapshot(v2.data) : null
        if (migrated) { this.$patch(migrated); this.persist() }
      } catch {
        this.$reset()
        removeStored(DEMAND_STORAGE_KEY)
        removeStored(V2_DEMAND_STORAGE_KEY)
        removeStored(V1_DEMAND_STORAGE_KEY)
      }
    },
    persist(): void {
      writeVersionedJson<DemandSnapshot>(DEMAND_STORAGE_KEY, DEMAND_STORAGE_VERSION, {
        demand: this.demand, interpretation: this.interpretation, match_response: this.match_response,
        lead: this.lead, idempotency_key: this.idempotency_key,
      })
    },
    resetFlow(): void {
      this.$reset()
      removeStored(DEMAND_STORAGE_KEY)
      removeStored(V2_DEMAND_STORAGE_KEY)
      removeStored(V1_DEMAND_STORAGE_KEY)
    },
  },
})
