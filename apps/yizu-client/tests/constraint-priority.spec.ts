import { createPinia, setActivePinia } from 'pinia'
import { DEMAND_STORAGE_KEY, emptyDemand, useDemandStore } from '@/stores/demand'
import { effectiveConstraintLevel, setConstraintLevel } from '@/utils/constraint-priority'
import { installStorageMock } from './test-helpers'

describe('三态条件优先级', () => {
  beforeEach(() => {
    installStorageMock()
    setActivePinia(createPinia())
  })

  it('显式偏好覆盖货梯默认硬条件并可切换硬条件和不指定', () => {
    const demand = emptyDemand()
    demand.constraints.needs_freight_elevator = true
    expect(effectiveConstraintLevel(demand, 'freight_elevator')).toBe('hard')
    setConstraintLevel(demand, 'freight_elevator', 'preference')
    expect(effectiveConstraintLevel(demand, 'freight_elevator')).toBe('preference')
    setConstraintLevel(demand, 'freight_elevator', 'hard')
    expect(effectiveConstraintLevel(demand, 'freight_elevator')).toBe('hard')
    setConstraintLevel(demand, 'freight_elevator', 'unspecified')
    expect(effectiveConstraintLevel(demand, 'freight_elevator')).toBe('unspecified')
    expect(demand.constraint_priorities).toEqual([{ key: 'freight_elevator', level: 'unspecified' }])
  })

  it('刷新恢复后不指定保持显式值且不产生重复key', () => {
    const store = useDemandStore()
    store.demand.constraints.needs_freight_elevator = true
    setConstraintLevel(store.demand, 'freight_elevator', 'hard')
    setConstraintLevel(store.demand, 'freight_elevator', 'unspecified')
    store.persist()

    setActivePinia(createPinia())
    const restored = useDemandStore()
    restored.hydrate()
    expect(effectiveConstraintLevel(restored.demand, 'freight_elevator')).toBe('unspecified')
    expect(restored.demand.constraint_priorities).toHaveLength(1)
    expect((JSON.parse((uni.getStorageSync(DEMAND_STORAGE_KEY) as string)) as { version: number }).version).toBe(5)
  })
})
