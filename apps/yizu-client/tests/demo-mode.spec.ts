import { isLocalDemoMode } from '@/config/runtime'

describe('Mock 模式边界', () => {
  it('只有开发构建且显式开启才可用', () => {
    expect(isLocalDemoMode({ dev: true, enabled: true })).toBe(true)
    expect(isLocalDemoMode({ dev: false, enabled: true })).toBe(false)
    expect(isLocalDemoMode({ dev: true, enabled: false })).toBe(false)
  })
})
