import { centsToYuanInput, formatCentsAsYuan, yuanInputToCents } from '@/utils/money'

describe('用户端元与API整数分转换', () => {
  it.each([
    ['30000', 3_000_000],
    ['28.5', 2_850],
    ['28.50', 2_850],
    ['', null],
  ])('输入 %s 转换为 %s 分', (input, expected) => {
    expect(yuanInputToCents(input)).toBe(expected)
  })

  it('从整数分恢复时不放大或缩小100倍', () => {
    expect(centsToYuanInput(3_000_000)).toBe('30000')
    expect(centsToYuanInput(2_850)).toBe('28.5')
    expect(centsToYuanInput(null)).toBe('')
    expect(formatCentsAsYuan(4_500_000)).toBe('45,000')
  })

  it.each(['-1', '1.001', 'abc', '100000000.01'])('拒绝非法或超范围金额 %s', (input) => {
    expect(() => yuanInputToCents(input)).toThrow()
  })
})
