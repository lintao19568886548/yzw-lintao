import type { DemandDraft } from '@/types/domain'

export function isValidPhone(phone: string): boolean {
  return /^1[3-9]\d{9}$/u.test(phone)
}

export function containsUnsafeMarkup(value: string): boolean {
  return /[<>]|javascript:|data:text\/html/iu.test(value)
}

export function validateInitialDemand(demand: DemandDraft): string[] {
  const errors: string[] = []
  const text = demand.raw_text.trim()
  if (!text) errors.push('请描述你的找房需求')
  if (text.length > 1_000) errors.push('需求描述不能超过1000字')
  if (containsUnsafeMarkup(text)) errors.push('需求描述不能包含HTML或脚本标记')
  return errors
}

export function validateConfirmedDemand(demand: DemandDraft): string[] {
  const errors = validateInitialDemand(demand)
  const constraints = demand.constraints
  if (!constraints.space_type) errors.push('请选择空间类型')
  if (constraints.target_towns.length === 0) errors.push('请至少选择一个目标镇街')
  if (constraints.area_min_sqm === null || constraints.area_max_sqm === null) {
    errors.push('请填写面积上下限')
  } else if (constraints.area_min_sqm <= 0 || constraints.area_min_sqm > constraints.area_max_sqm) {
    errors.push('面积范围无效')
  }
  if ((constraints.rent_min_cents !== null || constraints.rent_max_cents !== null) && !constraints.rent_unit) {
    errors.push('填写预算时必须选择租金单位')
  }
  return errors
}
