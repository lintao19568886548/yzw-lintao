describe('设备限流标识', () => {
  afterEach(() => {
    Reflect.deleteProperty(globalThis, 'uni')
    vi.restoreAllMocks()
    vi.resetModules()
  })

  it('优先复用本地已存在的合法标识', async () => {
    const setStorageSync = vi.fn()
    Object.assign(globalThis, {
      uni: {
        getStorageSync: () => 'wx_existing_device_1234',
        setStorageSync,
      },
    })
    const { getOrCreateDeviceId } = await import('@/utils/device')

    expect(getOrCreateDeviceId()).toBe('wx_existing_device_1234')
    expect(setStorageSync).not.toHaveBeenCalled()
  })

  it('存储读写失败时返回稳定内存值而不阻断登录', async () => {
    Object.assign(globalThis, {
      uni: {
        getStorageSync: () => { throw new Error('storage unavailable') },
        setStorageSync: () => { throw new Error('storage unavailable') },
      },
    })
    const { getOrCreateDeviceId } = await import('@/utils/device')

    const first = getOrCreateDeviceId()
    expect(first).toMatch(/^[a-z0-9_-]{16,80}$/u)
    expect(getOrCreateDeviceId()).toBe(first)
  })

  it('uni 尚未注入时也能安全生成可控默认值', async () => {
    const { getOrCreateDeviceId } = await import('@/utils/device')

    expect(() => getOrCreateDeviceId()).not.toThrow()
    expect(getOrCreateDeviceId()).toMatch(/^[a-z0-9_-]{16,80}$/u)
  })
})
