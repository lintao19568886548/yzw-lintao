import { createPinia, setActivePinia } from 'pinia'
import { emptyDemand, useDemandStore } from '@/stores/demand'
import { installStorageMock } from './test-helpers'

describe('需求 Store', () => {
  beforeEach(() => {
    installStorageMock()
    setActivePinia(createPinia())
  })

  it('保存结构化需求并阻止重复提交', () => {
    const store = useDemandStore()
    const demand = emptyDemand()
    demand.raw_text = '松山湖厂房'
    store.setInterpretation({ demand, provider: 'local', fallback_reason: null })
    expect(store.demand.raw_text).toBe('松山湖厂房')
    expect(store.beginLeadSubmission()).toBe(true)
    expect(store.beginLeadSubmission()).toBe(false)
    store.finishLeadSubmission()
    expect(store.beginLeadSubmission()).toBe(true)
  })

  it('只允许无硬条件阻断的房源加入意向并可取消', () => {
    const store = useDemandStore()
    const demand = emptyDemand()
    demand.raw_text = '松山湖厂房'
    const listing = {
      listing_id: 'ok', listing_name: '演示房源', space_type: 'factory' as const, town: '松山湖',
      available_area_sqm: 1000, rent_cents_per_sqm_month: 3000, monthly_rent_cents: 3_000_000,
      verification_level: 'l3' as const, is_self_operated: true, source_label: '演示', updated_at: '2026-08-01',
      available_from: '2026-08-01', rental_status: 'available', power_capacity_kva: 500,
      has_freight_elevator: true, elevator_capacity_tons: 3, fire_rating: '丙类消防', truck_access: true,
      loading_dock: true, allows_sublease: false, floor_label: '首层', data_gaps: [],
    }
    const base = { listing, overall_score: 90, dimension_scores: [], recommendation_reasons: [], unmet_conditions: [], satisfied_hard_constraints: [], unmet_hard_constraints: [], unverified_hard_constraints: [], unmet_preferences: [], area_relaxed: false, data_gaps: [] }
    store.setMatches({ matches: [base, { ...base, listing: { ...listing, listing_id: 'blocked' }, unmet_hard_constraints: [{ key: 'power_capacity', label: '用电', detail: '不足' }] }], used_area_relaxation: false, next_step_suggestion: null, demo_data: true })
    expect(store.toggleListing('ok')).toBe(true)
    expect(store.selected_listing_ids).toEqual(['ok'])
    expect(store.toggleListing('blocked')).toBe(false)
    expect(store.toggleListing('ok')).toBe(true)
    expect(store.selected_listing_ids).toEqual([])
  })
})
