import { createPinia, setActivePinia } from 'pinia'
import { classifyConstraintNear, createLocalMatches, interpretLocalDemand, localDemoApi } from '@/api/local-demo'
import { emptyDemand } from '@/stores/demand'
import { useHistoryStore } from '@/stores/history'
import { installStorageMock } from './test-helpers'

describe('宜租网小程序 MVP 主流程', () => {
  beforeEach(() => {
    installStorageMock()
    setActivePinia(createPinia())
  })

  it('组合句按字段邻近语义区分货梯偏好和用电硬条件', () => {
    const demand = emptyDemand()
    demand.raw_text = '我想在松山湖找1500平方米厂房，最好有3吨货梯，同时需要500kVA用电，预算每月4万元，一个月内入驻。'
    const parsed = interpretLocalDemand(demand)
    expect(classifyConstraintNear(demand.raw_text, ['货梯'])).toBe('preference')
    expect(classifyConstraintNear(demand.raw_text, ['用电'])).toBe('hard')
    expect(parsed.constraint_priorities).toEqual(expect.arrayContaining([
      { key: 'freight_elevator', level: 'preference' },
      { key: 'power_capacity', level: 'hard' },
    ]))
    expect(parsed.constraints.power_capacity_kva).toBe(500)
  })

  it('只推荐L2/L3可租房源并支持严格面积为空后的20%放宽', () => {
    const demand = emptyDemand()
    demand.raw_text = '松山湖厂房'
    demand.constraints.space_type = 'factory'
    demand.constraints.target_towns = ['松山湖']
    demand.constraints.area_min_sqm = 1700
    demand.constraints.area_max_sqm = 1750
    const response = createLocalMatches(demand)
    expect(response.used_area_relaxation).toBe(true)
    expect(response.matches.length).toBeGreaterThanOrEqual(3)
    expect(response.matches.every((match) => ['l2', 'l3'].includes(match.listing.verification_level))).toBe(true)
    expect(response.matches.every((match) => match.listing.rental_status === 'available')).toBe(true)
  })

  it('无匹配返回中文下一步提示', () => {
    const demand = emptyDemand()
    demand.raw_text = '石排写字楼'
    demand.constraints.space_type = 'office'
    demand.constraints.target_towns = ['石排']
    demand.constraints.area_min_sqm = 5000
    demand.constraints.area_max_sqm = 6000
    const response = createLocalMatches(demand)
    expect(response.matches).toEqual([])
    expect(response.next_step_suggestion).toContain('调整')
  })

  it('相同幂等键重复提交得到同一需求和线索编号并写入历史', async () => {
    const demand = emptyDemand()
    demand.raw_text = '松山湖1500平方米厂房，最好有货梯，需要500kVA用电'
    const parsed = interpretLocalDemand(demand)
    const matches = createLocalMatches(parsed)
    const selectable = matches.matches.find((match) => !match.unmet_hard_constraints.length && !match.unverified_hard_constraints.length)
    expect(selectable).toBeTruthy()
    const request = {
      session_token: 'local_demo_test',
      submission: { demand: parsed, recommended_listing_ids: [selectable!.listing.listing_id], source_channel: 'miniapp_ai_demand' as const, idempotency_key: 'lead_fixed_key' },
    }
    const first = await localDemoApi.submitLead(request)
    const second = await localDemoApi.submitLead(request)
    expect(second.lead_number).toBe(first.lead_number)
    expect(second.demand_number).toBe(first.demand_number)
    const history = useHistoryStore()
    history.recordSubmission(first, '138****8000', matches)
    history.recordSubmission(second, '138****8000', matches)
    expect(history.items).toHaveLength(1)
  })
})
