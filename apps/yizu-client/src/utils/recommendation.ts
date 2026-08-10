import type { MatchResult, SpaceType } from '@/types/domain'
import { formatCentsAsYuan } from '@/utils/money'

const SPACE_LABELS: Record<SpaceType, string> = {
  factory: '厂房',
  warehouse: '仓库',
  office: '写字楼',
}

export interface ListingCardView {
  id: string
  title: string
  subtitle: string
  rent: string
  score: string
  verification: string
  source: string
  reasons: string[]
  warnings: string[]
  satisfiedHard: string[]
  blockedHard: string[]
  unmetPreferences: string[]
  submissionBlocked: boolean
}

export function mapMatchToCard(result: MatchResult): ListingCardView {
  const listing = result.listing
  return {
    id: listing.listing_id,
    title: listing.listing_name,
    subtitle: `${listing.town} · ${SPACE_LABELS[listing.space_type]} · ${listing.available_area_sqm}㎡`,
    rent: `¥${formatCentsAsYuan(listing.rent_cents_per_sqm_month)}/平方米/月 · 月租约¥${formatCentsAsYuan(listing.monthly_rent_cents)}元`,
    score: result.overall_score.toFixed(1),
    verification: listing.verification_level.toUpperCase(),
    source: listing.source_label,
    reasons: result.recommendation_reasons.slice(0, 3),
    warnings: [...result.unmet_conditions, ...result.data_gaps],
    satisfiedHard: result.satisfied_hard_constraints.map((item) => item.detail),
    blockedHard: [...result.unmet_hard_constraints, ...result.unverified_hard_constraints].map((item) => item.detail),
    unmetPreferences: result.unmet_preferences.map((item) => item.detail),
    submissionBlocked: result.unmet_hard_constraints.length > 0 || result.unverified_hard_constraints.length > 0,
  }
}
