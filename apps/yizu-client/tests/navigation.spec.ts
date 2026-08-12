import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

interface NavigationOptions {
  url: string
  success?: (result?: unknown) => void
  fail?: (result: { errMsg?: string }) => void
}

function installUni(overrides: {
  switchTab?: (options: NavigationOptions) => void
  reLaunch?: (options: NavigationOptions) => void
} = {}) {
  const switchTab = vi.fn(overrides.switchTab ?? ((options: NavigationOptions) => options.success?.()))
  const reLaunch = vi.fn(overrides.reLaunch ?? ((options: NavigationOptions) => options.success?.()))
  const showToast = vi.fn()
  Object.assign(globalThis, { uni: { switchTab, reLaunch, showToast } })
  return { switchTab, reLaunch, showToast }
}

describe('微信 tabBar 安全导航', () => {
  beforeEach(() => {
    vi.resetModules()
  })

  it('正常切换首页时注册成功和失败回调，不产生悬空 Promise', async () => {
    const runtime = installUni()
    const { goHome } = await import('@/utils/navigation')

    goHome()

    expect(runtime.switchTab).toHaveBeenCalledOnce()
    expect(runtime.switchTab).toHaveBeenCalledWith(expect.objectContaining({
      url: '/pages/home/index',
      success: expect.any(Function),
      fail: expect.any(Function),
    }))
    expect(runtime.reLaunch).not.toHaveBeenCalled()
  })

  it('switchTab 超时时使用 reLaunch 打开同一 tabBar 页', async () => {
    const runtime = installUni({
      switchTab: (options) => options.fail?.({ errMsg: 'switchTab:fail timeout' }),
    })
    const { goHome } = await import('@/utils/navigation')

    goHome()

    expect(runtime.reLaunch).toHaveBeenCalledWith(expect.objectContaining({
      url: '/pages/home/index',
      success: expect.any(Function),
      fail: expect.any(Function),
    }))
    expect(runtime.showToast).not.toHaveBeenCalled()
  })

  it('导航进行中忽略重复触发，完成后允许再次跳转', async () => {
    let activeNavigation: NavigationOptions | undefined
    const runtime = installUni({ switchTab: (options) => { activeNavigation = options } })
    const { goHome } = await import('@/utils/navigation')

    goHome()
    goHome()
    expect(runtime.switchTab).toHaveBeenCalledOnce()

    activeNavigation?.success?.()
    goHome()
    expect(runtime.switchTab).toHaveBeenCalledTimes(2)
  })

  it('非超时错误不反复导航，并给出可见提示', async () => {
    const runtime = installUni({
      switchTab: (options) => options.fail?.({ errMsg: 'switchTab:fail page is not a tabBar page' }),
    })
    const { goProfile } = await import('@/utils/navigation')

    goProfile()

    expect(runtime.reLaunch).not.toHaveBeenCalled()
    expect(runtime.showToast).toHaveBeenCalledWith(expect.objectContaining({
      title: '页面切换失败，请重试',
      icon: 'none',
      success: expect.any(Function),
      fail: expect.any(Function),
    }))
  })

  it('流程页面不再直接发起未处理的 switchTab', () => {
    const sourceRoot = fileURLToPath(new URL('../src/pages/', import.meta.url))
    const confirmPage = readFileSync(`${sourceRoot}/confirm/index.vue`, 'utf8')
    const resultsPage = readFileSync(`${sourceRoot}/results/index.vue`, 'utf8')

    expect(confirmPage).not.toContain('uni.switchTab(')
    expect(resultsPage).not.toContain('uni.switchTab(')
    expect(confirmPage).toContain('goHome()')
    expect(resultsPage).toContain('goHome()')
  })
})
