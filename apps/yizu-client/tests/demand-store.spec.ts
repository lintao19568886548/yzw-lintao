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
})
