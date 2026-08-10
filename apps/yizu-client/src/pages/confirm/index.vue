<script setup lang="ts">
import { computed, ref } from 'vue'
import { onShow } from '@dcloudio/uni-app'
import DemoBadge from '@/components/DemoBadge.vue'
import { miniappApi } from '@/api/miniapp'
import { mapApiError } from '@/api/client'
import { requireAuth } from '@/composables/useAuthGuard'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import type { RentUnit, SpaceType } from '@/types/domain'
import { validateConfirmedDemand } from '@/utils/validation'

const auth = useAuthStore()
const store = useDemandStore()
const demand = store.demand
const loading = ref(false)
const errorMessage = ref('')
const hardText = ref(demand.hard_conditions.join('、'))
const preferenceText = ref(demand.preference_conditions.join('、'))
const towns = ['莞城', '东城', '南城', '万江', '松山湖', '寮步', '大朗', '长安', '虎门', '厚街', '常平', '塘厦', '麻涌', '沙田']
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

const spaceLabel = computed(() => spaceOptions.find((item) => item.value === demand.constraints.space_type)?.label ?? '请选择')
const rentUnitLabel = computed(() => rentUnitOptions.find((item) => item.value === demand.constraints.rent_unit)?.label ?? '请选择')
const confidence = computed(() => `${Math.round(demand.ai_confidence * 100)}%`)

onShow(() => {
  auth.hydrate()
  store.hydrate()
  if (!requireAuth(auth)) return
  if (!store.interpretation) uni.reLaunch({ url: '/pages/home/index' })
})

function pickSpace(event: { detail: { value: number } }): void {
  demand.constraints.space_type = spaceOptions[event.detail.value]?.value ?? null
}

function pickRentUnit(event: { detail: { value: number } }): void {
  demand.constraints.rent_unit = rentUnitOptions[event.detail.value]?.value ?? null
}

function changeTowns(event: { detail: { value: string[] } }): void {
  demand.constraints.target_towns = event.detail.value.slice(0, 8)
}

function changeFreightElevator(event: unknown): void {
  const value = (event as { detail?: { value?: unknown } }).detail?.value
  demand.constraints.needs_freight_elevator = typeof value === 'boolean' ? value : null
}

function pickSublease(event: { detail: { value: number } }): void {
  demand.constraints.accepts_sublease = [null, true, false][event.detail.value] ?? null
}

function splitConditions(value: string): string[] {
  return value.split(/[、,，\n]/u).map((item) => item.trim()).filter(Boolean).slice(0, 20)
}

async function match(): Promise<void> {
  if (!requireAuth(auth)) return
  demand.hard_conditions = splitConditions(hardText.value)
  demand.preference_conditions = splitConditions(preferenceText.value)
  const errors = validateConfirmedDemand(demand)
  if (errors.length) {
    errorMessage.value = errors.join('；')
    return
  }
  loading.value = true
  errorMessage.value = ''
  store.matching = true
  store.persist()
  try {
    const response = await miniappApi.createMatches({ session_token: auth.session_token, demand })
    store.setMatches(response)
    uni.navigateTo({ url: '/pages/results/index' })
  } catch (error) {
    errorMessage.value = mapApiError(error)
  } finally {
    loading.value = false
    store.matching = false
  }
}
</script>

<template>
  <view class="page-shell content-width">
    <view class="summary card">
      <DemoBadge />
      <view class="summary-row"><text>AI提供方</text><text>{{ store.interpretation?.provider ?? 'local' }}</text></view>
      <view class="summary-row"><text>AI置信度</text><text class="confidence">{{ confidence }}</text></view>
      <view v-if="store.interpretation?.fallback_reason" class="notice">已回退本地解析：{{ store.interpretation.fallback_reason }}</view>
      <view v-if="demand.missing_fields.length" class="notice">尚缺必要字段：{{ demand.missing_fields.map((item) => missingLabels[item] ?? item).join('、') }}</view>
    </view>

    <view class="card form-card">
      <text class="section-title">确认并补充需求</text>
      <view class="field"><text class="field-label">原始描述</text><textarea v-model="demand.raw_text" class="textarea" maxlength="1000" /></view>
      <view class="row field">
        <view><text class="field-label">空间类型</text><picker :range="spaceOptions" range-key="label" @change="pickSpace"><view class="picker">{{ spaceLabel }}</view></picker></view>
        <view><text class="field-label">入驻时间</text><input v-model.trim="demand.constraints.move_in_time" class="input" placeholder="YYYY-MM-DD/立即" /></view>
      </view>

      <view class="field">
        <text class="field-label">目标镇街（可多选，最多8个）</text>
        <checkbox-group class="town-grid" @change="changeTowns">
          <label v-for="town in towns" :key="town" class="town-option"><checkbox :value="town" :checked="demand.constraints.target_towns.includes(town)" color="#8f1720" /><text>{{ town }}</text></label>
        </checkbox-group>
      </view>

      <view class="row field">
        <view><text class="field-label">面积下限（㎡）</text><input v-model.number="demand.constraints.area_min_sqm" class="input" type="number" /></view>
        <view><text class="field-label">面积上限（㎡）</text><input v-model.number="demand.constraints.area_max_sqm" class="input" type="number" /></view>
      </view>
      <view class="row field">
        <view><text class="field-label">预算下限（分）</text><input v-model.number="demand.constraints.rent_min_cents" class="input" type="number" placeholder="可选" /></view>
        <view><text class="field-label">预算上限（分）</text><input v-model.number="demand.constraints.rent_max_cents" class="input" type="number" placeholder="可选" /></view>
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
      <view class="field"><text class="field-label">其他补充</text><textarea v-model="demand.constraints.other_notes" class="textarea small" maxlength="500" /></view>
      <view class="field"><text class="field-label">硬条件（顿号/逗号分隔）</text><textarea v-model="hardText" class="textarea small" maxlength="1000" /></view>
      <view class="field"><text class="field-label">偏好条件（顿号/逗号分隔）</text><textarea v-model="preferenceText" class="textarea small" maxlength="1000" /></view>
      <text v-if="errorMessage" class="error">{{ errorMessage }}</text>
      <button class="primary-button" :disabled="loading" @click="match">{{ loading ? '正在匹配演示房源…' : '确认需求并开始匹配' }}</button>
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
</style>
