export interface AuthGuardState {
  is_authenticated: boolean
  clear?: () => void
}

export type NavigateToLogin = () => void

export function requireAuth(state: AuthGuardState, navigate: NavigateToLogin = () => {
  uni.reLaunch({ url: '/pages/login/index' })
}): boolean {
  if (state.is_authenticated) return true
  state.clear?.()
  navigate()
  return false
}
