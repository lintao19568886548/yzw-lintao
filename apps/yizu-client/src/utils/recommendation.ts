import type { MatchResult, SpaceType } from '@/types/domain'

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
}

export function mapMatchToCard(result: MatchResult): ListingCardView {
  const listing = result.listing
  return {
    id: listing.listing_id,
    title: listing.listing_name,
    subtitle: `${listing.town} · ${SPACE_LABELS[listing.space_type]} · ${listing.available_area_sqm}㎡`,
    rent: `¥${(listing.rent_cents_per_sqm_month / 100).toFixed(2)}/㎡/月 · 月租约¥${Math.round(listing.monthly_rent_cents / 100).toLocaleString('zh-CN')}`,
    score: result.overall_score.toFixed(1),
    verification: listing.verification_level.toUpperCase(),
    source: listing.source_label,
    reasons: result.recommendation_reasons.slice(0, 3),
    warnings: [...result.unmet_conditions, ...result.data_gaps],
  }
}
