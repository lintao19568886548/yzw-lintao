import { requireAuth } from '@/composables/useAuthGuard'

describe('登录守卫', () => {
  it('未登录时跳转登录页', () => {
    const navigate = vi.fn()
    expect(requireAuth({ is_authenticated: false }, navigate)).toBe(false)
    expect(navigate).toHaveBeenCalledOnce()
  })

  it('登录后直接放行', () => {
    const navigate = vi.fn()
    expect(requireAuth({ is_authenticated: true }, navigate)).toBe(true)
    expect(navigate).not.toHaveBeenCalled()
  })
})
