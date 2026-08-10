import { createPinia, setActivePinia } from 'pinia'
import { DONGGUAN_TOWN_FALLBACK, DONGGUAN_TOWNS } from '@/config/dongguan'
import { useMetadataStore } from '@/stores/metadata'
import type { MetadataOptions } from '@/types/domain'

function metadata(towns = [...DONGGUAN_TOWN_FALLBACK]): MetadataOptions {
  return {
    towns,
    space_types: ['factory', 'warehouse', 'office'],
    rent_units: ['yuan_per_month', 'yuan_per_square_metre_month'],
    verification_levels: ['l0', 'l1', 'l2', 'l3'],
    constraint_keys: ['budget', 'freight_elevator', 'elevator_capacity', 'power_capacity', 'fire_safety', 'truck_access', 'loading_dock', 'sublease', 'floor', 'move_in'],
    business_timezone: 'Asia/Shanghai', currency_storage_unit: 'cents', currency_display_unit: 'yuan', demo_data: true,
  }
}

describe('统一镇街元数据', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('完整覆盖33个镇街和此前缺失选项', () => {
    expect(DONGGUAN_TOWNS).toHaveLength(33)
    expect(DONGGUAN_TOWNS).toEqual(expect.arrayContaining(['石龙', '清溪', '凤岗', '桥头']))
  })

  it('成功时采用BFF权威元数据', async () => {
    const store = useMetadataStore()
    await store.load([], async () => metadata())
    expect(store.towns).toEqual(DONGGUAN_TOWN_FALLBACK)
    expect(store.notice).toBe('')
  })

  it('失败时保留完整降级列表和已选择镇街', async () => {
    const store = useMetadataStore()
    await store.load(['清溪'], async () => { throw new Error('offline') })
    expect(store.towns).toHaveLength(33)
    expect(store.towns).toContain('清溪')
    expect(store.notice).toContain('本地完整列表')
  })

  it('服务端返回不完整集合时安全降级', async () => {
    const store = useMetadataStore()
    await store.load(['石龙'], async () => metadata(['松山湖']))
    expect(store.towns).toEqual(DONGGUAN_TOWN_FALLBACK)
    expect(store.towns).toContain('石龙')
  })
})
