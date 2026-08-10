export interface DemoModeFlags {
  dev: boolean
  enabled: boolean
}

export function isLocalDemoMode(flags: DemoModeFlags): boolean {
  return flags.dev && flags.enabled
}

export const localDemoMode = isLocalDemoMode({
  dev: import.meta.env.DEV,
  enabled: import.meta.env.VITE_YIZU_DEMO_MODE === 'true',
})

export function apiBaseUrl(envValue: string | undefined, dev: boolean): string {
  const value = envValue?.trim()
  if (value) {
    if (!/^https?:\/\//u.test(value)) {
      throw new Error('VITE_YIZU_API_BASE_URL 必须是 http(s) 地址')
    }
    return value.replace(/\/$/u, '')
  }
  if (dev) return 'http://127.0.0.1:8080'
  throw new Error('生产构建必须配置 VITE_YIZU_API_BASE_URL')
}
