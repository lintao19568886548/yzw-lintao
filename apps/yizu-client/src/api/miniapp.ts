import { requestApi } from './client'
import { localDemoApi } from './local-demo'
import { localDemoMode } from '@/config/runtime'
import type {
  DevSessionRequest, InterpretDemandRequest, MatchRequest, MiniappApi, SmsSendRequest,
  SmsSendResponse, SmsVerifyRequest, SubmitLeadRequest, WechatLoginRequest,
} from '@/types/api'
import type { DemandInterpretation, DevSessionResponse, LeadRecord, MatchResponse, MetadataOptions } from '@/types/domain'

const bffApi: MiniappApi = {
  createDevSession(request: DevSessionRequest) {
    return requestApi<DevSessionResponse>('/api/miniapp/v1/auth/dev-session', 'POST', request)
  },
  sendSmsCode(request: SmsSendRequest) {
    return requestApi<SmsSendResponse>('/api/miniapp/v1/auth/sms/send', 'POST', request)
  },
  verifySmsCode(request: SmsVerifyRequest) {
    return requestApi<DevSessionResponse>('/api/miniapp/v1/auth/sms/verify', 'POST', request)
  },
  exchangeWechatCode(request: WechatLoginRequest) {
    return requestApi<DevSessionResponse>('/api/miniapp/v1/auth/wechat', 'POST', request)
  },
  interpretDemand(request: InterpretDemandRequest) {
    return requestApi<DemandInterpretation>('/api/miniapp/v1/demands/interpret', 'POST', request, 35_000)
  },
  createMatches(request: MatchRequest) {
    return requestApi<MatchResponse>('/api/miniapp/v1/matches', 'POST', request)
  },
  submitLead(request: SubmitLeadRequest) {
    return requestApi<LeadRecord>('/api/miniapp/v1/leads', 'POST', request)
  },
  loadMetadata() {
    return requestApi<MetadataOptions>('/api/miniapp/v1/metadata/options', 'GET')
  },
}

export const miniappApi: MiniappApi = localDemoMode ? localDemoApi : bffApi
export const miniappDataMode = localDemoMode ? 'local-demo' : 'rust-bff'
