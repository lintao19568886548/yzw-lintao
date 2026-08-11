import { miniappApi } from '@/api/miniapp'
import type { WechatLoginRequest } from '@/types/api'
import type { DevSessionResponse } from '@/types/domain'
import { getOrCreateDeviceId } from '@/utils/device'

export function requestWechatLoginCode(): Promise<string> {
  return new Promise((resolve, reject) => {
    uni.login({
      provider: 'weixin',
      success(result) {
        const code = typeof result.code === 'string' ? result.code.trim() : ''
        if (!code) {
          reject(new Error('微信登录凭证获取失败，请重试'))
          return
        }
        resolve(code)
      },
      fail() {
        reject(new Error('微信登录凭证获取失败，请重试'))
      },
    })
  })
}

export interface WechatLoginDependencies {
  requestCode(): Promise<string>
  deviceId(): string
  exchange(request: WechatLoginRequest): Promise<DevSessionResponse>
}

export async function createWechatSession(
  agreementsAccepted: boolean,
  dependencies: WechatLoginDependencies = {
    requestCode: requestWechatLoginCode,
    deviceId: getOrCreateDeviceId,
    exchange: (request) => miniappApi.exchangeWechatCode(request),
  },
): Promise<DevSessionResponse> {
  if (!agreementsAccepted) throw new Error('请阅读并同意《用户协议》与《隐私政策》')
  const code = await dependencies.requestCode()
  return dependencies.exchange({
    code,
    device_id: dependencies.deviceId(),
    agreements_accepted: true,
  })
}
