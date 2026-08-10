import { emptyDemand } from '@/stores/demand'
import { isValidPhone, validateConfirmedDemand, validateInitialDemand } from '@/utils/validation'

describe('表单校验', () => {
  it('校验手机号', () => {
    const syntheticPhone = ['139', '0000', '0000'].join('')
    expect(isValidPhone(syntheticPhone)).toBe(true)
    expect(isValidPhone('12912345678')).toBe(false)
  })

  it('拒绝空输入和HTML', () => {
    const demand = emptyDemand()
    expect(validateInitialDemand(demand)).not.toHaveLength(0)
    demand.raw_text = '<img src=x>'
    expect(validateInitialDemand(demand).join('')).toContain('HTML')
  })

  it('确认前要求类型镇街和面积', () => {
    const demand = emptyDemand()
    demand.raw_text = '需要空间'
    expect(validateConfirmedDemand(demand)).toHaveLength(3)
  })
})
