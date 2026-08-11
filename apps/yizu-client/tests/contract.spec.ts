import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { emptyDemand } from '@/stores/demand'

const repositoryRoot = fileURLToPath(new URL('../../..', import.meta.url))

describe('跨语言HTTP契约', () => {
  it('OpenAPI、Rust和TypeScript使用相同三态枚举', () => {
    const openapi = readFileSync(`${repositoryRoot}/docs/phase1-ai-demand-flow.openapi.yaml`, 'utf8')
    const rust = readFileSync(`${repositoryRoot}/src/services/miniapp/types.rs`, 'utf8')
    const typescript = readFileSync(`${repositoryRoot}/apps/yizu-client/src/types/domain.ts`, 'utf8')
    expect(openapi).toContain('enum: [hard, preference, unspecified]')
    expect(rust).toMatch(/enum ConstraintLevel \{\s*Hard,\s*Preference,\s*Unspecified,/u)
    expect(typescript).toContain("'hard' | 'preference' | 'unspecified'")
  })

  it('uni-app POST发送裸请求对象而不是额外request包装层', async () => {
    vi.stubEnv('VITE_YIZU_MODE', 'test')
    vi.stubEnv('VITE_YIZU_API_BASE_URL', 'http://127.0.0.1:8080')
    vi.resetModules()
    const { miniappApi } = await import('@/api/miniapp')
    let sent: UniNamespace.RequestOptions | undefined
    Object.assign(globalThis, {
      uni: {
        getStorageSync: () => '',
        setStorageSync: vi.fn(),
        request(options: UniNamespace.RequestOptions) {
          sent = options
          options.success?.({
            statusCode: 200,
            data: {
              code: 'OK', message: 'ok', request_id: 'contract-test', errors: [],
              data: { matches: [], used_area_relaxation: false, next_step_suggestion: null, demo_data: true },
            },
            header: {}, cookies: [], errMsg: 'request:ok',
          })
        },
      },
    })
    const request = { session_token: 'contract-session', demand: emptyDemand() }
    await miniappApi.createMatches(request)
    expect(sent?.data).toEqual(request)
    expect(sent?.data).not.toHaveProperty('request')
    vi.unstubAllEnvs()
    vi.resetModules()
  })

  it('真实认证接口只指向 Rust BFF 契约', () => {
    const client = readFileSync(`${repositoryRoot}/apps/yizu-client/src/api/miniapp.ts`, 'utf8')
    expect(client).toContain("'/api/miniapp/v1/auth/sms/send'")
    expect(client).toContain("'/api/miniapp/v1/auth/sms/verify'")
    expect(client).toContain("'/api/miniapp/v1/auth/wechat'")
    expect(client).not.toMatch(/api\.weixin\.qq\.com|dashscope|shlianlu/iu)
  })
})
