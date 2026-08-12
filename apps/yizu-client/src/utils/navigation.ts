type TabPath = '/pages/home/index' | '/pages/demands/index' | '/pages/profile/index'

let pendingTabPath: TabPath | null = null

function finishNavigation(): void {
  pendingTabPath = null
}

function showNavigationFailure(): void {
  uni.showToast({
    title: '页面切换失败，请重试',
    icon: 'none',
    success: () => undefined,
    fail: () => undefined,
  })
}

function relaunchTab(path: TabPath): void {
  uni.reLaunch({
    url: path,
    success: finishNavigation,
    fail: () => {
      finishNavigation()
      showNavigationFailure()
    },
  })
}

export function switchTabSafely(path: TabPath): void {
  if (pendingTabPath) return
  pendingTabPath = path

  try {
    uni.switchTab({
      url: path,
      success: finishNavigation,
      fail: (result) => {
        const message = String((result as { errMsg?: unknown }).errMsg ?? '')
        if (/timeout/iu.test(message)) {
          relaunchTab(path)
          return
        }
        finishNavigation()
        showNavigationFailure()
      },
    })
  } catch {
    relaunchTab(path)
  }
}

export function goHome(): void {
  switchTabSafely('/pages/home/index')
}

export function goMyDemands(): void {
  switchTabSafely('/pages/demands/index')
}

export function goProfile(): void {
  switchTabSafely('/pages/profile/index')
}
