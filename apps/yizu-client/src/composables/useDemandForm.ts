import { ref } from 'vue'
import type { SpaceType } from '@/types/domain'
import type { useAuthStore } from '@/stores/auth'
import type { useDemandStore } from '@/stores/demand'
import { centsToYuanInput, yuanInputToCents } from '@/utils/money'

type DemandStore = ReturnType<typeof useDemandStore>
type AuthStore = ReturnType<typeof useAuthStore>

export function splitConditions(value: string): string[] {
  return value.split(/[、,，\n]/u).map((item) => item.trim()).filter(Boolean).slice(0, 20)
}

export function useDemandForm(store: DemandStore) {
  const rawText = ref('')
  const selectedType = ref<SpaceType | null>(null)
  const selectedTown = ref('')
  const areaMin = ref<number | null>(null)
  const areaMax = ref<number | null>(null)
  const budgetYuan = ref('')
  const rentMinYuan = ref('')
  const rentMaxYuan = ref('')
  const hardText = ref('')
  const preferenceText = ref('')

  function syncFromStore(): void {
    const demand = store.demand
    rawText.value = demand.raw_text
    selectedType.value = demand.constraints.space_type
    selectedTown.value = demand.constraints.target_towns[0] ?? ''
    areaMin.value = demand.constraints.area_min_sqm
    areaMax.value = demand.constraints.area_max_sqm
    budgetYuan.value = centsToYuanInput(demand.constraints.rent_max_cents)
    rentMinYuan.value = centsToYuanInput(demand.constraints.rent_min_cents)
    rentMaxYuan.value = centsToYuanInput(demand.constraints.rent_max_cents)
    hardText.value = demand.hard_conditions.join('、')
    preferenceText.value = demand.preference_conditions.join('、')
  }

  function applyHomeToStore(): void {
    const budgetCents = yuanInputToCents(budgetYuan.value)
    const demand = store.demand
    demand.raw_text = rawText.value
    demand.constraints.space_type = selectedType.value
    demand.constraints.target_towns = selectedTown.value ? [selectedTown.value] : []
    demand.constraints.area_min_sqm = areaMin.value
    demand.constraints.area_max_sqm = areaMax.value
    demand.constraints.rent_max_cents = budgetCents
    demand.constraints.rent_unit = demand.constraints.rent_max_cents === null ? null : 'yuan_per_month'
    store.persist()
  }

  function applyConfirmationToStore(): void {
    const rentMinCents = yuanInputToCents(rentMinYuan.value)
    const rentMaxCents = yuanInputToCents(rentMaxYuan.value)
    store.demand.constraints.rent_min_cents = rentMinCents
    store.demand.constraints.rent_max_cents = rentMaxCents
    store.demand.hard_conditions = splitConditions(hardText.value)
    store.demand.preference_conditions = splitConditions(preferenceText.value)
    store.persist()
  }

  return {
    rawText, selectedType, selectedTown, areaMin, areaMax, budgetYuan,
    rentMinYuan, rentMaxYuan, hardText, preferenceText,
    syncFromStore, applyHomeToStore, applyConfirmationToStore,
  }
}

export function restoreFlowState(auth: AuthStore, demand: DemandStore, syncPage: () => void): void {
  auth.hydrate()
  demand.hydrate()
  syncPage()
}
