import { mapMatchToCard } from '@/utils/recommendation'
import type { MatchResult } from '@/types/domain'

describe('推荐结果转换', () => {
  it('将服务端结果转换为卡片并保留三条理由', () => {
    const result = {
      listing: {
        listing_id: 'demo-1', listing_name: '虚构厂房', space_type: 'factory', town: '松山湖',
        available_area_sqm: 1500, rent_cents_per_sqm_month: 3000, monthly_rent_cents: 4500000,
        verification_level: 'l3', is_self_operated: true, source_label: '演示', updated_at: '2026-08-10',
        available_from: '2026-08-10', rental_status: 'available', power_capacity_kva: 500,
        has_freight_elevator: true, elevator_capacity_tons: 3, fire_rating: '丙类消防',
        truck_access: true, loading_dock: true, allows_sublease: false, floor_label: '独栋', data_gaps: [],
      },
      overall_score: 92.5,
      dimension_scores: [],
      recommendation_reasons: ['位置匹配', '面积匹配', '预算匹配', '第四条'],
      unmet_conditions: [], area_relaxed: false, data_gaps: [],
    } satisfies MatchResult
    const card = mapMatchToCard(result)
    expect(card.reasons).toHaveLength(3)
    expect(card.subtitle).toContain('1500㎡')
  })
})
