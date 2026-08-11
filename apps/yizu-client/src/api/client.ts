import { runtimeConfig } from '@/config/runtime'
import type { ApiResponse } from '@/types/domain'
import { getOrCreateDeviceId } from '@/utils/device'

export class ApiError extends Error {
  readonly code: string
  readonly request_id: string

  constructor(code: string, message: string, requestId = '') {
    super(message)
    this.name = 'ApiError'
    this.code = code
    this.request_id = requestId
  }
}

export function mapApiError(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.code === 'SESSION_EXPIRED' || error.code === 'UNAUTHENTICATED') return '登录已失效，请重新登录'
    if (error.code === 'REQUEST_TIMEOUT') return '请求超时，请稍后重试'
    return error.message
  }
  if (error instanceof Error) return error.message
  return '网络请求失败，请检查连接后重试'
}

export function isSessionError(error: unknown): boolean {
  return error instanceof ApiError && (error.code === 'SESSION_EXPIRED' || error.code === 'UNAUTHENTICATED')
}

function redirectExpiredSession(code: string): void {
  if (code !== 'SESSION_EXPIRED' && code !== 'UNAUTHENTICATED') return
  uni.reLaunch({ url: '/pages/login/index' })
}

export async function requestApi<T>(
  path: string,
  method: 'GET' | 'POST',
  data?: object,
  timeout = 15_000,
): Promise<T> {
  if (!runtimeConfig.apiBaseUrl) throw new ApiError('DEMO_NETWORK_BLOCKED', '演示模式禁止访问远程 API')
  const baseUrl = runtimeConfig.apiBaseUrl
  return await new Promise<T>((resolve, reject) => {
    const options: UniNamespace.RequestOptions = {
      url: `${baseUrl}${path}`,
      method,
      timeout,
      header: {
        'content-type': 'application/json',
        'x-yizu-device-id': getOrCreateDeviceId(),
      },
      success(response: UniNamespace.RequestSuccessCallbackResult) {
        const envelope = response.data as ApiResponse<T>
        if (response.statusCode < 200 || response.statusCode >= 300) {
          reject(new ApiError('HTTP_ERROR', `服务暂时不可用（${response.statusCode}）`))
          return
        }
        if (!envelope || typeof envelope !== 'object' || typeof envelope.code !== 'string') {
          reject(new ApiError('INVALID_RESPONSE', '服务返回格式无效'))
          return
        }
        if (envelope.code !== 'OK' || envelope.data === null) {
          redirectExpiredSession(envelope.code)
          reject(new ApiError(envelope.code, envelope.message, envelope.request_id))
          return
        }
        resolve(envelope.data)
      },
      fail(error: UniNamespace.GeneralCallbackResult) {
        const timedOut = error.errMsg.toLowerCase().includes('timeout')
        reject(new ApiError(timedOut ? 'REQUEST_TIMEOUT' : 'NETWORK_ERROR', timedOut ? '请求超时，请重试' : '网络请求失败'))
      },
    }
    if (data !== undefined) options.data = data as Record<string, unknown>
    uni.request(options)
  })
}
