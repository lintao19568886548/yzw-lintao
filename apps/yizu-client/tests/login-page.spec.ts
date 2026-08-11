import {
  agreementFromCheckboxEvent,
  canSubmitLogin,
  completeDemoLogin,
  phoneFromInputEvent,
} from '@/pages/login/model'

describe('登录页状态与演示登录', () => {
  const phone = ['139', '0000', '0000'].join('')

  it('从微信 input 的 event.detail.value 写入手机号', () => {
    expect(phoneFromInputEvent({ detail: { value: phone } })).toBe(phone)
  })

  it('协议真实状态与微信 checkbox 事件一致', () => {
    expect(agreementFromCheckboxEvent({ detail: { value: ['accepted'] } })).toBe(true)
    expect(agreementFromCheckboxEvent({ detail: { value: [] } })).toBe(false)
  })

  it('唯一启用条件是合法手机号、同意协议且未提交', () => {
    expect(canSubmitLogin(phone, true, false)).toBe(true)
    expect(canSubmitLogin('123', true, false)).toBe(false)
    expect(canSubmitLogin(phone, false, false)).toBe(false)
    expect(canSubmitLogin(phone, true, true)).toBe(false)
  })

  it('演示登录保存会话后跳转 AI 找房首页', async () => {
    const saveSession = vi.fn()
    const navigateToAiHome = vi.fn()
    const session = {
      session_token: 'local_demo_test', masked_phone: '139****0000',
      expires_at_epoch_seconds: 2_000_000_000, local_demo: true,
    }
    await completeDemoLogin(phone, true, {
      createSession: vi.fn().mockResolvedValue(session), saveSession, navigateToAiHome,
    })
    expect(saveSession).toHaveBeenCalledWith(session)
    expect(navigateToAiHome).toHaveBeenCalledOnce()
  })

  it('未同意协议时不调用登录接口', async () => {
    const createSession = vi.fn()
    await expect(completeDemoLogin(phone, false, {
      createSession, saveSession: vi.fn(), navigateToAiHome: vi.fn(),
    })).rejects.toThrow(/用户协议/u)
    expect(createSession).not.toHaveBeenCalled()
  })

  it('登录失败向上抛出供页面显示，不静默吞掉', async () => {
    await expect(completeDemoLogin(phone, true, {
      createSession: vi.fn().mockRejectedValue(new Error('模拟登录失败')),
      saveSession: vi.fn(), navigateToAiHome: vi.fn(),
    })).rejects.toThrow('模拟登录失败')
  })
})
