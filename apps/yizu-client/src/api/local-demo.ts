import { ApiError } from '@/api/client'
import { DONGGUAN_TOWN_FALLBACK } from '@/config/dongguan'
import { DEMO_LISTINGS } from '@/fixtures/demo-listings'
import type { MiniappApi } from '@/types/api'
import type {
  ConstraintAssessment, ConstraintKey, ConstraintLevel, DemandDraft, LeadRecord,
  ListingDetail, MatchDimensionScore, MatchResponse, MatchResult, RentUnit, SpaceType,
} from '@/types/domain'
import { isValidPhone } from '@/utils/validation'

const SPACE_TYPES: Array<[SpaceType, string[]]> = [
  ['factory', ['厂房', '厂区', '车间']],
  ['warehouse', ['仓库', '仓储']],
  ['office', ['写字楼', '办公室', '办公']],
]
const CLAUSE_BREAKS = /[，。；;！？!?]|同时|另外|并且/u

function cloneDemand(demand: DemandDraft): DemandDraft {
  return JSON.parse(JSON.stringify(demand)) as DemandDraft
}

function wait(milliseconds = 180): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds))
}

function upsertPriority(demand: DemandDraft, key: ConstraintKey, level: ConstraintLevel): void {
  const existing = demand.constraint_priorities.find((priority) => priority.key === key)
  if (existing) existing.level = level
  else demand.constraint_priorities.push({ key, level })
}

function clauseAround(text: string, keywordIndex: number): string {
  const before = text.slice(0, keywordIndex)
  const after = text.slice(keywordIndex)
  const startParts = before.split(CLAUSE_BREAKS)
  const endParts = after.split(CLAUSE_BREAKS)
  return `${startParts.at(-1) ?? ''}${endParts[0] ?? ''}`
}

export function classifyConstraintNear(text: string, keywords: string[]): ConstraintLevel {
  const normalized = text.toLowerCase()
  const positions = keywords
    .map((keyword) => normalized.indexOf(keyword.toLowerCase()))
    .filter((position) => position >= 0)
  if (!positions.length) return 'preference'
  const clause = clauseAround(normalized, Math.min(...positions))
  if (/(最好|希望|优先|尽量)/u.test(clause)) return 'preference'
  if (/(必须|需要|要求|务必)/u.test(clause)) return 'hard'
  return 'preference'
}

function parseArea(text: string): [number, number] | null {
  const range = text.match(/(\d+(?:\.\d+)?)\s*(?:-|—|～|至|到)\s*(\d+(?:\.\d+)?)\s*(?:平方米|平米|㎡|平)/u)
  if (range?.[1] && range[2]) return [Math.round(Number(range[1])), Math.round(Number(range[2]))]
  const single = text.match(/(\d+(?:\.\d+)?)\s*(?:平方米|平米|㎡|平)/u)
  if (!single?.[1]) return null
  const value = Number(single[1])
  return [Math.max(1, Math.round(value * 0.9)), Math.round(value * 1.1)]
}

function parseBudget(text: string): { cents: number; unit: RentUnit } | null {
  const perSquare = text.match(/(?:预算|租金|单价)[^，。；]{0,12}?(\d+(?:\.\d+)?)\s*元\s*(?:\/|每)\s*(?:平方米|平米|㎡|平)/u)
  if (perSquare?.[1]) return { cents: Math.round(Number(perSquare[1]) * 100), unit: 'yuan_per_square_metre_month' }
  const monthly = text.match(/(?:预算|月租)[^，。；]{0,14}?(\d+(?:\.\d+)?)\s*(万)?\s*元?/u)
  if (!monthly?.[1]) return null
  return { cents: Math.round(Number(monthly[1]) * (monthly[2] ? 10_000 : 1) * 100), unit: 'yuan_per_month' }
}

function parseNumberBefore(text: string, suffixes: string[]): number | null {
  for (const suffix of suffixes) {
    const match = text.match(new RegExp(`(\\d+(?:\\.\\d+)?)\\s*${suffix}`, 'iu'))
    if (match?.[1]) return Number(match[1])
  }
  return null
}

export function interpretLocalDemand(source: DemandDraft): DemandDraft {
  const demand = cloneDemand(source)
  const text = demand.raw_text.trim()
  const normalized = text.toLowerCase()
  if (!text) throw new ApiError('VALIDATION_ERROR', '请描述你的找房需求')

  if (!demand.constraints.space_type) {
    demand.constraints.space_type = SPACE_TYPES.find(([, words]) => words.some((word) => normalized.includes(word)))?.[0] ?? null
  }
  for (const town of DONGGUAN_TOWN_FALLBACK) {
    if (normalized.includes(town) && !demand.constraints.target_towns.includes(town)) demand.constraints.target_towns.push(town)
  }
  const area = parseArea(normalized)
  if (area) {
    demand.constraints.area_min_sqm ??= area[0]
    demand.constraints.area_max_sqm ??= area[1]
  }
  const budget = parseBudget(normalized)
  if (budget) {
    demand.constraints.rent_max_cents ??= budget.cents
    demand.constraints.rent_unit ??= budget.unit
    upsertPriority(demand, 'budget', classifyConstraintNear(normalized, ['预算', '月租', '租金']))
  }
  if (normalized.includes('货梯')) {
    demand.constraints.needs_freight_elevator = true
    upsertPriority(demand, 'freight_elevator', classifyConstraintNear(normalized, ['货梯']))
    const tons = parseNumberBefore(normalized, ['吨货梯', '吨电梯'])
    if (tons !== null) {
      demand.constraints.elevator_min_tons ??= tons
      upsertPriority(demand, 'elevator_capacity', classifyConstraintNear(normalized, ['货梯', '电梯']))
    }
  }
  const power = parseNumberBefore(normalized, ['kva', '千伏安', 'kw', '千瓦'])
  if (power !== null) {
    demand.constraints.power_capacity_kva ??= Math.round(power)
    upsertPriority(demand, 'power_capacity', classifyConstraintNear(normalized, ['用电', '变压器', 'kva', '千伏安']))
  }
  if (!demand.constraints.industry_or_use) {
    demand.constraints.industry_or_use = ['电子制造', '五金加工', '食品生产', '电商仓储', '物流仓储', '研发办公']
      .find((value) => normalized.includes(value)) ?? null
  }
  if (demand.constraints.clear_height_m === null) {
    const height = normalized.match(/(?:层高|净高)\s*(\d+(?:\.\d+)?)\s*米?/u)
      ?? normalized.match(/(\d+(?:\.\d+)?)\s*米\s*(?:层高|净高)/u)
    if (height?.[1]) demand.constraints.clear_height_m = Number(height[1])
  }
  if (demand.constraints.floor_load_kg_sqm === null) {
    const load = normalized.match(/(?:承重|荷载)[^，。；]{0,8}?(\d+)\s*(?:kg|公斤)/u)
      ?? normalized.match(/(\d+)\s*(?:kg|公斤)[^，。；]{0,8}?(?:承重|荷载)/u)
    if (load?.[1]) demand.constraints.floor_load_kg_sqm = Number(load[1])
  }
  const fire = normalized.match(/([甲乙丙丁戊]类)消防/u)
  if (fire?.[1]) {
    demand.constraints.fire_requirement ??= `${fire[1]}消防`
    upsertPriority(demand, 'fire_safety', classifyConstraintNear(normalized, ['消防']))
  }
  if (/货车|大车|物流/u.test(normalized)) {
    demand.constraints.logistics_requirement ??= '支持大型货车通行'
    upsertPriority(demand, 'truck_access', classifyConstraintNear(normalized, ['货车', '大车', '物流']))
  }
  if (/装卸|卸货|月台/u.test(normalized)) {
    demand.constraints.loading_requirement ??= '需要装卸区或月台'
    upsertPriority(demand, 'loading_dock', classifyConstraintNear(normalized, ['装卸', '卸货', '月台']))
  }
  if (/一个月内|1个月内/u.test(normalized)) demand.constraints.move_in_time ??= 'within_30_days'
  else if (/三个月内|3个月内/u.test(normalized)) demand.constraints.move_in_time ??= 'within_90_days'
  else if (/立即|随时/u.test(normalized)) demand.constraints.move_in_time ??= 'immediate'
  if (demand.constraints.move_in_time) upsertPriority(demand, 'move_in', classifyConstraintNear(normalized, ['入驻', '进场']))

  demand.missing_fields = []
  if (!demand.constraints.space_type) demand.missing_fields.push('space_type')
  if (!demand.constraints.target_towns.length) demand.missing_fields.push('target_towns')
  if (demand.constraints.area_min_sqm === null || demand.constraints.area_max_sqm === null) demand.missing_fields.push('area_range')
  const filled = Object.values(demand.constraints).filter((value) => value !== null && (!Array.isArray(value) || value.length)).length
  demand.ai_confidence = Math.min(0.95, 0.42 + filled * 0.045)
  demand.constraint_priorities.sort((left, right) => left.key.localeCompare(right.key))
  return demand
}

function levelFor(demand: DemandDraft, key: ConstraintKey): ConstraintLevel {
  const explicit = demand.constraint_priorities.find((priority) => priority.key === key)?.level
  if (explicit) return explicit
  return key === 'freight_elevator' && demand.constraints.needs_freight_elevator ? 'hard' : 'preference'
}

interface CheckResult {
  key: ConstraintKey
  label: string
  applicable: boolean
  verified: boolean
  met: boolean
  detail: string
}

function listingChecks(demand: DemandDraft, listing: ListingDetail): CheckResult[] {
  const c = demand.constraints
  const budgetValue = c.rent_unit === 'yuan_per_square_metre_month' ? listing.rent_cents_per_sqm_month : listing.monthly_rent_cents
  const budgetLabel = c.rent_unit === 'yuan_per_square_metre_month' ? '单价预算' : '月租预算'
  return [
    { key: 'budget', label: budgetLabel, applicable: c.rent_max_cents !== null, verified: true, met: c.rent_max_cents === null || budgetValue <= c.rent_max_cents, detail: `房源${budgetLabel}${c.rent_max_cents === null || budgetValue <= c.rent_max_cents ? '符合' : '超出'}` },
    { key: 'freight_elevator', label: '货梯', applicable: c.needs_freight_elevator === true, verified: listing.has_freight_elevator !== null, met: listing.has_freight_elevator === true, detail: listing.has_freight_elevator === true ? '已核验配备货梯' : '未核验或未配备货梯' },
    { key: 'elevator_capacity', label: '电梯吨位', applicable: c.elevator_min_tons !== null, verified: listing.elevator_capacity_tons !== null, met: c.elevator_min_tons === null || (listing.elevator_capacity_tons ?? 0) >= c.elevator_min_tons, detail: listing.elevator_capacity_tons === null ? '电梯吨位待核验' : `货梯 ${listing.elevator_capacity_tons} 吨` },
    { key: 'power_capacity', label: '用电容量', applicable: c.power_capacity_kva !== null, verified: listing.power_capacity_kva !== null, met: c.power_capacity_kva === null || (listing.power_capacity_kva ?? 0) >= c.power_capacity_kva, detail: listing.power_capacity_kva === null ? '用电容量待核验' : `可用容量 ${listing.power_capacity_kva}kVA` },
    { key: 'fire_safety', label: '消防', applicable: Boolean(c.fire_requirement), verified: listing.fire_rating !== null, met: !c.fire_requirement || listing.fire_rating?.includes(c.fire_requirement.slice(0, 2)) === true, detail: listing.fire_rating ?? '消防等级待核验' },
    { key: 'truck_access', label: '货车通行', applicable: Boolean(c.logistics_requirement), verified: listing.truck_access !== null, met: listing.truck_access === true, detail: listing.truck_access ? '支持大型货车通行' : '货车通行待确认或不满足' },
    { key: 'loading_dock', label: '装卸条件', applicable: Boolean(c.loading_requirement), verified: listing.loading_dock !== null, met: listing.loading_dock === true, detail: listing.loading_dock ? '具备装卸区/月台' : '装卸条件待确认或不满足' },
  ]
}

function assessment(check: CheckResult): ConstraintAssessment {
  return { key: check.key, label: check.label, detail: check.detail }
}

function ratioScore(value: boolean | null, neutral = 72): number {
  return value === true ? 100 : value === false ? 45 : neutral
}

function scoreListing(demand: DemandDraft, listing: ListingDetail, relaxed: boolean): MatchResult {
  const checks = listingChecks(demand, listing).filter((check) => check.applicable && levelFor(demand, check.key) !== 'unspecified')
  const hard = checks.filter((check) => levelFor(demand, check.key) === 'hard')
  const preferences = checks.filter((check) => levelFor(demand, check.key) === 'preference')
  const c = demand.constraints
  const targetArea = ((c.area_min_sqm ?? listing.available_area_sqm) + (c.area_max_sqm ?? listing.available_area_sqm)) / 2
  const areaDelta = Math.abs(listing.available_area_sqm - targetArea) / Math.max(1, targetArea)
  const budgetValue = c.rent_unit === 'yuan_per_square_metre_month' ? listing.rent_cents_per_sqm_month : listing.monthly_rent_cents
  const budgetScore = c.rent_max_cents === null ? 82 : Math.max(25, Math.min(100, 100 - Math.max(0, budgetValue - c.rent_max_cents) / c.rent_max_cents * 100))
  const scores: MatchDimensionScore[] = [
    { dimension: 'location', score: c.target_towns.includes(listing.town) ? 100 : 55, weight: 0.2, reason: c.target_towns.includes(listing.town) ? '位于目标镇街' : '不在首选镇街' },
    { dimension: 'space', score: Math.max(55, 100 - areaDelta * 100), weight: 0.2, reason: relaxed ? '面积按 ±20% 放宽后匹配' : '空间类型与面积匹配' },
    { dimension: 'cost', score: budgetScore, weight: 0.2, reason: budgetScore >= 90 ? '租金处于预算内' : '租金接近或高于预算' },
    { dimension: 'production', score: (ratioScore(listing.has_freight_elevator) + (listing.power_capacity_kva ? 100 : 65)) / 2, weight: 0.15, reason: '综合货梯与用电条件' },
    { dimension: 'logistics', score: (ratioScore(listing.truck_access) + ratioScore(listing.loading_dock)) / 2, weight: 0.1, reason: '综合货车与装卸能力' },
    { dimension: 'compliance', score: listing.fire_rating ? 96 : 58, weight: 0.1, reason: listing.fire_rating ? '消防信息已核验' : '消防信息待核验' },
    { dimension: 'move_in', score: 88, weight: 0.05, reason: `可用时间 ${listing.available_from}` },
  ]
  const overall = scores.reduce((total, item) => total + item.score * item.weight, 0)
  return {
    listing,
    overall_score: Number(overall.toFixed(1)),
    dimension_scores: scores,
    recommendation_reasons: [
      `${listing.town}${listing.space_type === 'factory' ? '厂房' : listing.space_type === 'warehouse' ? '仓库' : '办公'}匹配`,
      `${listing.available_area_sqm}㎡可租，认证等级 ${listing.verification_level.toUpperCase()}`,
      listing.is_self_operated ? '宜租网自营演示房源' : '合作业主核验房源',
    ],
    unmet_conditions: preferences.filter((check) => !check.met).map((check) => check.detail),
    satisfied_hard_constraints: hard.filter((check) => check.verified && check.met).map(assessment),
    unmet_hard_constraints: hard.filter((check) => check.verified && !check.met).map(assessment),
    unverified_hard_constraints: hard.filter((check) => !check.verified).map(assessment),
    unmet_preferences: preferences.filter((check) => !check.met).map(assessment),
    area_relaxed: relaxed,
    data_gaps: listing.data_gaps,
  }
}

export function createLocalMatches(demand: DemandDraft): MatchResponse {
  const eligible = DEMO_LISTINGS.filter((listing) => listing.rental_status === 'available'
    && (listing.verification_level === 'l2' || listing.verification_level === 'l3')
    && (!demand.constraints.space_type || listing.space_type === demand.constraints.space_type)
    && (!demand.constraints.target_towns.length || demand.constraints.target_towns.includes(listing.town)))
  const inArea = (listing: ListingDetail, relaxed: boolean): boolean => {
    const min = demand.constraints.area_min_sqm
    const max = demand.constraints.area_max_sqm
    if (min === null || max === null) return true
    return listing.available_area_sqm >= min * (relaxed ? 0.8 : 1) && listing.available_area_sqm <= max * (relaxed ? 1.2 : 1)
  }
  let candidates = eligible.filter((listing) => inArea(listing, false))
  const relaxed = candidates.length === 0 && eligible.some((listing) => inArea(listing, true))
  if (relaxed) candidates = eligible.filter((listing) => inArea(listing, true))
  const matches = candidates.map((listing) => scoreListing(demand, listing, relaxed))
    .sort((left, right) => right.overall_score - left.overall_score
      || right.listing.verification_level.localeCompare(left.listing.verification_level)
      || right.listing.updated_at.localeCompare(left.listing.updated_at))
    .slice(0, 10)
  return {
    matches,
    used_area_relaxation: relaxed,
    next_step_suggestion: matches.length ? null : '可返回需求确认页调整镇街、面积或硬条件后再次匹配。',
    demo_data: true,
  }
}

function stableNumber(prefix: string, key: string): string {
  let hash = 2166136261
  for (const character of key) hash = Math.imul(hash ^ character.charCodeAt(0), 16777619)
  return `${prefix}${String(Math.abs(hash >>> 0)).padStart(10, '0').slice(0, 10)}`
}

export const localDemoApi: MiniappApi = {
  async createDevSession(request) {
    await wait()
    if (!isValidPhone(request.phone)) throw new ApiError('VALIDATION_ERROR', '请输入有效的11位中国大陆手机号')
    if (!request.contact_confirmed) throw new ApiError('VALIDATION_ERROR', '请确认联系方式')
    return {
      session_token: `local_demo_${Date.now()}`,
      masked_phone: `${request.phone.slice(0, 3)}****${request.phone.slice(-4)}`,
      expires_at_epoch_seconds: Math.floor(Date.now() / 1000) + 8 * 60 * 60,
      local_demo: true,
    }
  },
  async sendSmsCode() {
    throw new ApiError('DEMO_EXTERNAL_CALL_BLOCKED', '演示模式不会发送真实短信')
  },
  async verifySmsCode() {
    throw new ApiError('DEMO_EXTERNAL_CALL_BLOCKED', '演示模式不使用短信验证码')
  },
  async exchangeWechatCode() {
    throw new ApiError('DEMO_EXTERNAL_CALL_BLOCKED', '演示模式不调用微信凭证交换')
  },
  async interpretDemand(request) {
    await wait(260)
    if (!request.session_token.startsWith('local_demo_')) throw new ApiError('SESSION_EXPIRED', '登录已失效')
    return { demand: interpretLocalDemand(request.draft), provider: 'local-browser', fallback_reason: null }
  },
  async createMatches(request) {
    await wait(240)
    if (!request.session_token.startsWith('local_demo_')) throw new ApiError('SESSION_EXPIRED', '登录已失效')
    return createLocalMatches(request.demand)
  },
  async submitLead(request) {
    await wait(320)
    if (!request.session_token.startsWith('local_demo_')) throw new ApiError('SESSION_EXPIRED', '登录已失效')
    if (!request.submission.idempotency_key) throw new ApiError('VALIDATION_ERROR', '缺少幂等键')
    const recalculated = createLocalMatches(request.submission.demand)
    const selected = request.submission.recommended_listing_ids.map((id) => recalculated.matches.find((match) => match.listing.listing_id === id))
    if (selected.some((match) => !match)) throw new ApiError('LISTING_NOT_ELIGIBLE', '意向房源已不可提交，请重新匹配')
    if (selected.some((match) => (match?.unmet_hard_constraints.length ?? 0) > 0 || (match?.unverified_hard_constraints.length ?? 0) > 0)) {
      throw new ApiError('HARD_CONSTRAINT_BLOCKED', '房源不满足或无法核验硬条件，请重新选择')
    }
    const key = request.submission.idempotency_key
    const lead: LeadRecord = {
      demand_number: stableNumber('D', key), lead_number: stableNumber('L', key),
      demand_snapshot: cloneDemand(request.submission.demand),
      recommended_listing_ids: [...request.submission.recommended_listing_ids],
      source_channel: 'miniapp_ai_demand', status: 'pending_assignment', sla_minutes: 15,
      created_at_epoch_seconds: Math.floor(Date.now() / 1000), temporary_storage: true,
    }
    return lead
  },
  async loadMetadata() {
    await wait(80)
    return {
      towns: [...DONGGUAN_TOWN_FALLBACK], space_types: ['factory', 'warehouse', 'office'],
      rent_units: ['yuan_per_month', 'yuan_per_square_metre_month'], verification_levels: ['l0', 'l1', 'l2', 'l3'],
      constraint_keys: ['budget', 'freight_elevator', 'elevator_capacity', 'power_capacity', 'fire_safety', 'truck_access', 'loading_dock', 'sublease', 'floor', 'move_in'],
      business_timezone: 'Asia/Shanghai', currency_storage_unit: 'cents', currency_display_unit: 'yuan', demo_data: true,
    }
  },
}
