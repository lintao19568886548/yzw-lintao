<script setup lang="ts">
import { ref, watch } from 'vue'
import { onShow } from '@dcloudio/uni-app'
import BrandHeader from '@/components/BrandHeader.vue'
import DemoBadge from '@/components/DemoBadge.vue'
import { miniappApi } from '@/api/miniapp'
import { mapApiError } from '@/api/client'
import { requireAuth } from '@/composables/useAuthGuard'
import { restoreFlowState, useDemandForm } from '@/composables/useDemandForm'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import { useMetadataStore } from '@/stores/metadata'
import type { SpaceType } from '@/types/domain'
import { validateInitialDemand } from '@/utils/validation'

const auth = useAuthStore()
const demandStore = useDemandStore()
const metadata = useMetadataStore()
const {
  rawText, selectedType, selectedTown, areaMin, areaMax, budgetYuan,
  syncFromStore, applyHomeToStore,
} = useDemandForm(demandStore)
const loading = ref(false)
const errorMessage = ref('')

const typeOptions: Array<{ value: SpaceType; label: string }> = [
  { value: 'factory', label: '厂房' },
  { value: 'warehouse', label: '仓库' },
  { value: 'office', label: '写字楼' },
]

watch([rawText, selectedType, selectedTown, areaMin, areaMax, budgetYuan], () => {
  try { applyHomeToStore() } catch { /* 输入完成前保留上一份有效持久化快照。 */ }
})

onShow(async () => {
  restoreFlowState(auth, demandStore, syncFromStore)
  if (!requireAuth(auth)) return
  await metadata.load(demandStore.demand.constraints.target_towns)
})

function onTownChange(event: { detail: { value: number } }): void {
  selectedTown.value = metadata.towns[event.detail.value] ?? ''
}

async function interpret(): Promise<void> {
  if (!requireAuth(auth)) return
  try {
    applyHomeToStore()
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : '预算格式无效'
    return
  }
  const demand = demandStore.demand
  const errors = validateInitialDemand(demand)
  if (errors.length) {
    errorMessage.value = errors.join('；')
    return
  }
  loading.value = true
  errorMessage.value = ''
  try {
    const interpretation = await miniappApi.interpretDemand({ session_token: auth.session_token, draft: demand })
    demandStore.setInterpretation(interpretation)
    uni.navigateTo({ url: '/pages/confirm/index' })
  } catch (error) {
    errorMessage.value = mapApiError(error)
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <view>
    <BrandHeader />
    <view class="page-shell content-width">
      <view class="hero-copy">
        <DemoBadge />
        <text class="eyebrow">AI 产业空间顾问</text>
        <text class="headline">直接告诉我，{{ '\n' }}你想找什么空间</text>
        <text class="section-subtitle">先说大概想法，AI 会整理成可确认的需求，不确定的字段会明确标出。</text>
      </view>
      <view class="card demand-card">
        <textarea v-model="rawText" class="textarea" maxlength="1000" placeholder="想在松山湖附近找1500平方米左右的厂房，需要货梯和较大用电容量。" />
        <text class="counter">{{ rawText.length }}/1000</text>
        <view class="field">
          <text class="field-label">空间类型快捷标签</text>
          <view class="chips"><button v-for="item in typeOptions" :key="item.value" class="chip" :class="{ active: selectedType === item.value }" @click="selectedType = item.value">{{ item.label }}</button></view>
        </view>
        <view class="row field">
          <view>
            <text class="field-label">目标镇街</text>
            <picker :range="metadata.towns" @change="onTownChange"><view class="picker">{{ selectedTown || '请选择' }}</view></picker>
          </view>
          <view>
            <text class="field-label">月租预算（元）</text>
            <input v-model="budgetYuan" class="input" type="digit" placeholder="可选，最多两位小数" />
          </view>
        </view>
        <view v-if="metadata.notice" class="notice">{{ metadata.notice }}</view>
        <view class="row field">
          <view><text class="field-label">面积下限（㎡）</text><input v-model.number="areaMin" class="input" type="number" placeholder="如 1200" /></view>
          <view><text class="field-label">面积上限（㎡）</text><input v-model.number="areaMax" class="input" type="number" placeholder="如 1800" /></view>
        </view>
        <text v-if="errorMessage" class="error">{{ errorMessage }}</text>
        <button class="primary-button" :disabled="loading" @click="interpret">{{ loading ? 'AI 正在整理需求…' : '让 AI 帮我整理' }}</button>
        <button v-if="errorMessage" class="secondary-button" :disabled="loading" @click="interpret">重试</button>
      </view>
    </view>
  </view>
</template>

<style scoped>
.hero-copy { padding: 18rpx 8rpx 28rpx; }
.eyebrow, .headline { display: block; }
.eyebrow { margin-top: 24rpx; color: #a06f24; font-size: 23rpx; font-weight: 800; letter-spacing: 4rpx; }
.headline { margin: 12rpx 0 18rpx; color: #681116; font-family: serif; font-size: 54rpx; font-weight: 900; line-height: 1.25; }
.demand-card { position: relative; }
.counter { display: block; margin-top: 8rpx; text-align: right; color: #9a8479; font-size: 21rpx; }
</style>
