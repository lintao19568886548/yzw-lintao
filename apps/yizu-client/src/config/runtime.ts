export type YizuClientMode = 'demo' | 'test' | 'production'

export interface RuntimeEnvironment {
  mode?: string | undefined
  demoEnabled?: string | undefined
  apiBaseUrl?: string | undefined
  publicH5BaseUrl?: string | undefined
}

export function resolveClientMode(environment: RuntimeEnvironment): YizuClientMode {
  const explicit = environment.mode?.trim().toLowerCase()
  if (explicit) {
    if (explicit === 'demo' || explicit === 'test' || explicit === 'production') return explicit
    throw new Error('VITE_YIZU_MODE 只能是 demo、test 或 production')
  }
  // An explicit demo flag must also work in `uni build` output. Tying it to
  // import.meta.env.DEV made the WeChat build silently disable login.
  return environment.demoEnabled === 'true' ? 'demo' : 'test'
}

export function isLocalDemoMode(mode: YizuClientMode): boolean {
  return mode === 'demo'
}

export function apiBaseUrl(envValue: string | undefined, mode: YizuClientMode): string {
  if (mode === 'demo') return ''
  const value = envValue?.trim()
  if (!value) {
    if (mode === 'test') return 'http://127.0.0.1:8080'
    throw new Error('production 模式必须显式配置 VITE_YIZU_API_BASE_URL')
  }
  if (!/^https?:\/\//u.test(value)) throw new Error('VITE_YIZU_API_BASE_URL 必须是 http(s) 地址')
  if (mode === 'production' && !/^https:\/\//u.test(value)) {
    throw new Error('production 模式的 VITE_YIZU_API_BASE_URL 必须使用 HTTPS')
  }
  return value.replace(/\/$/u, '')
}

export function publicH5BaseUrl(envValue: string | undefined): string {
  const value = envValue?.trim() || 'https://yizuw.cn'
  if (!/^https:\/\//u.test(value)) throw new Error('VITE_YIZU_PUBLIC_H5_BASE_URL 必须使用 HTTPS')
  return value.replace(/\/$/u, '')
}

export const runtimeMode = resolveClientMode({
  mode: import.meta.env.VITE_YIZU_MODE,
  demoEnabled: import.meta.env.VITE_YIZU_DEMO_MODE,
})

export const localDemoMode = isLocalDemoMode(runtimeMode)

export const runtimeConfig = Object.freeze({
  mode: runtimeMode,
  apiBaseUrl: apiBaseUrl(import.meta.env.VITE_YIZU_API_BASE_URL, runtimeMode),
  publicH5BaseUrl: publicH5BaseUrl(import.meta.env.VITE_YIZU_PUBLIC_H5_BASE_URL),
})
