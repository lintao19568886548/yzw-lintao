import { createPinia, setActivePinia } from 'pinia'
import { restoreFlowState, useDemandForm } from '@/composables/useDemandForm'
import { AUTH_STORAGE_KEY, useAuthStore } from '@/stores/auth'
import { DEMAND_STORAGE_KEY, emptyDemand, useDemandStore } from '@/stores/demand'
import { installStorageMock } from './test-helpers'

describe('刷新与深链恢复', () => {
  beforeEach(() => {
    installStorageMock()
    setActivePinia(createPinia())
  })

  it('按认证、需求、页面表单顺序恢复首页和确认页字段', () => {
    const auth = useAuthStore()
    auth.setSession({
      session_token: 'demo-session', masked_phone: '139****0000',
      expires_at_epoch_seconds: Math.floor(Date.now() / 1000) + 3600, local_demo: true,
    })
    const store = useDemandStore()
    const demand = emptyDemand()
    demand.raw_text = '清溪找28.5元每平方米的厂房'
    demand.constraints.space_type = 'factory'
    demand.constraints.target_towns = ['清溪']
    demand.constraints.area_min_sqm = 1000
    demand.constraints.area_max_sqm = 1200
    demand.constraints.rent_max_cents = 2850
    demand.constraints.rent_unit = 'yuan_per_square_metre_month'
    demand.hard_conditions = ['其他未知硬条件']
    demand.preference_conditions = ['靠近高速']
    store.setInterpretation({ demand, provider: 'local-rules', fallback_reason: null })
    store.setMatches({ matches: [], used_area_relaxation: false, next_step_suggestion: '保留结果', demo_data: true })
    store.setLead({
      demand_number: 'DEMO-D-000001', lead_number: 'DEMO-L-000001', demand_snapshot: demand,
      recommended_listing_ids: ['demo-factory-001'], source_channel: 'miniapp_ai_demand',
      status: 'pending_assignment', sla_minutes: 15, created_at_epoch_seconds: 1, temporary_storage: true,
    })

    setActivePinia(createPinia())
    const restoredAuth = useAuthStore()
    const restoredStore = useDemandStore()
    const form = useDemandForm(restoredStore)
    const order: string[] = []
    const originalAuthHydrate = restoredAuth.hydrate.bind(restoredAuth)
    const originalDemandHydrate = restoredStore.hydrate.bind(restoredStore)
    restoredAuth.hydrate = () => { order.push('auth'); originalAuthHydrate() }
    restoredStore.hydrate = () => { order.push('demand'); originalDemandHydrate() }
    restoreFlowState(restoredAuth, restoredStore, () => { order.push('sync'); form.syncFromStore() })

    expect(order).toEqual(['auth', 'demand', 'sync'])
    expect(form.rawText.value).toContain('清溪')
    expect(form.selectedType.value).toBe('factory')
    expect(form.selectedTown.value).toBe('清溪')
    expect(form.areaMin.value).toBe(1000)
    expect(form.areaMax.value).toBe(1200)
    expect(form.budgetYuan.value).toBe('28.5')
    expect(form.hardText.value).toBe('其他未知硬条件')
    expect(form.preferenceText.value).toBe('靠近高速')
    expect(restoredStore.match_response?.next_step_suggestion).toBe('保留结果')
    expect(restoredStore.lead?.lead_number).toBe('DEMO-L-000001')
  })

  it('页面修改立即写回store且金额保持整数分', () => {
    const store = useDemandStore()
    const form = useDemandForm(store)
    form.rawText.value = '石龙30000元月租厂房'
    form.selectedType.value = 'factory'
    form.selectedTown.value = '石龙'
    form.areaMin.value = 900
    form.areaMax.value = 1000
    form.budgetYuan.value = '30000'
    form.applyHomeToStore()
    expect(store.demand.constraints.rent_max_cents).toBe(3_000_000)

    form.syncFromStore()
    form.rentMinYuan.value = '28.5'
    form.rentMaxYuan.value = '30'
    form.hardText.value = '必须独栋'
    form.preferenceText.value = '靠近高速'
    form.applyConfirmationToStore()
    expect(store.demand.constraints.rent_min_cents).toBe(2850)
    expect(store.demand.hard_conditions).toEqual(['必须独栋'])
  })

  it('损坏和旧版本缓存被清理且不抛异常', () => {
    const storage = installStorageMock({
      [AUTH_STORAGE_KEY]: '{bad-json',
      [DEMAND_STORAGE_KEY]: JSON.stringify({ version: 1, data: { old: true } }),
      'yizu-miniapp-demand-v1': JSON.stringify({ legacy: true }),
    })
    setActivePinia(createPinia())
    expect(() => useAuthStore().hydrate()).not.toThrow()
    expect(() => useDemandStore().hydrate()).not.toThrow()
    expect(storage.has(AUTH_STORAGE_KEY)).toBe(false)
    expect(storage.has(DEMAND_STORAGE_KEY)).toBe(false)
    expect(storage.has('yizu-miniapp-demand-v1')).toBe(false)
  })
})
