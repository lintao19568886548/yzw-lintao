import { ApiError, mapApiError } from '@/api/client'

describe('API 错误映射', () => {
  it('登录错误映射为可行动提示', () => {
    expect(mapApiError(new ApiError('SESSION_EXPIRED', 'expired'))).toBe('登录已失效，请重新登录')
  })

  it('未知失败不会暴露内部对象', () => {
    expect(mapApiError({ secret: 'x' })).toBe('网络请求失败，请检查连接后重试')
  })
})
