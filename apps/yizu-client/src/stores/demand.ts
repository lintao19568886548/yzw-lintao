import { defineStore } from 'pinia'
import type { DemandDraft, DemandInterpretation, LeadRecord, MatchResponse } from '@/types/domain'
import { readJson, removeStored, writeJson } from '@/utils/storage'

const DEMAND_STORAGE_KEY = 'yizu-miniapp-demand-v1'

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
      const saved = readJson<DemandSnapshot>(DEMAND_STORAGE_KEY)
      if (saved) this.$patch(saved)
    },
    persist(): void {
      writeJson<DemandSnapshot>(DEMAND_STORAGE_KEY, {
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
    },
  },
})
