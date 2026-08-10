import { defineStore } from 'pinia'
import type { DemandDraft, DemandInterpretation, LeadRecord, MatchResponse } from '@/types/domain'
import { readVersionedJson, removeStored, writeVersionedJson } from '@/utils/storage'

export const DEMAND_STORAGE_KEY = 'yizu-miniapp-demand-v2'
const LEGACY_DEMAND_STORAGE_KEY = 'yizu-miniapp-demand-v1'
const DEMAND_STORAGE_VERSION = 2

export function emptyDemand(): DemandDraft {
  return {
    raw_text: '',
    constraints: {
      space_type: null,
      target_towns: [],
      area_min_sqm: null,
      area_max_sqm: null,
      rent_min_cents: null,
      rent_max_cents: null,
      rent_unit: null,
      move_in_time: null,
      floor_preference: null,
      needs_freight_elevator: null,
      elevator_min_tons: null,
      power_capacity_kva: null,
      fire_requirement: null,
      logistics_requirement: null,
      loading_requirement: null,
      accepts_sublease: null,
      other_notes: null,
    },
    hard_conditions: [],
    preference_conditions: [],
    constraint_priorities: [],
    missing_fields: [],
    ai_confidence: 0,
  }
}

interface DemandSnapshot {
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

function isDemandSnapshot(value: unknown): value is DemandSnapshot {
  if (!value || typeof value !== 'object') return false
  const snapshot = value as Partial<DemandSnapshot>
  const demand = snapshot.demand as Partial<DemandDraft> | undefined
  return Boolean(
    demand
    && typeof demand.raw_text === 'string'
    && demand.constraints && typeof demand.constraints === 'object'
    && Array.isArray(demand.hard_conditions)
    && Array.isArray(demand.preference_conditions)
    && Array.isArray(demand.constraint_priorities)
    && Array.isArray(demand.missing_fields)
    && typeof snapshot.idempotency_key === 'string',
  )
}

export function newIdempotencyKey(now = Date.now(), random = Math.random()): string {
  return `lead_${now}_${Math.floor(random * 1_000_000_000).toString(36)}`
}

export const useDemandStore = defineStore('demand', {
  state: (): DemandState => ({
    demand: emptyDemand(),
    interpretation: null,
    match_response: null,
    lead: null,
    idempotency_key: '',
    interpreting: false,
    matching: false,
    submitting: false,
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
    setLead(value: LeadRecord): void {
      this.lead = value
      this.persist()
    },
    beginLeadSubmission(): boolean {
      if (this.submitting) return false
      this.submitting = true
      return true
    },
    finishLeadSubmission(): void {
      this.submitting = false
    },
    hydrate(): void {
      removeStored(LEGACY_DEMAND_STORAGE_KEY)
      const saved = readVersionedJson(DEMAND_STORAGE_KEY, DEMAND_STORAGE_VERSION, isDemandSnapshot)
      if (saved) this.$patch(saved)
    },
    persist(): void {
      writeVersionedJson<DemandSnapshot>(DEMAND_STORAGE_KEY, DEMAND_STORAGE_VERSION, {
        demand: this.demand,
        interpretation: this.interpretation,
        match_response: this.match_response,
        lead: this.lead,
        idempotency_key: this.idempotency_key,
      })
    },
    resetFlow(): void {
      this.$reset()
      removeStored(DEMAND_STORAGE_KEY)
      removeStored(LEGACY_DEMAND_STORAGE_KEY)
    },
  },
})
