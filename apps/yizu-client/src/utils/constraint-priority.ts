import type { ConstraintKey, ConstraintLevel, DemandDraft } from '@/types/domain'

export const CONSTRAINT_KEYS: readonly ConstraintKey[] = [
  'budget', 'freight_elevator', 'elevator_capacity', 'power_capacity', 'fire_safety',
  'truck_access', 'loading_dock', 'sublease', 'floor', 'move_in',
]

export const CONSTRAINT_LEVELS: readonly ConstraintLevel[] = ['hard', 'preference', 'unspecified']

export function constraintHasValue(demand: DemandDraft, key: ConstraintKey): boolean {
  const value = demand.constraints
  const checks: Record<ConstraintKey, boolean> = {
    budget: value.rent_max_cents !== null && value.rent_unit !== null,
    freight_elevator: value.needs_freight_elevator === true,
    elevator_capacity: value.elevator_min_tons !== null,
    power_capacity: value.power_capacity_kva !== null,
    fire_safety: Boolean(value.fire_requirement),
    truck_access: Boolean(value.logistics_requirement),
    loading_dock: Boolean(value.loading_requirement),
    sublease: value.accepts_sublease !== null,
    floor: Boolean(value.floor_preference),
    move_in: Boolean(value.move_in_time),
  }
  return checks[key]
}

export function effectiveConstraintLevel(demand: DemandDraft, key: ConstraintKey): ConstraintLevel {
  const explicit = demand.constraint_priorities.find((item) => item.key === key)
  if (explicit) return explicit.level
  if (key === 'freight_elevator' && demand.constraints.needs_freight_elevator === true) return 'hard'
  return constraintHasValue(demand, key) ? 'preference' : 'unspecified'
}

export function setConstraintLevel(demand: DemandDraft, key: ConstraintKey, level: ConstraintLevel): void {
  demand.constraint_priorities = demand.constraint_priorities.filter((item) => item.key !== key)
  demand.constraint_priorities.push({ key, level })
}

export function materializeLegacyPriorities(demand: DemandDraft): DemandDraft {
  const copy = JSON.parse(JSON.stringify(demand)) as DemandDraft
  for (const key of CONSTRAINT_KEYS) {
    if (constraintHasValue(copy, key) && !copy.constraint_priorities.some((item) => item.key === key)) {
      setConstraintLevel(copy, key, effectiveConstraintLevel(copy, key))
    }
  }
  return copy
}
