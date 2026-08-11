<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { onShow } from '@dcloudio/uni-app'
import DemoBadge from '@/components/DemoBadge.vue'
import DemandSummary from '@/components/DemandSummary.vue'
import { miniappApi } from '@/api/miniapp'
import { isSessionError, mapApiError } from '@/api/client'
import { requireAuth } from '@/composables/useAuthGuard'
import { restoreFlowState, useDemandForm } from '@/composables/useDemandForm'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import { useMetadataStore } from '@/stores/metadata'
import type { ConstraintKey, ConstraintLevel, RentUnit, SpaceType } from '@/types/domain'
import { constraintHasValue, effectiveConstraintLevel, setConstraintLevel } from '@/utils/constraint-priority'
import { validateConfirmedDemand } from '@/utils/validation'

const auth = useAuthStore()
const store = useDemandStore()
const metadata = useMetadataStore()
const { demand } = storeToRefs(store)
const { rentMinYuan, rentMaxYuan, hardText, preferenceText, syncFromStore, applyConfirmationToStore } = useDemandForm(store)
const loading = ref(false)
const errorMessage = ref('')
const spaceOptions: Array<{ value: SpaceType; label: string }> = [
  { value: 'factory', label: '厂房' }, { value: 'warehouse', label: '仓库' }, { value: 'office', label: '写字楼' },
]
const rentUnitOptions: Array<{ value: RentUnit; label: string }> = [
  { value: 'yuan_per_month', label: '元/月' },
  { value: 'yuan_per_square_metre_month', label: '元/平方米/月' },
]
const missingLabels: Record<string, string> = {
  space_type: '空间类型', target_towns: '目标镇街', area_range: '面积范围',
}
const constraintLabels: Record<ConstraintKey, string> = {
  budget: '预算', freight_elevator: '货梯', elevator_capacity: '电梯吨位', power_capacity: '用电容量',
  fire_safety: '消防要求', truck_access: '货车通行', loading_dock: '装卸条件', sublease: '接受分租',
  floor: '楼层', move_in: '入驻时间',
}
const priorityOptions: Array<{ value: ConstraintLevel; label: string }> = [
  { value: 'preference', label: '偏好' },
  { value: 'hard', label: '硬条件' },
  { value: 'unspecified', label: '不指定' },
]

const applicableConstraintKeys = computed<ConstraintKey[]>(() => {
  return (Object.keys(constraintLabels) as ConstraintKey[]).filter((key) => constraintHasValue(demand.value, key))
})

const spaceLabel = computed(() => spaceOptions.find((item) => item.value === demand.value.constraints.space_type)?.label ?? '请选择')
const rentUnitLabel = computed(() => rentUnitOptions.find((item) => item.value === demand.value.constraints.rent_unit)?.label ?? '请选择')
const confidence = computed(() => `${Math.round(demand.value.ai_confidence * 100)}%`)

onShow(async () => {
  restoreFlowState(auth, store, syncFromStore)
  if (!requireAuth(auth)) return
  if (!store.interpretation) { uni.switchTab({ url: '/pages/home/index' }); return }
  await metadata.load(demand.value.constraints.target_towns)
})

watch(demand, () => store.persist(), { deep: true })
watch([rentMinYuan, rentMaxYuan, hardText, preferenceText], () => {
  try { applyConfirmationToStore() } catch { /* 输入完成前保留上一份有效持久化快照。 */ }
})

function pickSpace(event: { detail: { value: number } }): void {
  demand.value.constraints.space_type = spaceOptions[event.detail.value]?.value ?? null
}

function pickRentUnit(event: { detail: { value: number } }): void {
  demand.value.constraints.rent_unit = rentUnitOptions[event.detail.value]?.value ?? null
}

function changeTowns(event: { detail: { value: string[] } }): void {
  demand.value.constraints.target_towns = event.detail.value.slice(0, 8)
}

function changeFreightElevator(event: unknown): void {
  const value = (event as { detail?: { value?: unknown } }).detail?.value
  demand.value.constraints.needs_freight_elevator = typeof value === 'boolean' ? value : null
  setConstraintLevel(demand.value, 'freight_elevator', value === true ? 'hard' : 'unspecified')
}

function pickSublease(event: { detail: { value: number } }): void {
  demand.value.constraints.accepts_sublease = [null, true, false][event.detail.value] ?? null
}

function priorityLabel(key: ConstraintKey): string {
  const labels: Record<ConstraintLevel, string> = { hard: '硬条件', preference: '偏好', unspecified: '不指定' }
  return labels[effectiveConstraintLevel(demand.value, key)]
}

function changePriority(key: ConstraintKey, event: { detail: { value: number } }): void {
  const selected = priorityOptions[event.detail.value]
  if (selected) setConstraintLevel(demand.value, key, selected.value)
}

async function match(): Promise<void> {
  if (!requireAuth(auth)) return
  try {
    applyConfirmationToStore()
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : '预算格式无效'
    return
  }
  const errors = validateConfirmedDemand(demand.value)
  if (errors.length) {
    errorMessage.value = errors.join('；')
    return
  }
  loading.value = true
  errorMessage.value = ''
  store.matching = true
  store.persist()
  try {
    const response = await miniappApi.createMatches({ session_token: auth.session_token, demand: demand.value })
    store.setMatches(response)
    uni.navigateTo({ url: '/pages/results/index' })
  } catch (error) {
    if (isSessionError(error)) { auth.clear(); requireAuth(auth); return }
    errorMessage.value = mapApiError(error)
  } finally {
    loading.value = false
    store.matching = false
  }
}
</script>

<template>
  <view class="page-shell content-width">
    <view class="page-head"><text class="page-kicker">AI 已完成第一轮理解</text><text class="page-title">请确认你的找房条件</text><text class="page-desc">信息不完整也没关系，补齐必要字段后即可匹配。硬条件会由服务端再次核验。</text></view>
    <view class="summary card">
      <DemoBadge />
      <view class="summary-row"><text>AI提供方</text><text>{{ store.interpretation?.provider ?? 'local' }}</text></view>
      <view class="summary-row"><text>AI置信度</text><text class="confidence">{{ confidence }}</text></view>
      <view v-if="store.interpretation?.fallback_reason" class="notice">已回退本地解析：{{ store.interpretation.fallback_reason }}</view>
      <view v-if="demand.missing_fields.length" class="notice">尚缺必要字段：{{ demand.missing_fields.map((item) => missingLabels[item] ?? item).join('、') }}</view>
      <view v-if="metadata.notice" class="notice">{{ metadata.notice }}</view>
      <view class="divider" /><DemandSummary :demand="demand" />
    </view>

    <view class="card form-card">
      <text class="section-title">确认并补充需求</text>
      <view class="field"><text class="field-label">原始描述</text><textarea v-model="demand.raw_text" class="textarea" maxlength="1000" /></view>
      <view class="row field">
        <view><text class="field-label">空间类型</text><picker :range="spaceOptions" range-key="label" @change="pickSpace"><view class="picker">{{ spaceLabel }}</view></picker></view>
        <view><text class="field-label">入驻时间</text><input v-model.trim="demand.constraints.move_in_time" class="input" placeholder="YYYY-MM-DD / immediate" /></view>
      </view>

      <view class="field">
        <text class="field-label">目标镇街（可多选，最多8个）</text>
        <checkbox-group class="town-grid" @change="changeTowns">
          <label v-for="town in metadata.towns" :key="town" class="town-option"><checkbox :value="town" :checked="demand.constraints.target_towns.includes(town)" color="#8f1720" /><text>{{ town }}</text></label>
        </checkbox-group>
      </view>

      <view class="row field">
        <view><text class="field-label">面积下限（㎡）</text><input v-model.number="demand.constraints.area_min_sqm" class="input" type="number" /></view>
        <view><text class="field-label">面积上限（㎡）</text><input v-model.number="demand.constraints.area_max_sqm" class="input" type="number" /></view>
      </view>
      <view class="row field">
        <view><text class="field-label">预算下限（元）</text><input v-model="rentMinYuan" class="input" type="digit" placeholder="可选，最多两位小数" /></view>
        <view><text class="field-label">预算上限（元）</text><input v-model="rentMaxYuan" class="input" type="digit" placeholder="可选，最多两位小数" /></view>
      </view>
      <view class="field"><text class="field-label">租金单位</text><picker :range="rentUnitOptions" range-key="label" @change="pickRentUnit"><view class="picker">{{ rentUnitLabel }}</view></picker></view>

      <view class="row field">
        <view><text class="field-label">楼层偏好</text><input v-model.trim="demand.constraints.floor_preference" class="input" placeholder="如 一楼/独栋" /></view>
        <view><text class="field-label">接受分租</text><picker :range="['待确认', '接受', '不接受']" @change="pickSublease"><view class="picker">{{ demand.constraints.accepts_sublease === null ? '待确认' : demand.constraints.accepts_sublease ? '接受' : '不接受' }}</view></picker></view>
      </view>

      <view class="switch-row field"><text class="field-label">需要货梯</text><switch :checked="demand.constraints.needs_freight_elevator === true" color="#8f1720" @change="changeFreightElevator" /></view>
      <view class="row field">
        <view><text class="field-label">电梯最低吨位</text><input v-model.number="demand.constraints.elevator_min_tons" class="input" type="digit" placeholder="如 3" /></view>
        <view><text class="field-label">用电/变压器（kVA）</text><input v-model.number="demand.constraints.power_capacity_kva" class="input" type="number" placeholder="如 500" /></view>
      </view>

      <view class="field"><text class="field-label">消防要求</text><input v-model.trim="demand.constraints.fire_requirement" class="input" placeholder="如 丙类消防" /></view>
      <view class="field"><text class="field-label">物流要求</text><input v-model.trim="demand.constraints.logistics_requirement" class="input" placeholder="如 17.5米货车通行" /></view>
      <view class="field"><text class="field-label">装卸要求</text><input v-model.trim="demand.constraints.loading_requirement" class="input" placeholder="如 需要装卸月台" /></view>
      <view v-if="applicableConstraintKeys.length" class="field">
        <text class="field-label">条件优先级（服务端强制执行硬条件）</text>
        <view v-for="key in applicableConstraintKeys" :key="key" class="priority-row">
          <text>{{ constraintLabels[key] }}</text>
          <picker :range="priorityOptions" range-key="label" @change="changePriority(key, $event)"><view class="picker compact">{{ priorityLabel(key) }}</view></picker>
        </view>
      </view>
      <view class="field"><text class="field-label">其他补充</text><textarea v-model="demand.constraints.other_notes" class="textarea small" maxlength="500" /></view>
      <view class="field"><text class="field-label">硬条件（顿号/逗号分隔）</text><textarea v-model="hardText" class="textarea small" maxlength="1000" /></view>
      <view class="field"><text class="field-label">偏好条件（顿号/逗号分隔）</text><textarea v-model="preferenceText" class="textarea small" maxlength="1000" /></view>
      <view v-if="errorMessage" class="error-panel">{{ errorMessage }}</view>
      <view class="sticky-action"><view><text class="action-title">条件确认完成</text><text class="muted">将匹配 3-10 套已核验房源</text></view><button class="primary-button" :disabled="loading" @click="match">{{ loading ? '正在匹配…' : '开始智能匹配' }}</button></view>
    </view>
  </view>
</template>

<style scoped>
.summary { margin-bottom: 24rpx; }
.summary-row { display: flex; justify-content: space-between; margin-top: 18rpx; color: #684f45; font-size: 25rpx; }
.confidence { color: #8f1720; font-weight: 900; }
.form-card { margin-bottom: 60rpx; }
.town-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 14rpx; }
.town-option { display: flex; align-items: center; gap: 6rpx; font-size: 23rpx; }
.switch-row { display: flex; align-items: center; justify-content: space-between; }
.switch-row .field-label { margin: 0; }
.textarea.small { min-height: 130rpx; }
.priority-row { display: flex; align-items: center; justify-content: space-between; margin-top: 12rpx; }
.picker.compact { min-width: 180rpx; padding: 12rpx 18rpx; }
.action-title { display: block; color: #5a302c; font-size: 24rpx; font-weight: 900; }
.sticky-action > view { flex: 0 0 235rpx; }.sticky-action .primary-button { min-width: 0; }
</style>
