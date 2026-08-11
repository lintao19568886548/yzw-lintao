import { apiBaseUrl, isLocalDemoMode, resolveClientMode } from '@/config/runtime'

describe('Mock 模式边界', () => {
  it('显式 demo 在微信 production build 中仍保持本地模式', () => {
    expect(resolveClientMode({ mode: 'demo' })).toBe('demo')
    expect(resolveClientMode({ demoEnabled: 'true' })).toBe('demo')
    expect(isLocalDemoMode('demo')).toBe(true)
    expect(isLocalDemoMode('test')).toBe(false)
  })

  it('未显式指定 production 时只会进入 test', () => {
    expect(resolveClientMode({})).toBe('test')
    expect(apiBaseUrl(undefined, 'test')).toBe('http://127.0.0.1:8080')
  })

  it('production 缺少 HTTPS BFF 地址时失败关闭', () => {
    expect(() => apiBaseUrl(undefined, 'production')).toThrow(/必须显式配置/u)
    expect(() => apiBaseUrl('http://api.example.com', 'production')).toThrow(/HTTPS/u)
  })
})
