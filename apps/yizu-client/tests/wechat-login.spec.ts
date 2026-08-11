import { createWechatSession, requestWechatLoginCode } from '@/api/wechat-login'

describe('微信临时登录凭证客户端边界', () => {
  it('只把 wx.login 的临时 code 交给 Rust BFF', async () => {
    const exchange = vi.fn().mockResolvedValue({
      session_token: 'synthetic-session',
      refresh_token: 'synthetic-refresh',
      masked_phone: '微信用户',
      expires_at_epoch_seconds: 2_000_000_000,
      local_demo: false,
    })
    await createWechatSession(true, {
      requestCode: vi.fn().mockResolvedValue('synthetic-one-time-code'),
      deviceId: () => 'device_test_abcdefghijkl',
      exchange,
    })
    expect(exchange).toHaveBeenCalledWith({
      code: 'synthetic-one-time-code',
      device_id: 'device_test_abcdefghijkl',
      agreements_accepted: true,
    })
    expect(exchange.mock.calls[0]?.[0]).not.toHaveProperty('openid')
    expect(exchange.mock.calls[0]?.[0]).not.toHaveProperty('app_secret')
  })

  it('正确读取 wx.login 返回的 code 且失败不静默', async () => {
    Object.assign(globalThis, {
      uni: {
        login(options: UniNamespace.LoginOptions) {
          options.success?.({ code: 'synthetic-wx-code', authResult: '', errMsg: 'login:ok' })
        },
      },
    })
    await expect(requestWechatLoginCode()).resolves.toBe('synthetic-wx-code')

    Object.assign(globalThis, {
      uni: {
        login(options: UniNamespace.LoginOptions) {
          options.fail?.({ errMsg: 'login:fail' })
        },
      },
    })
    await expect(requestWechatLoginCode()).rejects.toThrow(/微信登录凭证获取失败/u)
  })

  it('未同意协议时不请求 wx.login', async () => {
    const requestCode = vi.fn()
    await expect(createWechatSession(false, {
      requestCode,
      deviceId: () => 'device_test_abcdefghijkl',
      exchange: vi.fn(),
    })).rejects.toThrow(/用户协议/u)
    expect(requestCode).not.toHaveBeenCalled()
  })
})
