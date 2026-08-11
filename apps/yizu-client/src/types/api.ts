import type { DemandDraft, DemandInterpretation, DevSessionResponse, LeadRecord, MatchResponse, MetadataOptions } from './domain'

export interface DevSessionRequest {
  phone: string
  contact_confirmed: boolean
}

export interface SmsSendRequest {
  phone: string
  device_id: string
}

export interface SmsVerifyRequest {
  phone: string
  code: string
  device_id: string
  agreements_accepted: boolean
}

export interface WechatLoginRequest {
  code: string
  device_id: string
  agreements_accepted: boolean
}

export interface SmsSendResponse {
  expires_in_seconds: number
  retry_after_seconds: number
}

export interface InterpretDemandRequest {
  session_token: string
  draft: DemandDraft
}

export interface MatchRequest {
  session_token: string
  demand: DemandDraft
}

export interface SubmitLeadRequest {
  session_token: string
  submission: {
    demand: DemandDraft
    recommended_listing_ids: string[]
    source_channel: 'miniapp_ai_demand'
    idempotency_key: string
  }
}

export interface MiniappApi {
  createDevSession(request: DevSessionRequest): Promise<DevSessionResponse>
  sendSmsCode(request: SmsSendRequest): Promise<SmsSendResponse>
  verifySmsCode(request: SmsVerifyRequest): Promise<DevSessionResponse>
  exchangeWechatCode(request: WechatLoginRequest): Promise<DevSessionResponse>
  interpretDemand(request: InterpretDemandRequest): Promise<DemandInterpretation>
  createMatches(request: MatchRequest): Promise<MatchResponse>
  submitLead(request: SubmitLeadRequest): Promise<LeadRecord>
  loadMetadata(): Promise<MetadataOptions>
}
