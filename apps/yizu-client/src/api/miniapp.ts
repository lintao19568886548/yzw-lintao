import { requestApi } from './client'
import type { InterpretDemandRequest, MatchRequest, MiniappApi, SubmitLeadRequest, DevSessionRequest } from '@/types/api'
import type { DemandInterpretation, DevSessionResponse, LeadRecord, MatchResponse, MetadataOptions } from '@/types/domain'

export const miniappApi: MiniappApi = {
  createDevSession(request: DevSessionRequest) {
    return requestApi<DevSessionResponse>('/api/miniapp/v1/auth/dev-session', 'POST', request)
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
